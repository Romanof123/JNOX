//! Single-core dispatch policies on an integer tick clock. These backends own
//! policy state. The caller owns task frames, timer interrupts and execution.

use core::hint::black_box;

#[derive(Clone, Copy)]
struct Window {
    partition: Option<usize>,
    end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowDecision {
    pub(crate) owner: Option<usize>,
    pub(crate) runnable: Option<usize>,
    pub(crate) end_tick: u64,
}

/// A repeating table of half-open partition windows, anchored at tick zero.
pub(crate) struct CyclicExecutive<const N: usize> {
    frame_ticks: u64,
    windows: [Window; N],
    count: usize,
}

impl<const N: usize> CyclicExecutive<N> {
    /// Each slot gives a partition ID (or idle) and a positive duration. The
    /// configured durations must exactly cover the declared major frame.
    pub(crate) fn new(
        partition_count: usize,
        frame_ticks: u64,
        slots: [(Option<usize>, u64); N],
    ) -> Option<Self> {
        let table = Self::from_ticks(partition_count, &slots)?;
        (table.frame_ticks == frame_ticks).then_some(table)
    }

    fn from_ticks(partition_count: usize, slots: &[(Option<usize>, u64)]) -> Option<Self> {
        if slots.is_empty() || slots.len() > N { return None; }
        let mut windows = [Window { partition: None, end: 0 }; N];
        let mut end = 0u64;
        for (window, &(partition, ticks)) in windows.iter_mut().zip(slots) {
            if ticks == 0 || partition.is_some_and(|id| id >= partition_count) {
                return None;
            }
            end = end.checked_add(ticks)?;
            *window = Window { partition, end };
        }
        Some(Self { frame_ticks: end, windows, count: slots.len() })
    }

    /// A blocked partition leaves its window idle. No other partition may
    /// borrow that window. Partition-local thread selection is up to the caller.
    pub(crate) fn dispatch(&self, tick: u64, ready: impl FnOnce(usize) -> bool) -> Option<usize> {
        let (slot, _) = self.window_at(tick)?;
        let window = &self.windows[slot];
        window.partition.filter(|&id| ready(id))
    }

    /// Return the active slot and its absolute exclusive end. A slot whose
    /// boundary exceeds the clock range cannot produce a timed decision.
    pub(crate) fn decision(
        &self,
        tick: u64,
        ready: impl FnOnce(usize) -> bool,
    ) -> Option<WindowDecision> {
        let (slot, phase) = self.window_at(tick)?;
        let window = &self.windows[slot];
        let end_tick = tick.checked_add(window.end - phase)?;
        Some(WindowDecision {
            owner: window.partition,
            runnable: window.partition.filter(|&id| ready(id)),
            end_tick,
        })
    }

    fn window_at(&self, tick: u64) -> Option<(usize, u64)> {
        let phase = tick % self.frame_ticks;
        let slot = self.windows[..self.count].iter().position(|window| phase < window.end)?;
        Some((slot, phase))
    }
}

/// Absolute counter view of the same half-open executive used by linked partitions.
pub(crate) struct NativeCyclic {
    epoch: u64,
    table: CyclicExecutive<16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeWindow {
    pub(crate) frame: u64,
    pub(crate) slot: usize,
    pub(crate) sequence: u64,
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) owner: Option<usize>,
    pub(crate) runnable: Option<usize>,
}

impl NativeCyclic {
    pub(crate) fn new(epoch: u64, hz: u64, slots: &[(Option<usize>, u64)]) -> Option<Self> {
        if !(2..=16).contains(&slots.len()) || hz == 0 || hz % 1000 != 0 { return None; }
        let mut converted = [(None, 0); 16];
        let mut owners = [false; 2];
        for (out, &(owner, ms)) in converted.iter_mut().zip(slots) {
            if let Some(id) = owner { *owners.get_mut(id)? = true; }
            let ticks = ms.checked_mul(hz / 1000)?;
            // Signed physical timer comparison must distinguish a future end.
            if ticks > i64::MAX as u64 { return None; }
            *out = (owner, ticks);
        }
        if owners != [true, true] { return None; }
        let table = CyclicExecutive::from_ticks(2, &converted[..slots.len()])?;
        epoch.checked_add(table.frame_ticks)?;
        Some(Self { epoch, table })
    }

    pub(crate) fn frame_ticks(&self) -> u64 { self.table.frame_ticks }

    /// None before epoch, Err when this active window's absolute end cannot fit.
    pub(crate) fn at(&self, now: u64, ready: u8) -> Result<Option<NativeWindow>, ()> {
        let Some(elapsed) = now.checked_sub(self.epoch) else { return Ok(None); };
        let (slot, phase) = self.table.window_at(elapsed).ok_or(())?;
        let frame = elapsed / self.frame_ticks();
        let base = now - phase;
        let offset = if slot == 0 { 0 } else { self.table.windows[slot - 1].end };
        let window = &self.table.windows[slot];
        let sequence = frame.checked_mul(self.table.count as u64)
            .and_then(|first| first.checked_add(slot as u64)).ok_or(())?;
        Ok(Some(NativeWindow {
            frame, slot, sequence, start: base + offset,
            end: base.checked_add(window.end).ok_or(())?,
            owner: window.partition,
            runnable: window.partition.filter(|&id| ready & (1 << id) != 0),
        }))
    }
}

#[derive(Clone, Copy)]
struct Server {
    quota: u32,
    period: u32,
    remaining: u32,
    deadline: u64,
    work: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CbsError {
    UnknownServer,
    EmptyJob,
    JobPending,
    MissedDeadline(usize),
    TimeOverflow,
    InvalidOpening,
    StaleOwner,
    InactiveTransition,
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Accumulate exact reduced fractions in input order. Each intermediate
/// cross-product has a u64 factor and a u32 factor, so their sum fits u128.
/// Reject if the reduced running total exceeds one or cannot fit in two u64s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub(crate) enum AdmissionError { Invalid = 1, Capacity, Arithmetic }

fn admits(reservations: &[(u32, u32)]) -> Result<(), AdmissionError> {
    let (mut numerator, mut denominator) = (0u64, 1u64);
    for &(quota, period) in reservations {
        if quota == 0 || quota > period {
            return Err(AdmissionError::Invalid);
        }
        let period = u128::from(period);
        let denominator_wide = u128::from(denominator);
        let total = u128::from(numerator) * period + u128::from(quota) * denominator_wide;
        let scale = denominator_wide * period;
        if total > scale {
            return Err(AdmissionError::Capacity);
        }
        let divisor = gcd(total, scale);
        numerator = u64::try_from(total / divisor).map_err(|_| AdmissionError::Arithmetic)?;
        denominator = u64::try_from(scale / divisor).map_err(|_| AdmissionError::Arithmetic)?;
    }
    Ok(())
}

/// Hard constant-bandwidth servers under preemptive uniprocessor EDF. Each
/// reservation has an implicit relative deadline equal to its period and at
/// most one outstanding job. Jobs release at tick boundaries and run until
/// completion or preemption. Admission assumes independent reservations on one
/// CPU with no blocking, IRQ interference or scheduling overhead.
/// Equal scheduling deadlines use ascending server ID. No bandwidth reclaiming.
/// See <https://docs.kernel.org/scheduler/sched-deadline.html#main-algorithm>.
pub(crate) struct HardCbs<const N: usize> {
    now: u64,
    servers: [Server; N],
}

impl<const N: usize> HardCbs<N> {
    /// Reservation parameters are (quota, period), in whole ticks.
    pub(crate) fn new(start_tick: u64, reservations: [(u32, u32); N]) -> Option<Self> {
        admits(&reservations).ok()?;
        let servers = reservations.map(|(quota, period)| Server {
            quota,
            period,
            remaining: 0,
            deadline: start_tick,
            work: 0,
        });
        Some(Self {
            now: start_tick,
            servers,
        })
    }

    pub(crate) fn now(&self) -> u64 {
        self.now
    }

    #[cfg(jnox_bench = "cbs_wake")]
    pub(crate) fn allocation(&self, id: usize) -> Option<(u32, u64)> {
        self.servers.get(id).map(|s| (s.remaining, s.deadline))
    }

    /// Wake an idle server. Retain its old allocation when the residual demand
    /// density fits the reservation. Reset both quota and deadline when that
    /// density would exceed Q/P, or when the old deadline has passed. Waking a
    /// depleted server before its deadline preserves its zero budget.
    pub(crate) fn release(&mut self, id: usize, work: u32) -> Result<(), CbsError> {
        let server = self.servers.get_mut(id).ok_or(CbsError::UnknownServer)?;
        if work == 0 {
            return Err(CbsError::EmptyJob);
        }
        if server.work != 0 {
            return Err(CbsError::JobPending);
        }
        activate(server, self.now)?;
        server.work = work;
        Ok(())
    }

    /// Account for [now, now+1), returning the server that used the CPU. A hard
    /// server with zero budget waits until its scheduling deadline. At that
    /// boundary its allocation becomes (Q, old_deadline + P). Replenishment is
    /// evaluated lazily so waiting behind an earlier EDF job retains the same
    /// deadline. Any error leaves the clock and all allocations unchanged.
    pub(crate) fn tick(&mut self) -> Result<Option<usize>, CbsError> {
        let next = self.now.checked_add(1).ok_or(CbsError::TimeOverflow)?;
        let winner = choose_grant(&self.servers, self.now, |_, server| server.work != 0, |_, _, _| {})?;
        self.now = next;
        Ok(winner.map(|(id, deadline, remaining)| {
            let server = &mut self.servers[id];
            server.deadline = deadline;
            server.remaining = remaining - 1;
            server.work -= 1;
            id
        }))
    }
}

fn activate(server: &mut Server, now: u64) -> Result<(), CbsError> {
    let time_left = server.deadline.saturating_sub(now);
    if time_left == 0
        || u128::from(server.remaining) * u128::from(server.period)
            > u128::from(server.quota) * u128::from(time_left)
    {
        server.deadline = now
            .checked_add(u64::from(server.period))
            .ok_or(CbsError::TimeOverflow)?;
        server.remaining = server.quota;
    }
    Ok(())
}

// A candidate includes the allocation after any due hard replenishment.
fn candidate(server: &Server, now: u64, id: usize) -> Result<Option<(u64, u32)>, CbsError> {
    let (deadline, remaining) = if server.remaining == 0 {
        if now < server.deadline { return Ok(None); }
        (server.deadline.checked_add(u64::from(server.period)).ok_or(CbsError::TimeOverflow)?,
         server.quota)
    } else { (server.deadline, server.remaining) };
    if deadline <= now { return Err(CbsError::MissedDeadline(id)); }
    Ok(Some((deadline, remaining)))
}

fn choose_grant<const N: usize>(
    servers: &[Server; N], now: u64, active: impl Fn(usize, &Server) -> bool,
    mut allocation: impl FnMut(usize, u64, u32),
) -> Result<Option<(usize, u64, u32)>, CbsError> {
    let mut winner = None;
    for (id, server) in servers.iter().enumerate() {
        if !active(id, server) { continue; }
        if let Some((deadline, remaining)) = candidate(server, now, id)? {
            allocation(id, deadline, remaining);
            if winner.is_none_or(|(_, earliest, _)| deadline < earliest) {
                winner = Some((id, deadline, remaining));
            }
        }
    }
    Ok(winner)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Readiness { Active(u64), Sleeping, Exited }

/// An admitted persistent process consumes one entitlement per selected opening.
/// The native timer and saved process frames belong to the caller.
pub(crate) struct NativeCbs<const N: usize> {
    servers: [Server; N],
    states: [Readiness; N],
    first_unopened: u64,
    owner: Option<usize>,
}

impl<const N: usize> NativeCbs<N> {
    pub(crate) fn new(reservations: [(u32, u32); N]) -> Result<Self, AdmissionError> {
        if !(1..=3).contains(&N) { return Err(AdmissionError::Invalid); }
        admits(&reservations)?;
        let servers = reservations.map(|(quota, period)| Server {
            quota, period, remaining: quota, deadline: u64::from(period), work: 0,
        });
        Ok(Self { servers, states: [Readiness::Active(0); N], first_unopened: 0, owner: None })
    }

    pub(crate) fn allocation(&self, id: usize) -> Option<(u32, u64)> {
        self.servers.get(id).map(|s| (s.remaining, s.deadline))
    }
    pub(crate) fn active(&self, id: usize) -> bool {
        matches!(self.states.get(id), Some(Readiness::Active(_)))
    }
    pub(crate) fn exited(&self, id: usize) -> bool {
        self.states.get(id) == Some(&Readiness::Exited)
    }
    pub(crate) fn owner(&self) -> Option<usize> { self.owner }

    pub(crate) fn next_slot(&self) -> Option<u64> {
        self.servers.iter().zip(self.states).filter_map(|(s, state)| {
            let Readiness::Active(first) = state else { return None; };
            let eligible = if s.remaining == 0 { first.max(s.deadline) } else { first };
            Some(eligible.max(self.first_unopened))
        }).min()
    }

    pub(crate) fn open(&mut self, t: u64) -> Result<Option<usize>, CbsError> {
        if self.owner.is_some() || t < self.first_unopened || self.next_slot().is_some_and(|due| t > due) {
            return Err(CbsError::InvalidOpening);
        }
        let next = t.checked_add(1).ok_or(CbsError::TimeOverflow)?;
        let active = |id: usize, _: &Server| matches!(self.states[id], Readiness::Active(first) if first <= t);
        // Stage all due refills, including equal-deadline peers that lose this slot.
        let mut allocations = self.servers;
        let winner = choose_grant(&self.servers, t, active, |id, deadline, remaining| {
            allocations[id].deadline = deadline;
            allocations[id].remaining = remaining;
        })?;
        self.servers = allocations;
        self.owner = winner.map(|(id, _, _)| {
            self.servers[id].remaining -= 1;
            id
        });
        self.first_unopened = next;
        Ok(self.owner)
    }

    fn take_owner(&mut self, id: usize) -> Result<(), CbsError> {
        if self.owner != Some(id) { return Err(CbsError::StaleOwner); }
        self.owner = None;
        Ok(())
    }
    pub(crate) fn yield_grant(&mut self, id: usize) -> Result<(), CbsError> { self.take_owner(id) }
    pub(crate) fn cutoff(&mut self, id: usize) -> Result<(), CbsError> { self.take_owner(id) }
    pub(crate) fn sleep(&mut self, id: usize) -> Result<(), CbsError> {
        self.take_owner(id)?;
        self.states[id] = Readiness::Sleeping;
        Ok(())
    }
    pub(crate) fn exit(&mut self, id: usize) -> Result<(), CbsError> {
        self.take_owner(id)?;
        self.states[id] = Readiness::Exited;
        Ok(())
    }
    pub(crate) fn wake(&mut self, id: usize, t: u64) -> Result<(), CbsError> {
        let state = self.states.get(id).ok_or(CbsError::UnknownServer)?;
        if *state != Readiness::Sleeping || t < self.first_unopened {
            return Err(CbsError::InactiveTransition);
        }
        activate(&mut self.servers[id], t)?;
        self.states[id] = Readiness::Active(t);
        Ok(())
    }
}

fn cyclic_checks() -> bool {
    let slots = black_box([(Some(0), 2), (Some(1), 3), (None, 1), (Some(0), 2)]);
    let Some(table) = CyclicExecutive::new(2, 8, slots) else {
        return false;
    };
    let trace = [
        Some(0),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(0),
    ];
    (0..24).all(|tick| table.dispatch(tick, |_| true) == trace[(tick % 8) as usize])
        && table.dispatch(2, |id| id == 0).is_none()
        && table.dispatch(u64::MAX, |_| true) == Some(0)
        && CyclicExecutive::new(2, 7, slots).is_none()
        && CyclicExecutive::new(2, 9, slots).is_none()
        && CyclicExecutive::new(0, 0, []).is_none()
        && CyclicExecutive::new(0, 1, []).is_none()
        && CyclicExecutive::new(0, 1, [(None, 0), (None, 1)]).is_none()
        && CyclicExecutive::new(1, 1, [(Some(1), 1)]).is_none()
        && CyclicExecutive::new(0, 1, black_box([(None, u64::MAX), (None, 2)])).is_none()
}

fn tick_trace<const N: usize>(policy: &mut HardCbs<N>, expected: &[Option<usize>]) -> bool {
    expected.iter().all(|&id| policy.tick() == Ok(id))
}

fn cbs_wake_checks() -> bool {
    let reservation = black_box([(2, 8)]);
    let run = Some(0);
    // For Q/P = 2/8, one tick used at t=0 leaves q=1 and d=8. At t=4
    // q/(d-t) is exactly 2/8. Retain q=1, then throttle through t=7.
    let Some(mut retain) = HardCbs::new(0, reservation) else {
        return false;
    };
    if retain.release(0, 1).is_err()
        || !tick_trace(&mut retain, &[run, None, None, None])
        || retain.release(0, 1).is_err()
        || retain.tick() != Ok(Some(0))
        || retain.release(0, 3).is_err()
        || !tick_trace(&mut retain, &[None, None, None, run, run, None])
        || retain.now() != 11
    {
        return false;
    }
    // Wake one tick later with the same remaining budget. The old deadline
    // would give density 1/3 > 2/8, so both ticks become available with d=13.
    let Some(mut reset) = HardCbs::new(0, reservation) else {
        return false;
    };
    if reset.release(0, 1).is_err()
        || !tick_trace(&mut reset, &[run, None, None, None, None])
        || reset.release(0, 3).is_err()
        || reset.servers[0].deadline != 13
        || !tick_trace(
            &mut reset,
            &[run, run, None, None, None, None, None, None, run],
        )
    {
        return false;
    }
    // At t=14 the residual density is below the limit. At t=22 an expired
    // idle server starts a fresh reservation at wake time.
    reset.release(0, 1).is_ok()
        && reset.servers[0].deadline == 21
        && tick_trace(&mut reset, &[run, None, None, None, None, None, None, None])
        && reset.release(0, 1).is_ok()
        && reset.servers[0].deadline == 30
        && reset.servers[0].remaining == 2
}

fn cbs_dispatch_checks() -> bool {
    let slow = Some(0);
    let fast = Some(1);
    let Some(mut edf) = HardCbs::new(0, black_box([(2, 8), (1, 4)])) else {
        return false;
    };
    if edf.release(0, 4).is_err() || edf.tick() != Ok(Some(0)) || edf.release(1, 2).is_err() {
        return false;
    }
    // Server 1 preempts at t=1, then depletes. It replenishes at t=5 and
    // completes. Server 0 receives its second allocation at t=8.
    if !tick_trace(
        &mut edf,
        &[fast, slow, None, None, fast, None, None, slow, slow],
    ) {
        return false;
    }
    let Some(mut tied) = HardCbs::new(0, black_box([(1, 3), (1, 3)])) else {
        return false;
    };
    tied.release(1, 2).is_ok()
        && tied.release(0, 2).is_ok()
        && tick_trace(&mut tied, &[Some(0), Some(1), None, Some(0), Some(1)])
        && tied.servers[1].deadline == 6
}

fn cbs_error_checks() -> bool {
    let Some(mut active) = HardCbs::new(0, black_box([(1, 2), (1, 2)])) else {
        return false;
    };
    if active.release(2, 1) != Err(CbsError::UnknownServer)
        || active.release(0, 0) != Err(CbsError::EmptyJob)
        || active.release(0, 2).is_err()
        || active.release(1, 1).is_err()
        || active.release(0, 3) != Err(CbsError::JobPending)
        || !tick_trace(&mut active, &[Some(0), Some(1), Some(0)])
    {
        return false;
    }
    let Some(mut renewal) = HardCbs::new(black_box(u64::MAX - 3), [(1, 2)]) else {
        return false;
    };
    if renewal.release(0, 2).is_err()
        || !tick_trace(&mut renewal, &[Some(0), None])
        || renewal.tick() != Err(CbsError::TimeOverflow)
        || renewal.now() != u64::MAX - 1
    {
        return false;
    }
    let Some(mut end) = HardCbs::new(black_box(u64::MAX - 2), [(1, 2), (1, 3)]) else {
        return false;
    };
    end.release(1, 1) == Err(CbsError::TimeOverflow)
        && end.servers[1].work == 0
        && end.release(0, 2).is_ok()
        && tick_trace(&mut end, &[Some(0), None])
        && end.tick() == Err(CbsError::TimeOverflow)
        && end.now() == u64::MAX
}

fn cbs_admission_checks() -> bool {
    let n = black_box(u32::MAX);
    // The second sum exceeds capacity by just 1/(n*(n-1)).
    HardCbs::new(0, [(n - 1, n), (1, n)]).is_some()
        && HardCbs::new(0, [(n - 1, n), (1, n - 1)]).is_none()
        && HardCbs::new(0, [(1, 2), (1, 3), (1, 6)]).is_some()
        && HardCbs::new(0, [(1, 1), (1, 1)]).is_none()
        && HardCbs::new(0, [(0, 1)]).is_none()
        && HardCbs::new(0, [(0, 0)]).is_none()
        && HardCbs::new(0, [(1, 0)]).is_none()
        && HardCbs::new(0, [(2, 1)]).is_none()
        // Pairwise coprime periods give a reduced denominator wider than u64.
        && HardCbs::new(0, [(1, 1 << 31), (1, (1 << 31) - 1), (1, (1 << 31) - 3)]).is_none()
}

pub(crate) fn bench_checks() -> bool {
    cyclic_checks()
        && cbs_admission_checks()
        && cbs_wake_checks()
        && cbs_dispatch_checks()
        && cbs_error_checks()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyclic_frame_boundaries_and_configuration() {
        assert!(cyclic_checks());
    }

    #[test]
    fn hard_cbs_admission_boundaries() {
        assert!(cbs_admission_checks());
    }

    #[test]
    fn hard_cbs_activation_and_throttling() {
        assert!(cbs_wake_checks());
    }

    #[test]
    fn hard_cbs_preemption_and_replenishment() {
        assert!(cbs_dispatch_checks());
    }

    #[test]
    fn hard_cbs_rejects_invalid_transitions() {
        assert!(cbs_error_checks());
    }
}
#[cfg(test)]
mod native_reservation_tests {
    use super::{AdmissionError, CbsError, NativeCbs};

    fn grants<const N: usize>(table: [(u32, u32); N], expected: &[Option<usize>]) {
        let mut p = NativeCbs::new(table).unwrap();
        for (t, &winner) in expected.iter().enumerate() {
            assert_eq!(p.open(t as u64), Ok(winner));
            if let Some(id) = winner { p.cutoff(id).unwrap(); }
        }
    }

    #[test]
    fn admitted_processes_receive_only_their_own_opening() {
        let (a, b, c) = (Some(0), Some(1), Some(2));
        grants([(1, 2), (1, 2)], &[a,b,a,b,a,b,a,b]);
        grants([(1, 4), (1, 4)], &[a,b,None,None,a,b,None,None]);
        grants([(1, 4), (1, 4), (1, 2)], &[c,a,b,c]);
        assert!(matches!(NativeCbs::new([(1, 2), (1, 2), (1, 8)]), Err(AdmissionError::Capacity)));
        assert!(matches!(NativeCbs::new([(0, 2)]), Err(AdmissionError::Invalid)));
        let p = 1 << 31;
        assert!(matches!(NativeCbs::new([(1, p), (1, p-1), (1, p-3)]), Err(AdmissionError::Arithmetic)));
    }

    #[test]
    fn wake_density_equality_zero_and_strict_excess_preserve_or_renew() {
        for (wake, allocation) in [(3, (1, 8)), (4, (1, 8)), (5, (2, 13)), (8, (2, 16))] {
            let mut p = NativeCbs::new([(2, 8)]).unwrap();
            assert_eq!(p.open(0), Ok(Some(0)));
            p.sleep(0).unwrap();
            assert_eq!(p.next_slot(), None);
            p.wake(0, wake).unwrap();
            assert_eq!(p.allocation(0), Some(allocation));
            assert_eq!(p.next_slot(), Some(wake));
            assert_eq!(p.open(wake), Ok(Some(0)));
        }
        let mut p = NativeCbs::new([(1, 4)]).unwrap();
        assert_eq!(p.open(0), Ok(Some(0)));
        p.sleep(0).unwrap();
        p.wake(0, 1).unwrap();
        assert_eq!(p.allocation(0), Some((0, 4)));
        assert_eq!(p.next_slot(), Some(4));
        assert_eq!(p.open(4), Ok(Some(0)));
    }

    #[test]
    fn failed_transitions_leave_debit_owner_and_next_opening_intact() {
        let mut p = NativeCbs::new([(1, 4), (1, 4)]).unwrap();
        assert_eq!(p.open(0), Ok(Some(0)));
        assert_eq!(p.cutoff(1), Err(CbsError::StaleOwner));
        assert_eq!(p.owner(), Some(0));
        p.yield_grant(0).unwrap();
        assert_eq!(p.open(0), Err(CbsError::InvalidOpening));
        assert_eq!(p.open(2), Err(CbsError::InvalidOpening));
        assert_eq!(p.allocation(0), Some((0, 4)));
        assert_eq!(p.next_slot(), Some(1));
        assert_eq!(p.open(1), Ok(Some(1)));
        p.exit(1).unwrap();
        assert_eq!(p.wake(1, 2), Err(CbsError::InactiveTransition));
        assert!(p.exited(1));
        assert_eq!(p.next_slot(), Some(4));
        assert_eq!(p.open(4), Ok(Some(0)));
        p.sleep(0).unwrap();
        assert_eq!(p.wake(0, u64::MAX), Err(CbsError::TimeOverflow));
        assert!(!p.active(0));
        assert_eq!(p.allocation(0), Some((0, 8)));
        assert_eq!(p.open(u64::MAX), Err(CbsError::TimeOverflow));
        assert_eq!(p.owner(), None);
        assert_eq!(p.next_slot(), None);
    }
}

#[cfg(test)]
mod native_cyclic_tests {
    use super::NativeCyclic;

    #[test]
    fn duplicate_late_and_wrap_samples_keep_the_absolute_sequence() {
        let c = NativeCyclic::new(40, 2000, &[(Some(1), 1), (Some(0), 2), (Some(1), 1)]).unwrap();
        assert_eq!(c.frame_ticks(), 8);
        assert_eq!(c.at(39, 3), Ok(None));
        let last = c.at(46, 3).unwrap().unwrap();
        let wrap = c.at(48, 3).unwrap().unwrap();
        assert_eq!((last.owner, last.sequence, last.end), (Some(1), 2, 48));
        assert_eq!((wrap.owner, wrap.sequence, wrap.frame, wrap.slot), (Some(1), 3, 1, 0));
        assert_eq!(c.at(49, 3), Ok(Some(wrap)));
        let late = c.at(74, 3).unwrap().unwrap();
        assert_eq!((late.sequence, late.start, late.end), (13, 74, 78));
    }

    #[test]
    fn wakes_only_restore_the_named_owner_in_the_remainder() {
        let c = NativeCyclic::new(10, 1000, &[(Some(0), 5), (None, 2), (Some(1), 3)]).unwrap();
        assert_eq!(c.at(10, 2).unwrap().unwrap().runnable, None);
        let woke = c.at(13, 3).unwrap().unwrap();
        assert_eq!((woke.runnable, woke.end), (Some(0), 15));
        assert_eq!(c.at(15, 3).unwrap().unwrap().runnable, None);
        assert_eq!(c.at(17, 1).unwrap().unwrap().runnable, None);
        assert_eq!(c.at(17, 3).unwrap().unwrap().runnable, Some(1));
    }

    #[test]
    fn invalid_tables_and_unrepresentable_later_boundaries_are_rejected() {
        let pair = [(Some(0), 1), (Some(1), 1)];
        for hz in [0, 999, 1500] { assert!(NativeCyclic::new(0, hz, &pair).is_none()); }
        for slots in [&pair[..1], &[(Some(0), 1); 17][..], &[(Some(0), 1), (None, 1)],
                      &[(Some(0), 1), (Some(2), 1)], &[(Some(0), 0), (Some(1), 1)],
                      &[(Some(0), u64::MAX), (Some(1), 1)]] {
            assert!(NativeCyclic::new(0, 2000, slots).is_none());
        }
        assert!(NativeCyclic::new(u64::MAX - 1, 1000, &pair).is_none());
        let c = NativeCyclic::new(u64::MAX - 2, 1000, &pair).unwrap();
        assert_eq!(c.at(u64::MAX - 1, 3).unwrap().unwrap().end, u64::MAX);
        assert_eq!(c.at(u64::MAX, 3), Err(()));
    }
}
