//! The linked services emit their normal structured reports to this private
//! sink. Ring snapshots read the real channel's state through its reporter.
use core::ptr::{read_volatile, write_volatile};

#[used]
#[unsafe(no_mangle)]
pub(super) static mut permit_queue_header: [u64; 12] = [0; 12];
#[used]
#[unsafe(no_mangle)]
pub(super) static mut permit_queue_info: [[u64; 7]; 6] = [[0; 7]; 6];
#[used]
#[unsafe(no_mangle)]
pub(super) static mut permit_queue_ring: [[u64; 8]; 4] = [[0; 8]; 4];
#[used]
#[unsafe(no_mangle)]
static mut permit_queue_pages: [[[u64; 4]; 3]; 4] = [[[0; 4]; 3]; 4];
#[used]
#[unsafe(no_mangle)]
static mut permit_queue_maps: [[u64; 10]; 24] = [[0; 10]; 24];
#[used]
#[unsafe(no_mangle)]
static mut permit_queue_map_count: u64 = 0;
#[used]
#[unsafe(no_mangle)]
pub(super) static mut permit_queue_frames: [[u8; 44]; 2] = [[0; 44]; 2];
#[used]
#[unsafe(no_mangle)]
pub(super) static mut permit_queue_attempts: [i64; 7] = [i64::MIN; 7];
#[used]
#[unsafe(no_mangle)]
static mut permit_queue_events: [[u64; 10]; 160] = [[0; 10]; 160];
#[used]
#[unsafe(no_mangle)]
static mut permit_queue_event_count: u64 = 0;
#[used]
#[unsafe(no_mangle)]
static mut permit_queue_event_loss: u64 = u64::MAX;

#[derive(Clone, Copy)]
enum Row {
    Ignore,
    Map,
    Ring,
    Page,
    Safety,
    Loss,
}

static mut ROW: Row = Row::Ignore;
static mut WORDS: [u64; 10] = [0; 10];
static mut USED: usize = 0;
static mut RING: Option<[u64; 8]> = None;
static mut STAGE: usize = 0;
static mut PAGE_COUNT: usize = 0;

pub(super) fn uart_write(bytes: &[u8]) {
    match bytes {
        b" " => {}
        b"\n" => unsafe {
            let words = WORDS;
            match ROW {
                Row::Ring if USED == 8 => RING = Some(words[..8].try_into().unwrap()),
                Row::Map if USED == 10 && permit_queue_map_count < 24 => {
                    let index = permit_queue_map_count as usize;
                    write_volatile(&raw mut permit_queue_maps[index], words);
                    write_volatile(&raw mut permit_queue_map_count, (index + 1) as u64);
                }
                Row::Page if USED == 4 && PAGE_COUNT < 3 && words[0] == PAGE_COUNT as u64 => {
                    write_volatile(
                        &raw mut permit_queue_pages[STAGE][PAGE_COUNT],
                        words[..4].try_into().unwrap(),
                    );
                    PAGE_COUNT += 1;
                }
                Row::Safety if USED == 10 && permit_queue_event_count < 160 => {
                    let index = permit_queue_event_count as usize;
                    write_volatile(&raw mut permit_queue_events[index], words);
                    write_volatile(&raw mut permit_queue_event_count, (index + 1) as u64);
                }
                Row::Loss if USED == 1 => {
                    write_volatile(&raw mut permit_queue_event_loss, words[0])
                }
                Row::Ignore => {}
                _ => super::fail(10),
            }
            ROW = Row::Ignore;
        },
        tag => unsafe {
            USED = 0;
            ROW = match tag {
                b"NATIVE LOAN STATE" => Row::Ring,
                b"NATIVE LOAN MAP" => Row::Map,
                b"NATIVE LOAN PAGE" => Row::Page,
                b"NATIVE SAFETY EVENT" => Row::Safety,
                b"NATIVE SAFETY DROPPED " => Row::Loss,
                _ => Row::Ignore,
            };
        },
    }
}

pub(super) fn unsigned(word: u64) {
    unsafe {
        if !matches!(ROW, Row::Ignore) {
            if USED >= 10 {
                super::fail(10);
            }
            WORDS[USED] = word;
            USED += 1;
        }
    }
}

pub(super) fn ring(stage: usize, queued_handle: Option<u64>, live: bool) -> bool {
    unsafe {
        STAGE = stage;
        RING = None;
        PAGE_COUNT = 0;
        write_volatile(&raw mut permit_queue_map_count, 0);
    }
    super::loans::report();
    let Some(state) = (unsafe { RING }) else {
        return false;
    };
    unsafe {
        write_volatile(&raw mut permit_queue_ring[stage], state);
    }
    let used = u64::from(queued_handle.is_some());
    if unsafe { PAGE_COUNT != 3 || permit_queue_map_count != state[6] } {
        return false;
    }
    for slot in 0..3 {
        let row = unsafe { read_volatile(&raw const permit_queue_pages[stage][slot]) };
        let slot = slot as u64;
        let expected = match queued_handle {
            Some(handle) if handle & 3 == slot => [slot, 2, handle, 44],
            _ => [slot, 0, 0, 0],
        };
        if row != expected {
            return false;
        }
    }
    state[1].checked_sub(state[0]) == Some(used)
        && state[2] == used
        && state[3] == 1
        && state[4] == u64::from(live)
        && state[5] == u64::from(live)
        && state[7] == 0
}

pub(super) fn decision_and_identity(
    tick: u64,
    issue: u64,
    final_seq: u64,
    final_issue: u64,
    streak: u64,
) -> bool {
    let mut saw_check = 0;
    let mut saw_queue = 0;
    let mut saw_complete = 0;
    let handle = unsafe { read_volatile(&raw const permit_queue_header[10]) };
    let count = unsafe { read_volatile(&raw const permit_queue_event_count) };
    if unsafe { read_volatile(&raw const permit_queue_event_loss) } != 0 {
        return false;
    }
    for i in 0..count as usize {
        let row = unsafe { read_volatile(&raw const permit_queue_events[i]) };
        if row[0] == 12 && row[2] == tick && row[3] == 1 {
            if row[4..] != [1, 3, issue, 0, u64::from(final_seq == 3), 1] {
                return false;
            }
            saw_check += 1;
        }
        if row[0] == 17 && row[2] == tick {
            let flags = if final_seq == 3 { 3 } else { 1 };
            if row[3..] != [1, 3, 1, handle, 44, 1, flags] {
                return false;
            }
            saw_queue += 1;
        }
        if row[0] == 10 && row[2] == tick {
            if row[4..] != [0, 1, final_seq, final_issue, 1, streak] {
                return false;
            }
            saw_complete += 1;
        }
    }
    saw_check == 1 && saw_queue == 1 && saw_complete == 1
}
