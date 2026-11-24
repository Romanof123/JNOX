use core::arch::{asm, global_asm};
use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{Frame, LONGEST_MASKED, now};

const LIMIT: usize = 1024;
type Row = [u64; 7];

#[repr(C)]
struct Journal {
    reserved: AtomicU64,
    overflow: AtomicU64,
    sealed: AtomicU64,
    rows: [UnsafeCell<Row>; LIMIT],
}

// One atomic reservation owns one row, including across an intervening IRQ.
// A reader uses a stopped VM after the terminal line. No guest reader races a writer.
unsafe impl Sync for Journal {}

#[unsafe(no_mangle)]
static control_marker_journal: Journal = Journal {
    reserved: AtomicU64::new(0),
    overflow: AtomicU64::new(0),
    sealed: AtomicU64::new(0),
    rows: [const { UnsafeCell::new([0; 7]) }; LIMIT],
};

#[unsafe(no_mangle)]
static mut control_restore_samples: [u64; 3] = [0; 3];

pub(crate) fn record(kind: u64, t: u64, a: u64, b: u64) {
    let start = now();
    let j = &control_marker_journal;
    let i = j.reserved.fetch_add(1, Ordering::Relaxed);
    if i >= LIMIT as u64 {
        j.overflow.store(1, Ordering::Relaxed);
        return;
    }
    let slot = j.rows[i as usize].get().cast::<u64>();
    unsafe {
        for (k, v) in [kind, t, a, b, start].into_iter().enumerate() {
            ptr::write_volatile(slot.add(k), v);
        }
        let end = now();
        ptr::write_volatile(slot.add(5), end);
        ptr::write_volatile(slot.add(6), i + 1);
    }
}

pub(super) fn seal() {
    control_marker_journal.sealed.store(1, Ordering::Release);
}

pub(super) fn handler(kind: u64, frame: *mut Frame, entry: u64) {
    // IRQs are masked. The preceding restore owns all three words until the next ERET.
    let restore = unsafe { ptr::read_volatile(&raw const control_restore_samples) };
    record(4, restore[0], restore[1], restore[2]);
    record(kind, entry, unsafe { (*frame).elr }, now());
}

global_asm!(
    ".section .text.control_marker_entry,\"ax\"",
    ".balign 4",
    ".global control_marker_entry",
    ".type control_marker_entry,%function",
    "control_marker_entry:",
    "isb",
    "mrs x0,cntpct_el0",
    "bl {observe}",
    "b {body}",
    ".size control_marker_entry,.-control_marker_entry",
    observe = sym first,
    body = sym super::control,
);

unsafe extern "C" {
    pub(super) fn control_marker_entry() -> !;
}

extern "C" fn first(t: u64) {
    record(5, t, 0, 0);
}

pub(super) fn wait_phase(index: u64) {
    record(9, now(), 1, index);
    let resumed: u64;
    unsafe { asm!("svc #1", "isb", "mrs {},cntpct_el0", out(reg) resumed, options(nostack)) };
    record(10, resumed, index, 0);
}

// Only called on the control stack. Sender and idle retain the stock wrapper.
// Separate entry/exit rows also expose an interrupted or nonreturning section.
pub(super) fn control_locked<R>(f: impl FnOnce() -> R) -> R {
    let (before, prior): (u64, u64);
    unsafe {
        asm!("mrs {prior},daif", "isb", "mrs {before},cntpct_el0", "msr daifset,#2",
            prior=out(reg)prior, before=out(reg)before, options(nostack));
    }
    let begin = now();
    record(6, before, begin, prior);
    let r = f();
    let end = now();
    unsafe { LONGEST_MASKED = LONGEST_MASKED.max((end - begin, begin, end)) };
    let (restore, after): (u64, u64);
    unsafe {
        asm!("isb", "mrs {restore},cntpct_el0", "msr daif,{prior}", "isb", "mrs {after},cntpct_el0",
            prior=in(reg)prior, restore=out(reg)restore, after=out(reg)after, options(nostack));
    }
    record(7, restore, after, prior);
    r
}
