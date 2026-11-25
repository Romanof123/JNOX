//! Measured strict-class and feedback dispatch on native CPU0.
use super::*;
use crate::multilevel_policy::Multilevel;

const LIMIT: usize = 512;
const WIDTH: usize = 56;
static mut JOURNAL: [[u64; WIDTH]; LIMIT] = [[0; WIDTH]; LIMIT];
static mut USED: usize = 0;
static mut LOST: u64 = 0;
static mut STATE: Option<Runtime> = None;

#[derive(Default, Clone, Copy)]
struct Grant {
    start: u64,
    ticks: u64,
    expiry: Option<u64>,
    boost_due: Option<u64>,
}

struct Runtime {
    policy: Multilevel<COUNT>,
    hz: u64,
    owner: Option<usize>,
    grant: Grant,
    sleepers: [Option<u64>; COUNT],
    exited: u64,
    service: [u64; COUNT],
    total: u64,
    next_boost: Option<u64>,
    compare: Option<u64>,
    needs_start: bool,
    pending_row: Option<usize>,
}

fn state() -> &'static mut Runtime {
    // Boot and both exception paths are masked on the same CPU.
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(18)) }
}

pub(super) fn owner() -> Option<usize> { state().owner }
pub(super) fn finished() -> bool { state().exited == (1 << COUNT) - 1 }

fn order(ids: &[usize]) -> u64 {
    ids.iter().fold(0, |n, &id| n * 16 + id as u64 + 1)
}

impl Runtime {
    fn wake_time(&self) -> Option<u64> { self.sleepers.iter().flatten().copied().min() }

    fn admit_wakes(&mut self, now: u64) -> u64 {
        let mut order = 0;
        while let Some((_, id)) = self.sleepers.iter().enumerate()
            .filter_map(|(id, &at)| at.filter(|&at| at <= now).map(|at| (at, id))).min()
        {
            if !self.policy.wake(id) { fail(18) }
            self.sleepers[id] = None;
            order = order * 16 + id as u64 + 1;
        }
        order
    }

    fn ranks(&self) -> ([u64; COUNT], [u64; COUNT]) {
        let rank = core::array::from_fn(|id| self.policy.priority(id).map(u64::from).unwrap_or_else(|| fail(18)));
        let left = core::array::from_fn(|id| self.policy.remaining(id).unwrap_or_else(|| fail(18)));
        (rank, left)
    }

    fn select(&mut self) -> Option<usize> {
        self.owner = self.policy.dispatch();
        self.grant = Grant::default();
        self.needs_start = self.owner.is_some();
        if let Some(id) = self.owner {
            self.grant.ticks = self.policy.remaining(id).unwrap_or_else(|| fail(18));
            // Re-arm after the selected address space and frame are in place.
            self.compare = None;
        } else { self.compare = self.wake_time(); }
        arm_compare(self.compare);
        self.owner
    }
}

pub(super) fn start(hz: u64) -> usize {
    let (policy, next_boost) = match NATIVE_POLICY {
        "mlq" => (Multilevel::fixed(NATIVE_CLASSES, NATIVE_QUANTUM_TICKS), None),
        "mlfq" if NATIVE_BOOST_TICKS != 0 =>
            (Multilevel::feedback(NATIVE_ALLOTMENT_TICKS), Some(NATIVE_BOOST_TICKS)),
        _ => fail(18),
    };
    let mut policy = policy.unwrap_or_else(|| fail(18));
    for id in 0..COUNT {
        if !policy.wake(id) { fail(18) }
    }
    unsafe {
        STATE = Some(Runtime {
            policy, hz, owner: None, grant: Grant::default(), sleepers: [None; COUNT],
            exited: 0, service: [0; COUNT], total: 0, next_boost, compare: None,
            needs_start: false, pending_row: None,
        });
    }
    boundary(counter(), 0, None, None).unwrap_or_else(|| fail(18))
}

pub(super) fn irq(now: u64, frame: Option<&NativeFrame>) -> Option<usize> {
    if state().compare.is_some_and(|at| now >= at) { boundary(now, 4, None, frame) } else { owner() }
}

pub(super) fn boundary(now: u64, reason: u64, sleep: Option<u64>, frame: Option<&NativeFrame>) -> Option<usize> {
    let r = state();
    let prior = r.owner;
    let grant = r.grant;
    let old_wake = r.wake_time();
    let old_compare = r.compare;
    let index = unsafe { USED };
    let capture = index < LIMIT;
    let old_queue = if capture { order(r.policy.queued()) } else { 0 };
    let before = capture.then(|| r.ranks());
    let mut elapsed = 0;
    if let Some(id) = prior {
        elapsed = now.checked_sub(grant.start).unwrap_or_else(|| fail(13));
        r.total = r.total.checked_add(elapsed).unwrap_or_else(|| fail(13));
        r.service[id] = r.service[id].checked_add(elapsed).unwrap_or_else(|| fail(13));
    }
    let wakes = r.admit_wakes(now);
    if let Some(id) = prior {
        let blocked = matches!(reason, 2 | 3);
        if reason == 2 { r.sleepers[id] = Some(sleep.unwrap_or_else(|| fail(18))); }
        if reason == 3 { r.exited |= 1 << id; }
        if !r.policy.close_interval(id, elapsed, blocked) { fail(18) }
    }
    let charged = capture.then(|| r.ranks());
    let charged_queue = if capture { order(r.policy.queued()) } else { 0 };
    let mut boosts = 0;
    if let Some(threshold) = r.next_boost {
        if r.total >= threshold {
            boosts = 1 + (r.total - threshold).checked_div(NATIVE_BOOST_TICKS).unwrap_or_else(|| fail(13));
            r.next_boost = Some(boosts.checked_mul(NATIVE_BOOST_TICKS)
                .and_then(|delta| threshold.checked_add(delta)).unwrap_or_else(|| fail(13)));
            if !r.policy.boost() { fail(18) }
        }
    }
    let next = r.select();
    if !capture {
        unsafe { LOST = LOST.saturating_add(1) };
        r.pending_row = None;
        return next;
    }
    let restored = next.map(|id| {
        if Some(id) == prior {
            if let Some(f) = frame { return (f.elr, f.sp_el0); }
        }
        unsafe { (FRAMES[id].elr, FRAMES[id].sp_el0) }
    });
    let row = unsafe { &mut *(&raw mut JOURNAL[index]) };
    row[..32].copy_from_slice(&[
        reason, now, prior.map_or(NO_PID, |id| id as u64), next.map_or(NO_PID, |id| id as u64),
        grant.start, elapsed, grant.ticks, grant.expiry.unwrap_or(0), grant.boost_due.unwrap_or(0),
        old_wake.unwrap_or(0), old_compare.unwrap_or(0), 0, r.grant.ticks, 0, 0,
        r.wake_time().unwrap_or(0), r.compare.unwrap_or(0), wakes, r.total, boosts,
        r.next_boost.unwrap_or(0), old_queue, charged_queue, order(r.policy.queued()),
        frame.map_or(0, |f| f.elr), frame.map_or(0, |f| f.sp_el0), restored.map_or(0, |f| f.0),
        restored.map_or(0, |f| f.1), prior.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 }),
        next.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 }), sleep.unwrap_or(0), r.exited,
    ]);
    let (old_rank, old_left) = before.unwrap();
    let (charged_rank, charged_left) = charged.unwrap();
    let (rank, left) = r.ranks();
    for id in 0..COUNT {
        row[32+id] = r.service[id]; row[35+id] = r.sleepers[id].unwrap_or(0);
        row[38+id] = old_rank[id]; row[41+id] = old_left[id];
        row[44+id] = charged_rank[id]; row[47+id] = charged_left[id];
        row[50+id] = rank[id]; row[53+id] = left[id];
    }
    unsafe { USED += 1 };
    r.pending_row = Some(index);
    next
}

#[unsafe(no_mangle)]
extern "C" fn native_multilevel_return() {
    let r = state();
    if !r.needs_start { return; }
    let wake = r.wake_time();
    let start = counter();
    let expiry = start.checked_add(r.grant.ticks).unwrap_or_else(|| fail(13));
    let boost_due = r.next_boost.map(|threshold| threshold.checked_sub(r.total)
        .and_then(|ticks| start.checked_add(ticks)).unwrap_or_else(|| fail(13)));
    r.grant.start = start;
    r.grant.expiry = Some(expiry);
    r.grant.boost_due = boost_due;
    r.compare = Some(boost_due.into_iter().chain(wake).fold(expiry, u64::min));
    arm_compare(r.compare);
    if let Some(index) = r.pending_row.take() {
        unsafe {
            JOURNAL[index][11] = start; JOURNAL[index][13] = expiry;
            JOURNAL[index][14] = boost_due.unwrap_or(0); JOURNAL[index][16] = r.compare.unwrap_or(0);
        }
    }
    r.needs_start = false;
}

fn print(label: &[u8], fields: &[u64]) {
    uart_write(if NATIVE_POLICY == "mlq" { b"NATIVE MLQ " } else { b"NATIVE MLFQ " });
    uart_write(label);
    for &v in fields { uart_write(b" "); unsigned(v); }
    uart_write(b"\n");
}

pub(super) fn finish() -> ! {
    head(10, 1); head(1, 3);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    let r = state();
    let [top, middle, bottom] = NATIVE_ALLOTMENT_TICKS;
    print(b"CONFIG", &[r.hz, NATIVE_QUANTUM_TICKS, top, middle, bottom, NATIVE_BOOST_TICKS]);
    for (id, class) in NATIVE_CLASSES.into_iter().enumerate() { print(b"JOB", &[id as u64, u64::from(class)]); }
    unsafe {
        for i in 0..USED { print(b"EVENT", &*(&raw const JOURNAL[i])); }
        print(b"DROPPED", &[LOST]);
        print(b"SERVICE", &r.service);
        for (pid,status) in EXIT_STATUS.into_iter().enumerate() {
            if COUNT == 1 {
                uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
            } else { event(b"EXIT", pid, Some(i64::from(status))); }
        }
    }
    park()
}
