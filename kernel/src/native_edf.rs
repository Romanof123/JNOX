//! CPU0 EDF adapter for separately loaded finite jobs. Counter stamps are
//! physical ticks. Only the shared selector sees logical grid indices.
use super::*;
use crate::deadline_policy::{
    absolute, admit_edf, grid_quantum, DeadlinePolicy, DeadlineScheduler, NumericJob,
};

struct Runtime {
    policy: DeadlineScheduler<COUNT>,
    epoch: u64,
    quantum: u64,
    frequency: u64,
    releases: [u64; COUNT],
    deadlines: [u64; COUNT],
    released: [bool; COUNT],
    sleepers: [Option<u64>; COUNT],
    exits: [Option<u64>; COUNT],
    service: [u64; COUNT],
    running: Option<usize>,
    started: Option<u64>,
    needs_start: bool,
    compare: Option<u64>,
    grid: Option<u64>,
    row: Option<usize>,
}

const LIMIT: usize = 512;
const WIDTH: usize = 36;
static mut STATE: Option<Runtime> = None;
static mut ROWS: [[u64; WIDTH]; LIMIT] = [[0; WIDTH]; LIMIT];
static mut USED: usize = 0;
static mut LOST: u64 = 0;

fn state() -> &'static mut Runtime {
    // Accessed only by the masked boot/exception path; no borrow crosses idle.
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(12)) }
}

pub(super) fn owner() -> Option<usize> { state().running }
pub(super) fn finished() -> bool { state().exits.iter().all(Option::is_some) }

pub(super) fn start(hz: u64) -> usize {
    let jobs: [NumericJob; COUNT] = core::array::from_fn(|id| {
        let (release, deadline, cost) = NATIVE_EDF_JOBS[id];
        NumericJob { id, release, deadline, cost }
    });
    let q = grid_quantum(hz).unwrap_or_else(|_| fail(13));
    // Complete the model before choosing S. Check all physical endpoints
    // against the real epoch separately before installing the first compare.
    if let Err(reason) = admit_edf(&jobs, hz, 0) {
        print(b"NATIVE EDF REFUSED", &[reason as u64]);
        fail(14)
    }
    let policy = DeadlineScheduler::new(DeadlinePolicy::Edf, 0, 1).unwrap_or_else(|| fail(12));
    let origin = counter().checked_add(q).unwrap_or_else(|| fail(13));
    absolute(origin, q, 16).unwrap_or_else(|_| fail(13));
    let at = |k| absolute(origin, q, k).unwrap_or_else(|_| fail(13));
    let releases: [u64; COUNT] = core::array::from_fn(|id| at(jobs[id].release));
    let deadlines = core::array::from_fn(|id| at(jobs[id].deadline));
    let first = releases.iter().copied().min().unwrap_or_else(|| fail(12));
    unsafe {
        STATE = Some(Runtime {
            policy, epoch: origin, quantum: q, frequency: hz, releases, deadlines,
            released: [false; COUNT], sleepers: [None; COUNT], exits: [None; COUNT],
            service: [0; COUNT], running: None, started: None, needs_start: false,
            compare: Some(first), grid: None, row: None,
        });
        // The first lower-EL entry has not run yet. Install the idle vector
        // before admitting any process through the first real timer interrupt.
        asm!("msr vbar_el1, {}", "isb", in(reg) &raw const native_el0_vectors, options(nostack));
    }
    arm_compare(Some(first));
    if counter() >= origin {
        arm_compare(None);
        fail(15)
    }
    let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
    unsafe { native_sleep_switch(top) };
    owner().unwrap_or_else(|| fail(12))
}

pub(super) fn irq(now: u64, frame: Option<&NativeFrame>) -> Option<usize> {
    if state().compare.is_some_and(|at| now >= at) {
        boundary(now, 4, None, frame)
    } else { owner() }
}

pub(super) fn boundary(now: u64, reason: u64, sleep: Option<u64>, frame: Option<&NativeFrame>) -> Option<usize> {
    let r = state();
    let k = now.checked_sub(r.epoch).unwrap_or_else(|| fail(13)) / r.quantum;
    let prior = r.running;
    let old_start = r.started.take();
    let mut elapsed = 0;
    if let Some(id) = prior {
        elapsed = now.checked_sub(old_start.unwrap_or_else(|| fail(12))).unwrap_or_else(|| fail(13));
        r.service[id] = r.service[id].checked_add(elapsed).unwrap_or_else(|| fail(13));
    }
    let old_compare = r.compare;
    let old_grid = r.grid;
    let sampled = old_grid.is_some_and(|at| at <= now);
    let required = old_grid.or_else(|| (0..COUNT).filter(|&id| !r.released[id]).map(|id| r.releases[id]).min());
    let skipped = required.filter(|&at| at <= now).map_or(0, |at| (now - at) / r.quantum);
    r.policy.account(k).unwrap_or_else(|_| fail(12));
    if let Some(id) = prior {
        match reason {
            1 => r.policy.yield_running(k, id).unwrap_or_else(|_| fail(12)),
            2 => {
                r.policy.suspend(k, id).unwrap_or_else(|_| fail(12));
                r.sleepers[id] = sleep;
            }
            3 => {
                r.policy.complete(k, id).unwrap_or_else(|_| fail(12));
                r.exits[id] = Some(now);
            }
            _ => {}
        }
    }
    let mut release_mask = 0;
    let mut wake_mask = 0;
    let mut order = 0u64;
    // One merged calendar. Lexicographic order makes an initial release
    // precede a resume at equal physical time; slot order breaks the final tie.
    loop {
        let releases = (0..COUNT).filter(|&id| !r.released[id])
            .map(|id| (r.releases[id], false, id));
        let wakes = r.sleepers.iter().enumerate().filter_map(|(id, &at)| at.map(|t| (t, true, id)));
        let Some((_, wake, id)) = releases.chain(wakes).filter(|e| e.0 <= now).min() else { break };
        if wake {
            r.policy.resume(k, id).unwrap_or_else(|_| fail(12));
            r.sleepers[id] = None;
            wake_mask |= 1 << id;
            order = (order << 4) | (id as u64 + 5);
        } else {
            let (_, deadline, cost) = NATIVE_EDF_JOBS[id];
            r.policy.release(k, id, cost, deadline).unwrap_or_else(|_| fail(12));
            r.released[id] = true;
            release_mask |= 1 << id;
            order = (order << 4) | (id as u64 + 1);
        }
    }
    let next = if sampled { r.policy.decide(k) } else { r.policy.decide_unsampled(k) }
        .unwrap_or_else(|_| fail(12));
    r.running = next;
    r.needs_start = next.is_some();
    r.grid = next.map(|_| {
        let index = k.checked_add(1).unwrap_or_else(|| fail(13));
        absolute(r.epoch, r.quantum, index).unwrap_or_else(|_| fail(13))
    });
    r.compare = (0..COUNT).filter(|&id| !r.released[id]).map(|id| r.releases[id])
        .chain(r.sleepers.iter().flatten().copied()).chain(r.grid).min();
    arm_compare(r.compare);

    let index = unsafe { USED };
    if index == LIMIT {
        unsafe { LOST = LOST.saturating_add(1) };
        r.row = None;
        return next;
    }
    let row = unsafe { &mut *(&raw mut ROWS[index]) };
    let restored = next.map(|id| {
        if prior == Some(id) {
            if let Some(f) = frame { return (f.elr, f.sp_el0); }
        }
        unsafe { (FRAMES[id].elr, FRAMES[id].sp_el0) }
    });
    let mut available = 0u64;
    let mut sleeping = 0u64;
    let mut exited = 0u64;
    let mut overdue = 0u64;
    row[27..30].fill(NO_PID);
    for id in 0..COUNT {
        if r.exits[id].is_some() { exited |= 1 << id; }
        else if r.sleepers[id].is_some() { sleeping |= 1 << id; }
        else if r.released[id] { available |= 1 << id; }
        if r.released[id] && r.exits[id].is_none() && now > r.deadlines[id] { overdue |= 1 << id; }
        row[24 + id] = r.service[id];
        row[27 + id] = r.policy.remaining(id).unwrap_or(NO_PID);
        row[30 + id] = r.exits[id].unwrap_or(0);
    }
    row[..24].copy_from_slice(&[
        reason, now, k, prior.map_or(NO_PID, |id| id as u64), next.map_or(NO_PID, |id| id as u64),
        old_start.unwrap_or(0), elapsed, 0, old_compare.unwrap_or(0), r.compare.unwrap_or(0),
        sleep.unwrap_or(0), release_mask, wake_mask, order, available, sleeping, exited, overdue,
        old_grid.unwrap_or(0), skipped, frame.map_or(0, |f| f.elr), frame.map_or(0, |f| f.sp_el0),
        restored.map_or(0, |f| f.0), restored.map_or(0, |f| f.1),
    ]);
    row[33] = r.policy.ready().iter().fold(0, |v, &id| (v << 4) | (id as u64 + 1));
    row[34] = prior.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 });
    row[35] = next.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 });
    unsafe { USED += 1; }
    r.row = Some(index);
    next
}

#[unsafe(no_mangle)]
extern "C" fn native_edf_return() {
    let r = state();
    if r.needs_start {
        let stamp = counter();
        r.started = Some(stamp);
        if let Some(index) = r.row.take() { unsafe { ROWS[index][7] = stamp } }
        r.needs_start = false;
    }
}

fn print(label: &[u8], fields: &[u64]) {
    uart_write(label);
    for &n in fields { uart_write(b" "); unsigned(n); }
    uart_write(b"\n");
}

pub(super) fn finish() -> ! {
    head(10, 1);
    head(1, 3);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    let r = state();
    print(b"NATIVE EDF CONFIG", &[r.epoch, r.quantum, r.frequency]);
    for (id, (release, due, cost)) in NATIVE_EDF_JOBS.into_iter().enumerate() {
        print(b"NATIVE EDF JOB", &[id as u64, release, due, cost]);
    }
    unsafe {
        for i in 0..USED { print(b"NATIVE EDF EVENT", &*(&raw const ROWS[i])); }
        print(b"NATIVE EDF DROPPED", &[LOST]);
        for (pid, status) in EXIT_STATUS.into_iter().enumerate() {
            if COUNT == 1 {
                uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
            } else { event(b"EXIT", pid, Some(i64::from(status))); }
        }
    }
    park()
}
