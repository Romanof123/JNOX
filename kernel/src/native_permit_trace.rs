//! Qualification only: single masked CPU, append-only, exported at terminal.
use super::{counter, uart_write, unsigned, Action, NativeFrame, CLOCK_DOMAIN};
use core::arch::asm;

const NONE: u64 = u64::MAX;
const LIMIT: usize = 1024;
static mut ARMED: [Option<u64>; 3] = [None; 3];
static mut LAST_COMPARE: Option<Option<u64>> = None;

/// Accessed only on the single masked CPU. A target ceases to be pending at
/// the scheduler transition making its process runnable.
pub(super) fn armed_target(pid: usize) -> Option<u64> {
    if pid < 3 { unsafe { ARMED[pid] } } else { None }
}
type Row = [u64; 12];
static mut ROWS: [Row; LIMIT] = [[0; 12]; LIMIT];
static mut ATTEMPTS: usize = 0;
// IRQ- and user-return assembly fill columns 10 and 11, then clear the pointer.
#[unsafe(no_mangle)]
static mut native_permit_irq_return: usize = 0;
#[unsafe(no_mangle)]
static mut native_permit_user_return: usize = 0;

fn append(row: Row) -> usize {
    unsafe {
        let i = ATTEMPTS;
        ATTEMPTS = i.saturating_add(1);
        if i >= LIMIT { return 0; }
        ROWS[i] = row;
        (&raw mut ROWS[i][10]).addr()
    }
}

pub(super) fn mark(pid: usize, f: &mut NativeFrame, entry: u64, now: u64) -> Action {
    let (job, phase, stage, target) = (f.x[0], f.x[1], f.x[2], f.x[3]);
    let valid = matches!((pid, phase), (1, 1 | 2) | (0, 3) | (2, 4))
        && (1..=3).contains(&stage);
    let delay = target.saturating_sub(now);
    let (status, action) = if !valid {
        (-22i64, Action::Poll)
    } else if stage == 1 && delay > i64::MAX as u64 {
        (-75, Action::Poll)
    } else if stage == 1 && delay != 0 {
        unsafe { ARMED[pid] = Some(target); }
        (0, Action::Sleep(delay))
    } else {
        (0, Action::Poll)
    };
    f.x[0] = status as u64;
    append([1, pid as u64, job, phase, stage, target, entry, now, f.elr, f.x[0], 0, 0]);
    action
}

pub(super) fn transition(now: u64, old: (u64, u64), ready: u64, prior: Option<usize>,
                         next: Option<usize>, action: Action, reason: u64) {
    let after = counter();
    for pid in 0..3 {
        if ready & !old.0 & (1 << pid) != 0 { unsafe { ARMED[pid] = None; } }
    }
    if prior == next && old.0 == ready && reason != 62 && !matches!(action, Action::Exit) { return; }
    let cell = append([2, reason, now, old.1, after, old.0, ready,
        prior.map_or(NONE, |p| p as u64), next.map_or(NONE, |p| p as u64),
        matches!(action, Action::Exit) as u64, 0, 0]);
    if next.is_some() { unsafe { native_permit_user_return = cell; } }
}

pub(super) fn compare(requested: Option<u64>, before: u64) {
    // Sample all arm calls at the source; store only changes of requested target.
    // Consecutive identical writes cannot establish a different deadline.
    if unsafe { LAST_COMPARE } == Some(requested) { return; }
    unsafe { LAST_COMPARE = Some(requested); }
    let (cval, ctl): (u64, u64);
    unsafe { asm!("mrs {},cntp_cval_el0", "mrs {},cntp_ctl_el0", out(reg) cval,
                  out(reg) ctl, options(nostack)); }
    append([5, before, counter(), requested.is_some() as u64, requested.unwrap_or(0),
            cval, ctl, 0, 0, 0, 0, 0]);
}

pub(super) fn irq(iar: u32, entry: u64, idle: bool) {
    let (compare, ctl, pc, spsr): (u64, u64, u64, u64);
    unsafe { asm!("mrs {},cntp_cval_el0", "mrs {},cntp_ctl_el0", "mrs {},elr_el1", "mrs {},spsr_el1",
                  out(reg) compare, out(reg) ctl, out(reg) pc, out(reg) spsr, options(nostack)); }
    let cell = append([3, entry, counter(), u64::from(iar), compare, ctl, pc, spsr, idle as u64, 0, 0, 0]);
    unsafe { native_permit_irq_return = cell; }
}

pub(super) fn decision(pid: usize, op: u64, command: u64, entry: u64, before: u64, result: u64) {
    append([4, pid as u64, op, command, entry, before, counter(), result, 0, 0, 0, 0]);
}

pub(super) fn report() {
    let attempted = unsafe { ATTEMPTS };
    uart_write(b"NATIVE QUAL COUNTS ");
    for n in [unsafe { CLOCK_DOMAIN.1 }, attempted as u64, attempted.min(LIMIT) as u64,
              attempted.saturating_sub(LIMIT) as u64] { unsigned(n); uart_write(b" "); }
    uart_write(b"\n");
    for i in 0..attempted.min(LIMIT) {
        uart_write(b"NATIVE QUAL ROW "); unsigned(i as u64);
        for n in unsafe { ROWS[i] } { uart_write(b" "); unsigned(n); }
        uart_write(b"\n");
    }
}
