//! Fixed DMA buffers for two split rings. SVCs serialize on one masked CPU.
//! Descriptor IDs and payload addresses are selected exclusively by EL1.
use super::{abort_setup, fence, jnox_linux_net_queue_pool, read, write, Ring,
            ACK_DRIVER, ACTIVE_TOKEN, FEATURES_OK, SIZE, STATUS};
use crate::native_el0_mmu::{copy_from_user, copy_to_user};
use core::{ptr, slice};

const RX: usize = 0;
const TX: usize = 1;
const HEADER: usize = 12;
const MIN: usize = HEADER + 14;
const CAPACITY: usize = HEADER + 1514;
const NOTIFY: usize = 0x50;
const INTERRUPT_STATUS: usize = 0x60;
const INTERRUPT_ACK: usize = 0x64;
const LIVE: u32 = ACK_DRIVER | FEATURES_OK | 4;

#[repr(C, align(16))]
struct Buffer([u8; CAPACITY]);

#[unsafe(no_mangle)]
static mut jnox_linux_net_packet_buffers: [[Buffer; SIZE]; 2] = unsafe { core::mem::zeroed() };

#[derive(Clone, Copy, Default)]
struct Completion { id: usize, len: usize }

#[derive(Clone, Copy, Default)]
struct Cursor {
    available: u16,
    used: u16,
    device_owned: u32,
    held: u32,
    completions: [Completion; SIZE],
    head: usize,
    count: usize,
}

static mut CURSORS: [Cursor; 2] = unsafe { core::mem::zeroed() };
static mut RUNNING_TOKEN: u64 = 0;

pub(super) fn running(token: u64) -> bool { token != 0 && token == unsafe { RUNNING_TOKEN } }

pub(super) fn retired() {
    // Only called once the transport confirmed Status=0 and both queues idle.
    unsafe {
        RUNNING_TOKEN = 0;
        CURSORS = [Cursor::default(); 2];
        ptr::write_bytes(&raw mut jnox_linux_net_packet_buffers, 0, 1);
    }
    fence();
}

fn ring(q: usize) -> *mut Ring { unsafe { &raw mut jnox_linux_net_queue_pool[q] } }
fn buffer(q: usize, id: usize) -> *mut u8 {
    unsafe { (&raw mut jnox_linux_net_packet_buffers[q][id]).cast() }
}

fn post(cursor: &mut Cursor, q: usize, id: usize, len: usize) {
    let ring = ring(q);
    let slot = usize::from(cursor.available) % SIZE;
    // Each descriptor references one contiguous kernel buffer, unchained.
    // Low 32 bits in word 1 are len, next 16 flags, then next=0 (little-endian).
    let flags = if q == RX { 2u64 } else { 0 };
    unsafe {
        (&raw mut (*ring).desc[id][0]).write_volatile(buffer(q, id).addr() as u64);
        (&raw mut (*ring).desc[id][1]).write_volatile((flags << 32) | len as u64);
        (&raw mut (*ring).available[2 + slot]).write_volatile(id as u16);
        cursor.device_owned |= 1u32 << id;
        fence();
        cursor.available = cursor.available.wrapping_add(1);
        (&raw mut (*ring).available[1]).write_volatile(cursor.available);
    }
    fence();
}

fn harvest(cursor: &mut Cursor, q: usize) -> Result<(), i64> {
    let ring = ring(q);
    let head = unsafe { (&raw const (*ring).used_header[1]).read_volatile() };
    fence();
    let delta = usize::from(head.wrapping_sub(cursor.used));
    if delta > cursor.device_owned.count_ones() as usize || delta > SIZE - cursor.count {
        return Err(-5);
    }
    for _ in 0..delta {
        let slot = usize::from(cursor.used) % SIZE;
        let id = unsafe { (&raw const (*ring).used[slot][0]).read_volatile() as usize };
        let len = unsafe { (&raw const (*ring).used[slot][1]).read_volatile() as usize };
        if id >= SIZE || cursor.device_owned & (1u32 << id) == 0
            || (q == TX && len != 0) || (q == RX && !(MIN..=CAPACITY).contains(&len)) {
            return Err(-5);
        }
        cursor.device_owned &= !(1u32 << id);
        cursor.held |= 1u32 << id;
        cursor.completions[(cursor.head + cursor.count) % SIZE] = Completion { id, len };
        cursor.count += 1;
        cursor.used = cursor.used.wrapping_add(1);
    }
    Ok(())
}

fn pop(cursor: &mut Cursor, id: usize) {
    cursor.head = (cursor.head + 1) % SIZE;
    cursor.count -= 1;
    cursor.held &= !(1u32 << id);
}

fn start(token: u64) -> i64 {
    if running(token) { return -16; }
    if read(STATUS) != (ACK_DRIVER | FEATURES_OK) { return abort_setup(); }
    for q in [RX, TX] {
        write(super::SELECT, q as u32);
        if read(super::READY) != 1 { return abort_setup(); }
    }
    let rx = unsafe { &mut *(&raw mut CURSORS[RX]) };
    for id in 0..SIZE { post(rx, RX, id, CAPACITY); }
    write(STATUS, LIVE);
    if read(STATUS) != LIVE { return abort_setup(); }
    unsafe { RUNNING_TOKEN = token };
    write(NOTIFY, RX as u32);
    0
}

fn submit(tx: &mut Cursor, address: usize, len: usize) -> i64 {
    if !(MIN..=CAPACITY).contains(&len) { return -90; }
    let unavailable = tx.device_owned | tx.held;
    let id = unavailable.trailing_ones() as usize;
    if id == SIZE { return -11; }
    // Only touch a free TX buffer. Keep the exact upstream-generated header
    // and packet; neither validation failure nor a copy fault publishes it.
    let bytes = unsafe { slice::from_raw_parts_mut(buffer(TX, id), len) };
    if !copy_from_user(address, bytes) { return -14; }
    if bytes[..HEADER] != [0; HEADER] { return -22; }
    post(tx, TX, id, len);
    id as i64
}

fn completion(cursor: &mut Cursor, q: usize, address: usize, cap: usize) -> Result<i64, i64> {
    harvest(cursor, q)?;
    if cursor.count == 0 { return Ok(-11); }
    let Completion { id, len } = cursor.completions[cursor.head];
    if q == TX {
        pop(cursor, id);
        return Ok(id as i64);
    }
    if cap < len { return Ok(-90); }
    let bytes = unsafe { slice::from_raw_parts(buffer(RX, id), len) };
    if !copy_to_user(address, bytes) { return Ok(-14); }
    pop(cursor, id);
    post(cursor, RX, id, CAPACITY);
    write(NOTIFY, RX as u32);
    Ok(len as i64)
}

fn tx_callbacks(mode: u64, reserved: u64) -> i64 {
    if reserved != 0 || mode > 1 { return -22; }
    const NO_INTERRUPT: u16 = 1;
    let ring = ring(TX);
    // EVENT_IDX is excluded by feature negotiation. EL1 is the sole writer
    // of avail.flags. Publish this change before sampling the device index.
    let flags = unsafe { &raw mut (*ring).available[0] };
    let desired = if mode == 0 { NO_INTERRUPT } else { 0 };
    unsafe { flags.write_volatile(desired) };
    fence();
    if unsafe { flags.read_volatile() } != desired { return abort_setup(); }
    if mode == 0 { return 0; }

    let cursor = unsafe { &*(&raw const CURSORS[TX]) };
    let device_used = unsafe { (&raw const (*ring).used_header[1]).read_volatile() };
    fence();
    i64::from(cursor.count == 0 && device_used == cursor.used)
}

pub(in crate::native_el0::linux_net) fn packet_service(number: u64, token: u64, address: u64, len: u64) -> i64 {
    if token == 0 || token != unsafe { ACTIVE_TOKEN as u64 } { return -116; }
    if number == 163 { return start(token); }
    if !running(token) { return -19; }
    if read(STATUS) != LIVE { return abort_setup(); }
    let result = match number {
        164 => return submit(unsafe { &mut *(&raw mut CURSORS[TX]) }, address as usize, len as usize),
        165 => { write(NOTIFY, TX as u32); return 0; }
        166 => completion(unsafe { &mut *(&raw mut CURSORS[TX]) }, TX, 0, 0),
        167 => completion(unsafe { &mut *(&raw mut CURSORS[RX]) }, RX, address as usize, len as usize),
        169 => return tx_callbacks(address, len),
        168 => {
            let pending = read(INTERRUPT_STATUS) & 3;
            if pending != 0 { write(INTERRUPT_ACK, pending); }
            return i64::from(pending);
        }
        _ => return -38,
    };
    result.unwrap_or_else(|_| abort_setup())
}
