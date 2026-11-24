//! Failure-only UART and RAM witness for the single-core legacy CBS fixture.
//! The checked-in cbs_condition_v1.csv defines the versioned wire operands.

use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use super::{Bench, EXPECTED, Frame, TIMER_IRQ};
use crate::temporal_policy::{CbsError, HardCbs};
use crate::{park, uart_write};

static WRITING: AtomicBool = AtomicBool::new(false);
#[unsafe(no_mangle)]
static mut cbs_first_condition_v1: [u64; 5] = [0; 5];

fn decimal(mut value: u64) {
    let mut digits = [0u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            uart_write(&digits[start..]);
            return;
        }
    }
}

#[cold]
#[inline(never)]
fn stop(site: u64, mask: u64, a: u64, b: u64, c: u64) -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    // The fixture has one CPU. A nested unexpected exception must never
    // replace or print a second first-failure record.
    if WRITING.swap(true, Ordering::Relaxed) {
        park();
    }
    let row = [site, mask, a, b, c];
    unsafe { ptr::write_volatile(&raw mut cbs_first_condition_v1, row) };
    uart_write(b"CBS DIAG1");
    for word in row {
        uart_write(b" ");
        decimal(word);
    }
    uart_write(b"\nCBS FAIL\n");
    park()
}

pub(super) fn no_operands(site: u64) -> ! {
    stop(site, 0, 0, 0, 0)
}

pub(super) fn pair(site: u64, a: u64, b: u64) -> ! {
    stop(site, 3, a, b, 0)
}

pub(super) fn equal(site: u64, actual: u64, expected: u64) {
    if actual != expected {
        pair(site, actual, expected);
    }
}

pub(super) fn at_most(site: u64, actual: u64, limit: u64) {
    if actual > limit {
        pair(site, actual, limit);
    }
}

pub(super) fn irq_masked(daif: u64) {
    if daif & 0x80 == 0 {
        pair(4, daif, 0x80);
    }
}

pub(super) extern "C" fn unexpected_sync() -> ! {
    no_operands(2)
}

// Called only from the taken failure branch in the task assembly. These
// registers already hold the compared values, including TPIDR or stack loads.
#[unsafe(no_mangle)]
extern "C" fn cbs_condition_asm_fail(site: u64, actual: u64, expected: u64, id: u64) -> ! {
    stop(site, 7, actual, expected, id)
}

pub(super) fn positive_quantum(quantum: u64) {
    if quantum == 0 {
        pair(8, quantum, 0);
    }
}

pub(super) fn boundary(epoch: u64, quantum: u64, index: u64) -> u64 {
    let delta = quantum.checked_mul(index).unwrap_or_else(|| pair(9, quantum, index));
    epoch.checked_add(delta).unwrap_or_else(|| pair(10, epoch, delta))
}

fn error_fields(err: CbsError) -> (u64, Option<usize>) {
    match err {
        CbsError::UnknownServer => (1, None),
        CbsError::EmptyJob => (2, None),
        CbsError::JobPending => (3, None),
        CbsError::MissedDeadline(id) => (4, Some(id)),
        CbsError::TimeOverflow => (5, None),
    }
}

pub(super) fn release(policy: &mut HardCbs<2>, id: usize, work: u32) {
    if let Err(err) = policy.release(id, work) {
        let (code, missed) = error_fields(err);
        if let Some(server) = missed {
            stop(11, 7, id as u64, code, server as u64);
        }
        stop(7, 7, id as u64, code, u64::from(work));
    }
}

pub(super) fn tick_error(slot: usize, err: CbsError) -> ! {
    let (code, missed) = error_fields(err);
    if let Some(server) = missed {
        stop(42, 7, slot as u64, code, server as u64);
    }
    pair(35, slot as u64, code)
}

pub(super) fn timer_source(iar: u32, control: u64) {
    if iar & 0x3ff != TIMER_IRQ {
        pair(20, u64::from(iar), u64::from(TIMER_IRQ));
    }
    if control & 7 != 5 {
        pair(21, control, 5);
    }
}

pub(super) fn range(site: u64, first: u64, end: u64, sample: u64) {
    if !(first..end).contains(&sample) {
        stop(site, 7, sample, first, end);
    }
}

pub(super) fn outgoing_owner(bench: &Bench, id: usize) {
    equal(23, bench.frames[id].addr() as u64, 0);
    equal(24, bench.markers as u64, bench.policy.now());
}

pub(super) fn registers(saved: &Frame, witness: u64) {
    if saved.spsr & 0x8f != 5 {
        pair(27, saved.spsr, 5);
    }
    equal(28, saved.gpr[19], witness);
    equal(29, saved.tpidr_el1, witness);
}

pub(super) fn progress(bench: &Bench, id: usize, count: u64, witness: u64) {
    if count >> 32 != witness {
        pair(30, count, witness);
    }
    if count as u32 == 0 {
        pair(31, count, 0);
    }
    let previous = bench.progress[id];
    if count <= previous {
        pair(32, count, previous);
    }
}

pub(super) fn incoming_frame(frame: *mut Frame) {
    if frame.is_null() {
        stop(37, 1, frame.addr() as u64, 0, 0);
    }
}

pub(super) fn before_deadline(sample: u64, deadline: u64) {
    if sample >= deadline {
        pair(39, sample, deadline);
    }
}

pub(super) fn marker_owner(bench: &Bench, slot: usize, id: usize) {
    let expected = EXPECTED.get(slot).unwrap_or_else(|| pair(50, slot as u64, EXPECTED.len() as u64));
    equal(51, id as u64, *expected as u64);
    equal(52, bench.current as u64, id as u64);
}

pub(super) fn terminal_frames(bench: &Bench) {
    for (id, frame) in bench.frames.iter().enumerate() {
        if frame.is_null() {
            pair(40, id as u64, frame.addr() as u64);
        }
    }
}

pub(super) fn terminal_progress(bench: &Bench) {
    for (id, &count) in bench.progress.iter().enumerate() {
        if count == 0 {
            pair(41, id as u64, count);
        }
    }
}
