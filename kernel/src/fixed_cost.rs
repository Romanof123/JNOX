//! CPU0 alone publishes with exceptions masked. Read only after QMP stop.
use super::{Frame, frequency};
use core::arch::asm;

const CAPACITY: usize = 128;
#[repr(C)]
struct Journal {
    hz: u64,
    calibration: [u64; 2],
    count: u64,
    dropped: u64,
    rows: [[u64; 7]; CAPACITY],
}

#[unsafe(no_mangle)]
static mut fixed_cost_records: Journal = Journal {
    hz: 0, calibration: [0; 2], count: 0, dropped: 0, rows: [[0; 7]; CAPACITY],
};

#[inline(always)]
pub(super) fn stamp() -> u64 {
    let ticks;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", "isb", out(reg) ticks, options(nostack, preserves_flags)) }
    ticks
}

pub(super) fn init() {
    unsafe {
        fixed_cost_records.hz = frequency();
        fixed_cost_records.calibration = [stamp(), stamp()];
    }
}

pub(super) fn record(op: u64, incoming: *mut Frame, next: *mut Frame, times: [u64; 4]) {
    unsafe {
        let journal = &mut *(&raw mut fixed_cost_records);
        let Some(row) = journal.rows.get_mut(journal.count as usize) else {
            journal.dropped = journal.dropped.saturating_add(1);
            return;
        };
        *row = [op, incoming.addr() as u64, next.addr() as u64, times[0], times[1], times[2], times[3]];
        journal.count += 1;
    }
}
