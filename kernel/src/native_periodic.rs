//! Native CPU0 adapter. Calendar state outlives every activation of a slot.
use super::*;
use crate::dispatch_policy::DispatchPolicy;
use crate::native_periodic_policy::{Action, Calendar, Error};

struct Runtime {
    calendar: Calendar<COUNT>,
    epoch: u64,
    hz: u64,
    service: [u64; COUNT],
    exits: [u64; COUNT],
    compare: Option<u64>,
    started: Option<u64>,
    needs_start: bool,
    row: Option<usize>,
}

const LIMIT: usize = 512;
const WIDTH: usize = 51;
static mut STATE: Option<Runtime> = None;
static mut ROWS: [[u64; WIDTH]; LIMIT] = [[0; WIDTH]; LIMIT];
static mut USED: usize = 0;
static mut LOST: u64 = 0;
static mut REJECTED: [[u64; 4]; 64] = [[0; 4]; 64];
static mut REJECTIONS: usize = 0;

fn state() -> &'static mut Runtime {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(16)) }
}
pub(super) fn owner() -> Option<usize> { state().calendar.owner() }
pub(super) fn finished() -> bool { state().exits.iter().all(|&t| t != 0) }
pub(super) fn due(now: u64) -> bool { state().compare.is_some_and(|at| now >= at) }

fn print(label: &[u8], fields: &[u64]) {
    uart_write(label);
    for &value in fields { uart_write(b" "); unsigned(value); }
    uart_write(b"\n");
}

fn refused(error: Error, now: u64) -> ! {
    print(b"NATIVE PERIODIC ERROR", &[error as u64, now]);
    fail(16)
}

// The signed hardware comparator cannot express a forward distance larger
// than i64::MAX. An intermediate interrupt covers longer absolute calendars.
fn physical_compare(now: u64, target: Option<u64>) -> Option<u64> {
    target.map(|at| at.min(now.saturating_add(i64::MAX as u64)))
}

pub(super) fn start(hz: u64) -> usize {
    let mode = match NATIVE_POLICY {
        "rm" => DispatchPolicy::RateMonotonic,
        "dm" => DispatchPolicy::DeadlineMonotonic,
        _ => fail(16),
    };
    let tuples = NATIVE_PERIODIC.map(|(phase, period, deadline)| [phase, period, deadline]);
    let now = counter();
    let epoch = now.checked_add((hz / 10).max(1)).unwrap_or_else(|| fail(13));
    let calendar = Calendar::new(mode, tuples, hz, epoch).unwrap_or_else(|e| refused(e, now));
    let compare = physical_compare(now, calendar.next_compare());
    unsafe {
        STATE = Some(Runtime {
            calendar, epoch, hz, service: [0; COUNT], exits: [0; COUNT],
            compare, started: None, needs_start: false, row: None,
        });
        asm!("msr vbar_el1, {}", "isb", in(reg) &raw const native_el0_vectors, options(nostack));
    }
    arm_compare(compare);
    if counter() >= epoch { arm_compare(None); fail(15) }
    let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
    unsafe { native_sleep_switch(top) };
    owner().unwrap_or_else(|| fail(16))
}

pub(super) fn irq(now: u64, frame: Option<&NativeFrame>) -> Option<usize> {
    if due(now) { boundary(now, 4, None, frame) } else { owner() }
}

pub(super) fn rejected(now: u64, pid: usize, service: u64, result: i64) {
    unsafe {
        if REJECTIONS == 64 { LOST = LOST.saturating_add(1); }
        else {
            REJECTED[REJECTIONS] = [now, pid as u64, service, result.unsigned_abs()];
            REJECTIONS += 1;
        }
    }
}

pub(super) fn boundary(now: u64, reason: u64, sleep: Option<u64>, frame: Option<&NativeFrame>) -> Option<usize> {
    let r = state();
    let prior = r.calendar.owner();
    let before = prior.and_then(|id| r.calendar.slot(id));
    let old_compare = r.compare;
    let started = r.started.take();
    let mut elapsed = 0;
    if let Some(id) = prior {
        elapsed = now.checked_sub(started.unwrap_or_else(|| fail(16))).unwrap_or_else(|| fail(13));
        r.service[id] = r.service[id].checked_add(elapsed).unwrap_or_else(|| fail(13));
    }
    let action = match reason {
        1 => Action::Yield,
        2 => Action::Sleep(sleep.unwrap_or_else(|| fail(16))),
        3 => Action::Exit,
        4 | 5 => Action::Poll,
        9 => Action::WaitNext,
        _ => fail(16),
    };
    let next = r.calendar.step(now, action).unwrap_or_else(|e| refused(e, now));
    if reason == 3 { r.exits[prior.unwrap_or_else(|| fail(16))] = now; }
    r.needs_start = next.is_some();
    r.compare = physical_compare(now, r.calendar.next_compare());
    arm_compare(r.compare);
    let index = unsafe { USED };
    if index == LIMIT {
        unsafe { LOST = LOST.saturating_add(1) };
        r.row = None;
        return next;
    }
    let row = unsafe { &mut *(&raw mut ROWS[index]) };
    let (generation, release, deadline) = before.map_or((NO_PID, 0, 0), |slot| {
        let delta = slot.generation.checked_mul(slot.period).unwrap_or_else(|| fail(13));
        let release = slot.first_release.checked_add(delta).unwrap_or_else(|| fail(13));
        let deadline = release.checked_add(slot.relative_deadline).unwrap_or_else(|| fail(13));
        (slot.generation, release, deadline)
    });
    let restored = next.map(|id| {
        if prior == Some(id) {
            if let Some(f) = frame { return (f.elr, f.sp_el0); }
        }
        unsafe { (FRAMES[id].elr, FRAMES[id].sp_el0) }
    });
    let mut suspended = 0;
    let mut exited = 0;
    let mut ready_mask = next.map_or(0, |id| 1u64 << id);
    let mut ready_order = 0;
    for &id in r.calendar.ready() {
        ready_mask |= 1 << id;
        ready_order = (ready_order << 4) | (id as u64 + 1);
    }
    for id in 0..COUNT {
        let slot = r.calendar.slot(id).unwrap_or_else(|| fail(16));
        if slot.sleep_until.is_some() { suspended |= 1 << id; }
        if slot.exited { exited |= 1 << id; }
        let at = 23 + id * 9;
        row[at..at + 9].copy_from_slice(&[
            slot.generation, slot.released, slot.missed, slot.miss_cursor, slot.overlaps,
            slot.cancelled, r.service[id], slot.sleep_until.unwrap_or(0), r.exits[id],
        ]);
    }
    row[..23].copy_from_slice(&[
        reason, now, prior.map_or(NO_PID, |id| id as u64), next.map_or(NO_PID, |id| id as u64),
        started.unwrap_or(0), elapsed, 0, old_compare.unwrap_or(0), r.compare.unwrap_or(0),
        sleep.unwrap_or(0), generation, release, deadline, ready_order, suspended, exited,
        frame.map_or(0, |f| f.elr), frame.map_or(0, |f| f.sp_el0),
        restored.map_or(0, |f| f.0), restored.map_or(0, |f| f.1),
        prior.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 }),
        next.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 }),
        if reason == 3 { frame.map_or(0, |f| f.x[0]) } else { 0 },
    ]);
    row[50] = ready_mask;
    unsafe { USED += 1; }
    r.row = Some(index);
    next
}

// Called after the chosen mapping is active and idle/GIC handling is complete.
#[unsafe(no_mangle)]
extern "C" fn native_periodic_return() {
    let r = state();
    if r.needs_start {
        let stamp = counter();
        r.started = Some(stamp);
        if let Some(index) = r.row.take() { unsafe { ROWS[index][6] = stamp; } }
        r.needs_start = false;
    }
}

pub(super) fn finish() -> ! {
    head(10, 1); head(1, 3);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    let r = state();
    print(b"NATIVE PERIODIC CONFIG", &[r.epoch, r.hz, u64::from(NATIVE_POLICY == "dm")]);
    for (id, (phase, period, deadline)) in NATIVE_PERIODIC.into_iter().enumerate() {
        let slot = r.calendar.slot(id).unwrap_or_else(|| fail(16));
        print(b"NATIVE PERIODIC JOB", &[id as u64, phase, period, deadline,
            slot.first_release, slot.period, slot.relative_deadline]);
    }
    unsafe {
        for i in 0..USED { print(b"NATIVE PERIODIC EVENT", &*(&raw const ROWS[i])); }
        for i in 0..REJECTIONS { print(b"NATIVE PERIODIC REJECT", &*(&raw const REJECTED[i])); }
        print(b"NATIVE PERIODIC DROPPED", &[LOST]);
        for (pid, status) in EXIT_STATUS.into_iter().enumerate() {
            if COUNT == 1 {
                uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
            } else { event(b"EXIT", pid, Some(i64::from(status))); }
        }
    }
    park()
}
