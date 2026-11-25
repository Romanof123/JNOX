//! Operator-supplied recurring service on a finite sixteen-cell deadline calendar.
use super::*;
use crate::deadline_policy::{admit_native_recurring, recurring_ordinal};

const MASK: u64 = NATIVE_DEADLINE_SERVICE_MASK as u64;
const SVC: usize = 2;
const IDLE_ID: usize = 3;
const PARK: usize = 4;
const MAX_ROWS: usize = 64; // sixteen boundaries, sixteen service SVCs, two exits and terminal
type Row = [u64; 25];

struct Calendar {
    selector: DeadlineScheduler<2>,
    epoch: u64,
    q: u64,
    initial_arm: u64,
    sampled: Option<u64>,
    compare: Option<u64>,
    owner: usize,
    start: Option<u64>,
    exits: [bool; 3],
    receipts: u64,
    count: usize,
    dropped: u64,
    rows: [Row; MAX_ROWS],
}
static mut CALENDAR: Option<Calendar> = None;
fn calendar() -> &'static mut Calendar {
    unsafe { (&mut *(&raw mut CALENDAR)).as_mut().unwrap_or_else(|| fail(12)) }
}
fn cell(r: &Calendar, now: u64) -> Option<u64> { Some(now.checked_sub(r.epoch)? / r.q) }
fn end(r: &Calendar, k: u64) -> u64 { absolute(r.epoch, r.q, k).unwrap_or_else(|_| fail(13)) }

pub(in super::super) fn start(hz: u64) -> usize {
    let q = grid_quantum(hz).unwrap_or_else(|_| fail(13));
    let epoch = counter().checked_add(q).unwrap_or_else(|| fail(13));
    let rule = if NATIVE_DEADLINE_LLF { DeadlinePolicy::Llf } else { DeadlinePolicy::Edf };
    let jobs: [NumericJob; 2] = core::array::from_fn(|id| {
        let (release, deadline, cost) = NATIVE_EDF_JOBS[id];
        NumericJob { id, release, deadline, cost }
    });
    print(b"RECURRING CONFIG", &[1, epoch, q, hz, MASK, NATIVE_DEADLINE_SWITCH_CAP]);
    for j in &jobs { print(b"RECURRING JOB", &[j.id as u64, j.release, j.deadline, j.cost]); }
    if let Err(reason) = admit_native_recurring(rule, &jobs, hz, epoch, MASK, NATIVE_DEADLINE_SWITCH_CAP) {
        print(b"RECURRING REFUSED", &[reason as u64, 0, 0]);
        fail(14);
    }
    if COUNT != 3 { fail(12); }
    unsafe {
        CALENDAR = Some(Calendar {
            selector: DeadlineScheduler::new(rule, 0, 1).unwrap_or_else(|| fail(12)),
            epoch, q, initial_arm: 0, sampled: None, compare: Some(epoch), owner: IDLE_ID, start: None,
            exits: [false; 3], receipts: 0, count: 0, dropped: 0, rows: [[0; 25]; MAX_ROWS],
        });
        asm!("msr vbar_el1, {}", "isb", in(reg) &raw const native_el0_vectors, options(nostack));
    }
    let arm_stamp = counter();
    calendar().initial_arm = arm_stamp;
    if arm_stamp >= epoch { stop(1); }
    arm_compare(Some(epoch));
    idle();
    calendar().owner
}

fn idle() {
    let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
    unsafe { native_sleep_switch(top); }
}

fn begin(now: u64, kind: u64, op: u64, f: Option<&NativeFrame>) {
    let r = calendar();
    if r.count >= MAX_ROWS { r.dropped += 1; stop(2); }
    let k = cell(r, now).unwrap_or(NO_PID);
    let start = r.start.take();
    let elapsed = start.map_or(0, |t| now.checked_sub(t).unwrap_or_else(|| fail(13)));
    let i = r.count;
    r.count += 1;
    r.rows[i] = [i as u64, kind, now, k, r.owner as u64, PARK as u64, op,
        f.map_or(0, |f| f.elr), f.map_or(0, |f| f.sp_el0), 0, 0, f.map_or(0, |f| f.x[0]), 0,
        r.compare.unwrap_or(0), r.compare.unwrap_or(0), 0, 0, start.unwrap_or(0), elapsed,
        recurring_ordinal(MASK, k).unwrap_or(NO_PID), NO_PID, NO_PID, 0, 0, 0];
}
fn choose(next: usize, interrupted: Option<&NativeFrame>) {
    let r = calendar();
    let frame = if next >= IDLE_ID { None } else if next == r.owner && interrupted.is_some() {
        interrupted
    } else { Some(unsafe { &*(&raw const FRAMES[next]) }) };
    let row = &mut r.rows[r.count - 1];
    row[5] = next as u64;
    if let Some(f) = frame { row[9] = f.elr; row[10] = f.sp_el0; row[12] = f.x[0]; }
    row[20] = r.selector.remaining(0).unwrap_or(NO_PID);
    row[21] = r.selector.remaining(1).unwrap_or(NO_PID);
    row[22] = r.selector.ready().iter().fold(0, |bits, &id| bits << 4 | (id as u64 + 1));
    r.owner = next;
}

fn set_compare(k: u64) {
    let r = calendar();
    let target = end(r, k);
    let stamp = counter();
    if stamp >= target { stop(3); }
    arm_compare(Some(target));
    r.compare = Some(target);
    let row = &mut r.rows[r.count - 1];
    row[14] = target;
    row[15] = stamp;
}

pub(in super::super) fn returning() {
    let r = calendar();
    let now = counter();
    r.rows[r.count - 1][16] = now;
    if cell(r, now) != r.sampled || r.compare.is_none_or(|at| now >= at) { stop(4); }
    r.start = Some(now);
}
fn dispatch(next: usize, f: &mut NativeFrame) {
    if next == IDLE_ID {
        unsafe { FRAMES[CURRENT] = *f; }
        returning();
        idle();
        activate(calendar().owner, f);
    } else { activate(next, f); }
}

pub(in super::super) fn interrupt(now: u64, ptr: *mut NativeFrame, iar: u32) -> u64 {
    let irq = iar & 0x3ff;
    let frame = unsafe { ptr.as_mut() };
    begin(now, if irq == context::TIMER_IRQ { 1 } else { 3 }, irq as u64, frame.as_deref());
    if irq != context::TIMER_IRQ { stop(5); }
    let r = calendar();
    let k = cell(r, now).unwrap_or(NO_PID);
    if k != r.sampled.map_or(0, |v| v + 1) || k > 16
        || r.compare.is_none_or(|at| now < at) || r.owner == SVC
        || frame.is_some() != (r.owner != IDLE_ID) { stop(6); }
    r.sampled = Some(k);
    if done(r, k) { stop(0); }
    if k == 16 { stop(7); }
    r.selector.account(k).unwrap_or_else(|_| stop(8));
    for id in 0..2 {
        let (release, due, cost) = NATIVE_EDF_JOBS[id];
        if k == release { r.selector.release(k, id, cost, due).unwrap_or_else(|_| stop(8)); }
    }
    let next = if let Some(ord) = recurring_ordinal(MASK, k) {
        if ord != r.receipts || r.exits[SVC] { stop(9); }
        r.selector.withhold(k).unwrap_or_else(|_| stop(8));
        SVC
    } else { r.selector.decide(k).unwrap_or_else(|_| stop(8)).unwrap_or(IDLE_ID) };
    choose(next, frame.as_deref());
    set_compare(k + 1);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
    context::gic_write(context::GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)); }
    if let Some(f) = frame { dispatch(next, f); }
    else if next == IDLE_ID { returning(); }
    (next != IDLE_ID) as u64
}

fn done(r: &Calendar, k: u64) -> bool {
    r.exits.iter().all(|&v| v) && k > u64::from(63 - MASK.leading_zeros())
}

pub(in super::super) fn fault(now: u64, f: &NativeFrame) -> ! {
    begin(now, 4, f.esr, Some(f));
    stop(12)
}

pub(in super::super) fn syscall(now: u64, f: &mut NativeFrame) {
    let op = f.x[8];
    begin(now, 2, op, Some(f));
    let r = calendar();
    let k = cell(r, now).unwrap_or(NO_PID);
    if r.sampled != Some(k) || k >= 16 || r.owner != unsafe { CURRENT } { stop(10); }
    let owner = r.owner;
    if op == 1 && f.x[0] != f.x[0] as i32 as i64 as u64 { stop(11); }
    let next = if owner == SVC {
        if recurring_ordinal(MASK, k) != Some(r.receipts) { stop(9); }
        let final_activation = r.receipts + 1 == u64::from(MASK.count_ones());
        if op != (if final_activation { 1 } else { 18 }) { stop(11); }
        r.receipts += 1;
        if !final_activation { f.x[0] = r.receipts; }
        IDLE_ID
    } else {
        if owner >= 2 || op != 1 || recurring_ordinal(MASK, k).is_some() { stop(11); }
        r.selector.complete(k, owner).unwrap_or_else(|_| stop(8));
        r.selector.decide_unsampled(k).unwrap_or_else(|_| stop(8)).unwrap_or(IDLE_ID)
    };
    if op == 1 {
        unsafe { EXIT_STATUS[owner] = f.x[0] as i32; }
        head(9, f.x[0]);
        r.exits[owner] = true;
    }
    if done(r, k) { stop(0); }
    choose(next, Some(f));
    dispatch(next, f);
}

fn stop(failure: u64) -> ! {
    arm_compare(None);
    let r = calendar();
    if r.count != 0 {
        let row = &mut r.rows[r.count - 1];
        row[5] = PARK as u64;
        row[14] = 0;
        row[23] = u64::from(failure == 0);
        row[24] = failure;
    }
    print(b"RECURRING ARM", &[r.initial_arm, r.epoch]);
    for row in &r.rows[..r.count] { print(b"RECURRING EVENT", row); }
    print(b"RECURRING END", &[failure, r.count as u64, r.dropped]);
    if failure != 0 { fail(15); }
    for id in 0..3 { event(b"EXIT", id, Some(i64::from(unsafe { EXIT_STATUS[id] }))); }
    head(10, 1);
    head(1, 3);
    park()
}
