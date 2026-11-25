//! Ordinary tasks for the two protected sensor-report epochs.
use super::{call, checked, exit, line, stop, ADMIT, LOCK, TIMED, UNLOCK, X, Y};
use core::{arch::asm, ptr, sync::atomic::{AtomicU64, Ordering::SeqCst}};

const NA: u64 = u64::MAX;
const EXPECTED_BATCH: [u64; 2] = [4_194_304, 35_651_584];
static INPUT: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
static CHUNKS: AtomicU64 = AtomicU64::new(0);
static IN_WORK: AtomicU64 = AtomicU64::new(0);

// Worker access to BATCH is serialized by Y, and access to REPORT by X.
// Masked exception handlers can sample the suspended worker on this sole CPU.
static mut BATCH: [u64; 2] = [0; 2];
#[repr(C)]
#[derive(Clone, Copy)]
struct Report {
    batch: [u64; 2],
    appends: u64,
    value: u64,
}
const EMPTY: Report = Report { batch: [0; 2], appends: 0, value: 0 };
static mut REPORT: Report = EMPTY;
static mut EPOCH: u64 = 0;
static mut HIGH_RESULT: u64 = NA;
static mut MED_SUM: u64 = NA;
static mut APPEND_ROWS: [[u64; 9]; 2] = [[NA; 9]; 2];
static mut RETURN_ROWS: [[u64; 2]; 2] = [[NA; 2]; 2];

pub(super) fn begin(epoch: usize) {
    CHUNKS.store(0, SeqCst);
    IN_WORK.store(0, SeqCst);
    unsafe {
        EPOCH = epoch as u64;
        BATCH = [0; 2];
        REPORT = EMPTY;
        HIGH_RESULT = NA;
        MED_SUM = NA;
        APPEND_ROWS = [[NA; 9]; 2];
        RETURN_ROWS = [[NA; 2]; 2];
    }
}

pub(super) fn progress() -> (u64, u64) {
    (CHUNKS.load(SeqCst), IN_WORK.load(SeqCst))
}

pub(super) fn snapshot() -> [u64; 10] {
    let (chunks, in_work) = progress();
    unsafe {
        let batch = ptr::read_volatile(&raw const BATCH);
        let report = ptr::read_volatile(&raw const REPORT);
        [in_work, chunks, batch[0], batch[1], report.batch[0], report.batch[1],
         report.appends, report.value, HIGH_RESULT, MED_SUM]
    }
}

pub(super) fn dump() {
    let epoch = unsafe { EPOCH };
    let mut state = [epoch; 11];
    state[1..].copy_from_slice(&snapshot());
    line(b"WP", &state);
    for index in 0..2 {
        let mut row = [epoch; 11];
        row[1] = index as u64;
        row[2..].copy_from_slice(&unsafe { ptr::read_volatile(&raw const APPEND_ROWS[index]) });
        line(b"WA", &row);
    }
    for (index, task) in [1, 3].into_iter().enumerate() {
        let [result, sp] = unsafe { ptr::read_volatile(&raw const RETURN_ROWS[index]) };
        line(b"WR", &[epoch, task, result, sp]);
    }
}

#[inline(always)]
fn stack_pointer() -> u64 {
    let sp;
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    sp
}

extern "C" fn base() -> ! {
    checked(LOCK, Y);
    let mut sum = 0;
    let mut count = 0;
    IN_WORK.store(1, SeqCst);
    for chunk in 1..=16_384 {
        for i in 0..256 {
            sum += unsafe { ptr::read_volatile(INPUT.as_ptr().add(i & 15)) } as u64;
            count += 1;
        }
        CHUNKS.store(chunk, SeqCst);
    }
    unsafe { ptr::write_volatile(&raw mut BATCH, [count, sum]) };
    IN_WORK.store(0, SeqCst);
    if [count, sum] != EXPECTED_BATCH { stop(31) }
    checked(UNLOCK, Y);
    exit()
}

extern "C" fn low() -> ! {
    checked(LOCK, X);
    unsafe { ptr::write_volatile(&raw mut REPORT.value, 4) };
    checked(ADMIT, 0);
    checked(ADMIT, 0);
    let result = call(LOCK, Y);
    unsafe { ptr::write_volatile(&raw mut RETURN_ROWS[0], [result, stack_pointer()]) };
    if result != 0 { stop(32) }
    let batch = unsafe { ptr::read_volatile(&raw const BATCH) };
    if batch != EXPECTED_BATCH { stop(33) }
    unsafe { ptr::write_volatile(&raw mut REPORT.batch, batch) };
    checked(UNLOCK, Y);
    checked(UNLOCK, X);
    exit()
}

extern "C" fn medium() -> ! {
    let sum = (0..256).map(|i| unsafe { ptr::read_volatile(INPUT.as_ptr().add(i & 15)) } as u64).sum();
    unsafe { ptr::write_volatile(&raw mut MED_SUM, sum) };
    if sum != 2_176 { stop(34) }
    exit()
}

extern "C" fn high() -> ! {
    let result = call(TIMED, X);
    unsafe {
        ptr::write_volatile(&raw mut HIGH_RESULT, result);
        ptr::write_volatile(&raw mut RETURN_ROWS[1], [result, stack_pointer()]);
    }
    if result != 1 { stop(35) }
    exit()
}

fn append(task: u64, digit: u64) -> ! {
    let result = call(LOCK, X);
    let sp = stack_pointer();
    if result != 0 { stop(36) }
    let before = unsafe { ptr::read_volatile(&raw const REPORT) };
    if before.batch != EXPECTED_BATCH || match before.appends {
        0 => before.value != 4,
        1 => before.value != 40 + (3 - digit),
        _ => true,
    } { stop(37) }
    unsafe {
        ptr::write_volatile(&raw mut REPORT.appends, before.appends + 1);
        ptr::write_volatile(&raw mut REPORT.value, before.value * 10 + digit);
        let after = ptr::read_volatile(&raw const REPORT);
        ptr::write_volatile(&raw mut APPEND_ROWS[before.appends as usize], [
            task, result, sp, before.batch[0], before.batch[1], before.appends,
            before.value, after.appends, after.value,
        ]);
    }
    checked(UNLOCK, X);
    exit()
}

extern "C" fn reporter_a() -> ! { append(4, 1) }
extern "C" fn reporter_b() -> ! { append(5, 2) }

pub(super) const ENTRIES: [extern "C" fn() -> !; 6] = [base, low, medium, high, reporter_a, reporter_b];
