//! Native timer adapter for the shared weighted virtual-runtime policy.
//! CPU0 enters all these functions with exceptions masked.
use super::*;
use crate::vruntime_policy::{grant_expiry, Lease, VruntimeScheduler};

const LIMIT: usize = 4096;
const WIDTH: usize = 39;
static mut ROWS: [[u64; WIDTH]; LIMIT] = [[0; WIDTH]; LIMIT];
static mut USED: usize = 0;
static mut LOST: u64 = 0;
static mut CLOCKS: [[u64; 5]; 128] = [[0; 5]; 128];
static mut CLOCK_USED: usize = 0;

struct Runtime {
    policy: VruntimeScheduler<COUNT>,
    lease: Option<Lease>,
    start: Option<u64>,
    sleepers: [Option<u64>; COUNT],
    exited: [bool; COUNT],
    compare: Option<u64>,
    expiry: Option<u64>,
    pending: bool,
    pending_row: Option<usize>,
}
static mut STATE: Option<Runtime> = None;

fn state() -> &'static mut Runtime {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(11)) }
}

pub(super) fn owner() -> Option<usize> { state().lease.map(|l| l.task) }
pub(super) fn finished() -> bool { state().exited.iter().all(|&done| done) }

pub(super) fn start() -> usize {
    unsafe {
        STATE = Some(Runtime {
            policy: VruntimeScheduler::new(NATIVE_WEIGHTS).unwrap_or_else(|| fail(11)),
            lease: None, start: None, sleepers: [None; COUNT], exited: [false; COUNT],
            compare: None, expiry: None, pending: false, pending_row: None,
        });
    }
    boundary(counter(), 0, None, None).unwrap_or_else(|| fail(11))
}

fn reserve() -> Option<usize> {
    unsafe {
        if USED == LIMIT { LOST = LOST.saturating_add(1); None }
        else { let index = USED; USED += 1; Some(index) }
    }
}

pub(super) fn clock(pid: usize, sample: u64, result: i64, pc: u64) {
    let end = counter();
    unsafe {
        if CLOCK_USED == 128 { LOST = LOST.saturating_add(1); }
        else {
            CLOCKS[CLOCK_USED] = [pid as u64, sample, end, result as u64, pc];
            CLOCK_USED += 1;
        }
    }
}

pub(super) fn timer(now: u64, frame: Option<&NativeFrame>) -> Option<usize> {
    // Ignore a previously latched level if a syscall has changed the compare.
    if state().compare.is_some_and(|at| now >= at) {
        boundary(now, 4, None, frame)
    } else { owner() }
}

pub(super) fn boundary(now: u64, reason: u64, target: Option<u64>, frame: Option<&NativeFrame>) -> Option<usize> {
    let r = state();
    let old = r.lease.take();
    let old_start = r.start.take();
    let prior_compare = r.compare.take();
    let prior_expiry = r.expiry.take();
    let prior_wake = r.sleepers.iter().flatten().copied().min();
    arm_compare(None);
    let mut elapsed = 0;
    if let Some(lease) = old {
        let start = old_start.unwrap_or_else(|| fail(11));
        elapsed = now.checked_sub(start).unwrap_or_else(|| fail(11));
        let accepted = if elapsed == 0 { r.policy.retire_zero(lease) } else { r.policy.charge(lease, elapsed) };
        if !accepted { fail(11) }
        if reason == 2 || reason == 3 {
            if !r.policy.sleep(lease.task) { fail(11) }
            if reason == 3 { r.exited[lease.task] = true; } else { r.sleepers[lease.task] = target; }
        }
    }
    let mut due_mask = 0;
    let mut order = 0;
    while let Some((_, id)) = r.sleepers.iter().enumerate()
        .filter_map(|(id, at)| at.filter(|&at| at <= now).map(|at| (at, id))).min()
    {
        if !r.policy.wake(id) { fail(11) }
        r.sleepers[id] = None;
        due_mask |= 1 << id;
        order = order * 16 + id as u64 + 1;
    }
    let wake_sample = if due_mask != 0 { counter() } else { 0 };
    let mut ready = 0u64;
    for id in 0..COUNT {
        if r.policy.ready(id) == Some(true) { ready |= 1 << id; }
    }
    r.lease = if ready != 0 { Some(r.policy.dispatch().unwrap_or_else(|| fail(11))) } else { None };
    let next = r.lease.map(|lease| lease.task);
    r.pending = next.is_some();
    let earliest = r.sleepers.iter().flatten().copied().min();
    r.compare = earliest;
    if next.is_none() { arm_compare(earliest); }

    r.pending_row = reserve();
    if let Some(index) = r.pending_row {
        let row = unsafe { &mut *(&raw mut ROWS[index]) };
        let prior = old.map(|lease| lease.task);
        let restored = next.map(|id| {
            if prior == Some(id) {
                if let Some(f) = frame { return (f.elr, f.sp_el0); }
            }
            unsafe { (FRAMES[id].elr, FRAMES[id].sp_el0) }
        });
        row[..21].copy_from_slice(&[
            reason, now, prior.map_or(NO_PID, |id| id as u64), next.map_or(NO_PID, |id| id as u64),
            old.map_or(0, |l| l.sequence), old_start.unwrap_or(0), elapsed, target.unwrap_or(0),
            prior_compare.unwrap_or(0), prior_expiry.unwrap_or(0), prior_wake.unwrap_or(0),
            ready, due_mask, wake_sample, r.lease.map_or(0, |l| l.sequence), 0, earliest.unwrap_or(0),
            frame.map_or(0, |f| f.elr), frame.map_or(0, |f| f.sp_el0),
            restored.map_or(0, |f| f.0), restored.map_or(0, |f| f.1),
        ]);
        let (whole, rem, den) = r.policy.floor().parts();
        row[21..24].copy_from_slice(&[whole, rem, den]);
        for id in 0..COUNT {
            row[24 + id] = r.policy.service(id).unwrap_or_else(|| fail(11));
            let (whole, rem, den) = r.policy.runtime(id).unwrap_or_else(|| fail(11)).parts();
            let at = 27 + 3 * id;
            row[at..at + 3].copy_from_slice(&[whole, rem, den]);
        }
        row[36] = order;
        row[37] = prior.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 });
        row[38] = next.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 });
    }
    next
}

// Called with mappings active after selection, EOI and idle-stack unwinding,
// immediately before the assembly restores the selected EL0 register file.
#[unsafe(no_mangle)]
extern "C" fn native_weighted_return() {
    let r = state();
    if !r.pending { return; }
    let competing = (0..COUNT).filter(|&id| r.policy.ready(id) == Some(true)).count() > 1;
    let start = counter();
    let expiry = if competing {
        Some(grant_expiry(start, NATIVE_QUANTUM_TICKS).unwrap_or_else(|| fail(11)))
    } else { None };
    r.start = Some(start);
    r.expiry = expiry;
    r.compare = r.compare.into_iter().chain(expiry).min();
    arm_compare(r.compare);
    if let Some(index) = r.pending_row.take() {
        unsafe { ROWS[index][15] = start; ROWS[index][16] = r.compare.unwrap_or(0); }
    }
    r.pending = false;
}

fn print(label: &[u8], fields: &[u64]) {
    uart_write(label);
    for &v in fields { uart_write(b" "); unsigned(v); }
    uart_write(b"\n");
}

pub(super) fn dump() {
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
        for index in 0..USED { print(b"NATIVE WEIGHTED EVENT", &*(&raw const ROWS[index])); }
        for index in 0..CLOCK_USED { print(b"NATIVE WEIGHTED CLOCK", &*(&raw const CLOCKS[index])); }
        print(b"NATIVE WEIGHTED DROPPED", &[LOST]);
    }
}

pub(super) fn finish() -> ! {
    head(10, 1);
    head(1, 3);
    dump();
    unsafe {
        for (pid, status) in EXIT_STATUS.into_iter().enumerate() {
            if COUNT == 1 {
                uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
            } else { event(b"EXIT", pid, Some(i64::from(status))); }
        }
    }
    park()
}
