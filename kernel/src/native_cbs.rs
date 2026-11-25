//! The physical CPU0 grant adapter. Entitlement comes from NativeCbs; all
//! service and guard observations below are physical counter samples.
use super::*;
use crate::temporal_policy::NativeCbs;

#[derive(Clone, Copy, Default)]
struct Grant {
    slot: u64, pid: Option<usize>, opening: u64, end: u64, cutoff: u64,
    entry: u64, selected: u64, lower: u64, upper: u64, witness: u64,
    pc: u64, sp: u64,
}
#[derive(Clone, Copy)]
struct Sleep { target: u64, opening: u64, observed: bool }

#[derive(Default)]
struct Startup {
    prepared_from: u64, frozen_at: u64, armed_at: u64, return_check: u64,
    witnessed: bool,
}

struct Runtime {
    startup: Startup,
    policy: NativeCbs<COUNT>,
    epoch: u64, hz: u64, span: u64, guard: u64,
    sleepers: [Option<Sleep>; COUNT], compare: Option<u64>, grant: Option<Grant>,
    wall: [u64; COUNT],
}

const LIMIT: usize = 1024;
static mut STATE: Option<Runtime> = None;
static mut ROWS: [[u64; 36]; LIMIT] = [[0; 36]; LIMIT];
static mut USED: usize = 0;
static mut LOST: u64 = 0;

fn state() -> &'static mut Runtime {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(17)) }
}
pub(super) fn owner() -> Option<usize> { state().policy.owner() }
pub(super) fn finished() -> bool { (0..COUNT).all(|id| state().policy.exited(id)) }

fn print(label: &[u8], fields: &[u64]) {
    uart_write(label);
    for &value in fields { uart_write(b" "); unsigned(value); }
    uart_write(b"\n");
}

impl Runtime {
    fn opening(&self, t: u64) -> u64 {
        t.checked_mul(self.span).and_then(|offset| self.epoch.checked_add(offset))
            .unwrap_or_else(|| fail(13))
    }

    fn next_opening(&self) -> Option<u64> {
        self.policy.next_slot().into_iter()
            .chain(self.sleepers.iter().flatten().map(|s| s.opening)).min()
    }

    fn arm(&mut self) {
        let primary = if let Some(g) = self.grant { Some(g.cutoff) }
            else { self.next_opening().map(|t| self.opening(t)) };
        self.compare = primary.into_iter().chain(self.sleepers.iter().flatten()
            .filter(|s| !s.observed).map(|s| s.target)).min();
        arm_compare(self.compare);
    }

    fn record(&self, kind: u64, sample: u64, g: Grant, prior: Option<u64>, target: u64,
              frame: Option<&NativeFrame>, wall: u64, inhibit: u64, lost_openings: u64) {
        let index = unsafe { USED };
        if index == LIMIT {
            unsafe { LOST = LOST.saturating_add(1) };
            return;
        }
        let row = unsafe { &mut *(&raw mut ROWS[index]) };
        row[..21].copy_from_slice(&[
            kind, sample, g.slot, g.pid.map_or(NO_PID, |id| id as u64), g.opening, g.end, g.cutoff,
            g.entry, g.selected, g.lower, g.upper, g.witness, sample, wall,
            if g.end == 0 { 0 } else { sample.saturating_sub(g.end) },
            prior.unwrap_or(0), self.compare.unwrap_or(0), target, frame.map_or(0, |f| f.elr),
            frame.map_or(0, |f| f.sp_el0), g.pid.map_or(0, |id| unsafe { (&raw const FRAMES[id]).addr() as u64 }),
        ]);
        for id in 0..COUNT {
            if self.policy.active(id) { row[21] |= 1 << id; }
            if self.policy.exited(id) { row[22] |= 1 << id; }
            let (q,d) = self.policy.allocation(id).unwrap_or_else(|| fail(17));
            row[23 + 2 * id] = u64::from(q);
            row[24 + 2 * id] = d;
            row[29 + id] = self.wall[id];
        }
        row[32..].copy_from_slice(&[g.pc, g.sp, inhibit, lost_openings]);
        unsafe { USED += 1; }
    }

    fn halt(&mut self, sample: u64, g: Grant, frame: Option<&NativeFrame>, code: u64) -> ! {
        let prior = self.compare.take();
        arm_compare(None);
        let elapsed = if let Some(id) = g.pid.filter(|_| g.lower != 0) {
            let n = sample.checked_sub(g.entry).unwrap_or_else(|| fail(13));
            self.wall[id] = self.wall[id].checked_add(n).unwrap_or_else(|| fail(13));
            n
        } else { 0 };
        let crossed = if g.end != 0 && sample >= g.end { 1 + (sample - g.end) / self.span } else { 0 };
        let slots = crossed + u64::from(code == 1 || code == 2);
        self.record(7, sample, g, prior, 0, frame, elapsed, code, slots);
        print(b"NATIVE CBS INHIBIT", &[code, sample, g.slot, slots, g.pid.map_or(NO_PID, |id| id as u64),
            g.opening, g.end, g.cutoff, g.entry, g.selected, g.lower, g.upper, elapsed,
            sample.saturating_sub(g.end), frame.map_or(0, |f| f.elr)]);
        dump(self);
        park()
    }

    fn observe_wakes(&mut self, now: u64) {
        for id in 0..COUNT {
            if let Some(mut sleep) = self.sleepers[id] {
                if !sleep.observed && sleep.target <= now {
                    sleep.observed = true;
                    self.sleepers[id] = Some(sleep);
                    let prior = self.compare;
                    self.record(5, now, Grant { pid: Some(id), slot: sleep.opening, ..Grant::default() },
                                prior, sleep.target, None, 0, 0, 0);
                }
            }
        }
    }

    fn open(&mut self, t: u64, now: u64, prior: Option<u64>) -> Option<usize> {
        let opening = self.opening(t);
        let end = opening.checked_add(self.span).unwrap_or_else(|| fail(13));
        let mut g = Grant { slot: t, opening, end, cutoff: end - self.guard, entry: now, ..Grant::default() };
        if now - opening > self.guard { self.halt(now, g, None, 1) }
        for id in 0..COUNT {
            if let Some(sleep) = self.sleepers[id] {
                if sleep.opening == t {
                    self.policy.wake(id, t).unwrap_or_else(|_| fail(17));
                    self.sleepers[id] = None;
                    self.record(6, now, Grant {pid: Some(id), ..g}, prior, sleep.target, None, 0, 0, 0);
                }
            }
        }
        g.pid = self.policy.open(t).unwrap_or_else(|_| fail(17));
        g.selected = counter();
        if let Some(id) = g.pid {
            unsafe { g.pc = FRAMES[id].elr; g.sp = FRAMES[id].sp_el0; }
            self.grant = Some(g);
        }
        self.arm();
        self.record(0, now, g, prior, 0, None, 0, 0, 0);
        g.pid
    }
}

pub(super) fn start(hz: u64) -> usize {
    if hz < 10_000 || hz % 1000 != 0 { fail(13) }
    let prepared_from = counter();
    let mut policy = NativeCbs::new(NATIVE_RESERVATIONS).unwrap_or_else(|error| {
        print(b"NATIVE CBS REFUSED", &[error as u64]); fail(17)
    });
    let first = policy.open(0).unwrap_or_else(|_| fail(17)).unwrap_or_else(|| fail(17));
    let selected = counter();
    let (pc, sp) = unsafe { (FRAMES[first].elr, FRAMES[first].sp_el0) };
    let grant = Grant { pid: Some(first), selected, pc, sp, ..Grant::default() };
    unsafe {
        STATE = Some(Runtime { policy, epoch: 0, hz, span: hz / 20, guard: hz / 1000,
            startup: Startup { prepared_from, ..Startup::default() },
            sleepers: [None; COUNT], compare: None, grant: Some(grant), wall: [0; COUNT] });
    }
    // The ordinary first-entry path switches this owner's mapping, sets
    // CURRENT, and installs the vectors, ELR, SP and PSTATE before its hook.
    first
}

pub(super) fn witness(now: u64, f: &NativeFrame, service: u64) {
    let r = state();
    let initial = r.grant.unwrap_or_else(|| fail(17));
    if initial.slot == 0 && !r.startup.witnessed {
        // Defer formatting the startup opening row until after the entry
        // sample. The opening itself had no observed EL0 return yet.
        r.record(0, initial.entry, Grant { lower: 0, ..initial }, None, 0, None, 0, 0, 0);
    }
    let g = r.grant.as_mut().unwrap_or_else(|| fail(17));
    if g.pid != Some(unsafe { CURRENT }) || g.lower == 0 { fail(17) }
    if g.upper == 0 {
        g.upper = now;
        g.witness = service;
        if g.slot == 0 { r.startup.witnessed = true; }
    }
    let grant = *g;
    if now > grant.end { r.halt(now, grant, Some(f), 3) }
}

pub(super) fn irq(now: u64, frame: Option<&NativeFrame>) -> Option<usize> {
    let r = state();
    let prior = r.compare;
    if !prior.is_some_and(|at| now >= at) { return r.policy.owner() }
    r.observe_wakes(now);
    if let Some(g) = r.grant {
        if now >= g.cutoff { return boundary(now, 4, None, frame); }
    } else if let Some(t) = r.next_opening() {
        if r.opening(t) <= now { return r.open(t, now, prior); }
    }
    r.arm();
    r.policy.owner()
}

pub(super) fn boundary(now: u64, reason: u64, target: Option<u64>, frame: Option<&NativeFrame>) -> Option<usize> {
    let r = state();
    let prior = r.compare;
    let g = r.grant.take().unwrap_or_else(|| fail(17));
    let id = g.pid.unwrap_or_else(|| fail(17));
    let result = match reason {
        1 => r.policy.yield_grant(id),
        2 => {
            let target = target.unwrap_or_else(|| fail(17));
            let t = target.checked_sub(r.epoch).unwrap_or_else(|| fail(13)).div_ceil(r.span);
            r.sleepers[id] = Some(Sleep { target, opening: t, observed: false });
            r.policy.sleep(id)
        }
        3 => r.policy.exit(id),
        4 => r.policy.cutoff(id),
        _ => fail(17),
    };
    result.unwrap_or_else(|_| fail(17));
    let elapsed = now.checked_sub(g.entry).unwrap_or_else(|| fail(13));
    r.wall[id] = r.wall[id].checked_add(elapsed).unwrap_or_else(|| fail(13));
    r.arm();
    r.record(reason, now, g, prior, target.unwrap_or(0), frame, elapsed, 0, 0);
    None
}

#[unsafe(no_mangle)]
extern "C" fn native_cbs_return() {
    let r = state();
    if let Some(mut g) = r.grant {
        if g.lower == 0 {
            if g.slot == 0 {
                if g.pid != Some(unsafe { CURRENT }) || r.epoch != 0 { fail(17) }
                let frozen_at = counter();
                r.startup.frozen_at = frozen_at;
                r.epoch = frozen_at.checked_add(r.hz / 10_000).unwrap_or_else(|| fail(13));
                for (_, p) in NATIVE_RESERVATIONS { r.opening(u64::from(p)); }
                g.opening = r.epoch;
                g.end = r.opening(1);
                g.cutoff = g.end - r.guard;
                r.compare = Some(g.cutoff);
                arm_compare(r.compare);
                r.startup.armed_at = counter();
                let mut entry = r.startup.armed_at;
                while entry < g.opening { entry = counter(); }
                g.entry = entry;
                r.grant = Some(g);
                if entry - g.opening > r.guard {
                    r.startup.return_check = entry;
                    r.record(0, entry, g, None, 0, None, 0, 0, 0);
                    r.halt(entry, g, None, 1)
                }
            }
            let now = counter();
            if g.slot == 0 { r.startup.return_check = now; }
            if now - g.opening > r.guard {
                if g.slot == 0 { r.record(0, g.entry, g, None, 0, None, 0, 0, 0); }
                r.halt(now, g, None, 2)
            }
            g.lower = now;
            r.grant = Some(g);
        }
    }
}

fn dump(r: &Runtime) {
    // Preparation start, selection is in the opening row, epoch freeze,
    // post-arm counter, pre-return guard sample, and verified EL0 launches.
    print(b"NATIVE CBS STARTUP", &[r.startup.prepared_from, r.startup.frozen_at,
        r.startup.armed_at, r.startup.return_check, u64::from(r.startup.witnessed)]);
    print(b"NATIVE CBS CONFIG", &[r.epoch, r.hz, r.span, r.guard]);
    for (id, (q,p)) in NATIVE_RESERVATIONS.into_iter().enumerate() {
        print(b"NATIVE CBS JOB", &[id as u64, u64::from(q), u64::from(p)]);
    }
    unsafe {
        for i in 0..USED { print(b"NATIVE CBS EVENT", &*(&raw const ROWS[i])); }
        print(b"NATIVE CBS DROPPED", &[LOST]);
    }
}

pub(super) fn finish() -> ! {
    let r = state();
    head(10, 1); head(1, 3);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    dump(r);
    unsafe {
        for (pid, status) in EXIT_STATUS.into_iter().enumerate() {
            if COUNT == 1 {
                uart_write(b"NATIVE EL0 EXIT "); decimal(i64::from(status)); uart_write(b"\n");
            } else { event(b"EXIT", pid, Some(i64::from(status))); }
        }
    }
    park()
}
