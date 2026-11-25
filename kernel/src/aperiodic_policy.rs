//! Uniprocessor polling, deferrable and sporadic servers on a logical tick clock.
//! Release work at a tick boundary, then call `tick` once for [t, t+1). The
//! caller arbitrates the CPU and supplies whether the server can run. Each
//! successful dispatch serves exactly one tick of aggregate aperiodic work.
//! The caller owns job identity, live timers and dispatch overhead accounting.

use core::hint::black_box;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServerKind {
    /// Poll at start + kP. Forfeit the current allocation on an empty poll
    /// or immediately after serving the last pending unit.
    Polling,
    /// Keep unused budget until the next aligned boundary, then replace it by Q.
    Deferrable,
    /// Tick-granular ideal sporadic server with bounded replenishment storage.
    /// Each unit served at t earns one unit back at t+P. When the queue is
    /// full, move its youngest entry to the new, later due time and add the
    /// new unit to it. This can delay service under fragmentation.
    /// Unit-capacity model: <https://www.cs.fsu.edu/files/reports/TR-091026.pdf>.
    Sporadic,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ServerError {
    EmptyRelease,
    BacklogOverflow,
    TimeOverflow,
    TimeRegression,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Replenishment {
    pub(crate) due: u64,
    pub(crate) amount: u32,
}

/// R is the maximum number of pending sporadic replenishments. Periodic
/// servers may use R=0. Backlog is an aggregate of at most u32::MAX ticks.
pub(crate) struct AperiodicServer<const R: usize> {
    kind: ServerKind,
    start: u64,
    now: u64,
    quota: u32,
    period: u32,
    remaining: u32,
    backlog: u32,
    replenishments: [Replenishment; R],
    used: usize,
    capacity: usize,
}

impl<const R: usize> AperiodicServer<R> {
    pub(crate) fn new(kind: ServerKind, start: u64, quota: u32, period: u32) -> Option<Self> {
        if quota == 0 || quota > period || (kind == ServerKind::Sporadic && R == 0) {
            return None;
        }
        Some(Self {
            kind,
            start,
            now: start,
            quota,
            period,
            remaining: quota,
            backlog: 0,
            replenishments: [Replenishment { due: 0, amount: 0 }; R],
            used: 0,
            capacity: R,
        })
    }

    pub(crate) fn new_live(kind: ServerKind, start: u64, quota: u32, period: u32,
                           capacity: usize) -> Option<Self> {
        match kind {
            ServerKind::Sporadic if !(1..=8).contains(&capacity) || capacity > R => return None,
            ServerKind::Polling | ServerKind::Deferrable if capacity != 0 => return None,
            _ => {}
        }
        let mut server = Self::new(kind, start, quota, period)?;
        server.capacity = capacity;
        Some(server)
    }

    /// A live grant does not drain demand. Only a real surrender, block or exit
    /// forfeits any remaining polling authorization before another decision.
    pub(crate) fn surrender_live(&mut self) {
        if self.kind == ServerKind::Polling { self.remaining = 0; }
    }

    pub(crate) fn tick_live(&mut self, runnable: bool, cpu_available: bool) -> Result<bool, ServerError> {
        self.tick_demand(cpu_available, Some(runnable))
    }

    /// Settle [now,target) with no grants, in time bounded by the credit table.
    /// The caller splits intervals at externally observed readiness changes.
    pub(crate) fn skip_live_to(&mut self, target: u64, runnable: bool) -> Result<(), ServerError> {
        if target < self.now { return Err(ServerError::TimeRegression); }
        if target == self.now { return Ok(()); }
        match self.kind {
            ServerKind::Sporadic => {
                let matured = self.pending_replenishments().partition_point(|entry| entry.due < target);
                // Every pending unit is a previous debit from this same quota.
                for entry in &self.replenishments[..matured] { self.remaining += entry.amount; }
                self.replenishments.copy_within(matured..self.used, 0);
                self.used -= matured;
            }
            ServerKind::Polling if !runnable => self.remaining = 0,
            ServerKind::Polling | ServerKind::Deferrable => {
                let last = target - 1;
                let aligned = last - (last - self.start) % u64::from(self.period);
                if aligned >= self.now { self.remaining = self.quota; }
            }
        }
        self.now = target;
        Ok(())
    }

    pub(crate) fn release(&mut self, work: u32) -> Result<(), ServerError> {
        if work == 0 {
            return Err(ServerError::EmptyRelease);
        }
        self.backlog = self
            .backlog
            .checked_add(work)
            .ok_or(ServerError::BacklogOverflow)?;
        Ok(())
    }

    pub(crate) fn now(&self) -> u64 {
        self.now
    }

    pub(crate) fn remaining_quota(&self) -> u32 {
        self.remaining
    }

    pub(crate) fn unreserved_work(&self) -> u32 {
        self.backlog
    }

    /// Entries are ordered by due tick. A full queue can merge several units
    /// at the youngest due tick.
    pub(crate) fn pending_replenishments(&self) -> &[Replenishment] {
        &self.replenishments[..self.used]
    }

    /// Replenishments precede dispatch. Preemption advances the clock and
    /// processes due credits without charging service. Errors leave the clock,
    /// backlog, budget and replenishment queue intact.
    pub(crate) fn tick(&mut self, cpu_available: bool) -> Result<bool, ServerError> {
        self.tick_demand(cpu_available, None)
    }

    fn tick_demand(&mut self, cpu_available: bool, live_runnable: Option<bool>) -> Result<bool, ServerError> {
        let next = self.now.checked_add(1).ok_or(ServerError::TimeOverflow)?;
        let period = u64::from(self.period);
        // Due times are strictly increasing and the clock advances one tick
        // at a time. At most one entry matures at this boundary.
        let due = self.used > 0 && self.replenishments[0].due == self.now;
        let available = match self.kind {
            ServerKind::Sporadic if due => self.remaining + self.replenishments[0].amount,
            ServerKind::Polling | ServerKind::Deferrable
                if (self.now - self.start) % period == 0 =>
            {
                self.quota
            }
            _ => self.remaining,
        };
        let run = cpu_available && live_runnable.unwrap_or(self.backlog > 0) && available > 0;
        let repay_at = if run && self.kind == ServerKind::Sporadic {
            Some(
                self.now
                    .checked_add(period)
                    .ok_or(ServerError::TimeOverflow)?,
            )
        } else {
            None
        };

        if due {
            self.replenishments.copy_within(1..self.used, 0);
            self.used -= 1;
        }
        if let Some(due) = repay_at {
            if self.used < self.capacity {
                self.replenishments[self.used] = Replenishment { due, amount: 1 };
                self.used += 1;
            } else {
                // A debit moves one unit from available to pending. Their sum
                // stays at quota, so both the credit and this merge fit u32.
                let latest = &mut self.replenishments[self.used - 1];
                latest.amount += 1;
                latest.due = due;
            }
        }
        self.remaining = available;
        if run {
            self.remaining -= 1;
            if live_runnable.is_none() { self.backlog -= 1; }
        }
        if self.kind == ServerKind::Polling && !live_runnable.unwrap_or(self.backlog > 0) {
            self.remaining = 0;
        }
        self.now = next;
        Ok(run)
    }
}

fn service_trace_checks() -> bool {
    use ServerKind::*;

    let (start, quota, period) = black_box((17, 2, 4));
    // An empty first poll, a release one tick later, then releases on both
    // sides of the next boundary. Each row lists actual service in PS/DS/SS order.
    let trace = [
        (0, [false, false, false]),
        (1, [false, true, true]),
        (0, [false, false, false]),
        (3, [false, true, true]),
        (1, [true, true, false]),
        (0, [true, true, true]),
        (0, [false, false, false]),
        (1, [false, false, true]),
        (0, [true, true, false]),
        (0, [true, true, true]),
    ];
    for (column, kind) in [Polling, Deferrable, Sporadic].into_iter().enumerate() {
        let Some(mut server) = AperiodicServer::<2>::new(kind, start, quota, period) else {
            return false;
        };
        for (work, expected) in trace {
            let work = black_box(work);
            if work > 0 && server.release(work).is_err() {
                return false;
            }
            if server.tick(true) != Ok(expected[column]) {
                return false;
            }
        }
    }
    true
}

fn periodic_boundary_checks() -> bool {
    use ServerKind::*;

    let (quota, period) = black_box((3, 4));
    for kind in [Polling, Deferrable] {
        let Some(mut server) = AperiodicServer::<0>::new(kind, 5, quota, period) else {
            return false;
        };
        // A nonempty poll while preempted retains its budget. The queue
        // drains at the end of the next tick. Release again immediately.
        if server.release(1).is_err()
            || server.tick(false) != Ok(false)
            || server.remaining != 3
            || server.tick(true) != Ok(true)
            || server.release(6).is_err()
            || server.tick(true) != Ok(kind == Deferrable)
            || server.tick(false) != Ok(false)
        {
            return false;
        }
        // DS carried one unused unit up to this boundary. Both periodic
        // servers now receive exactly three units with demand still pending.
        for expected in [true, true, true, false] {
            if server.tick(true) != Ok(expected) {
                return false;
            }
        }
    }
    true
}

fn sporadic_queue_checks() -> bool {
    let (quota, period) = black_box((3, 5));
    let Some(mut server) = AperiodicServer::<2>::new(ServerKind::Sporadic, 0, quota, period) else {
        return false;
    };
    if server.release(7).is_err() {
        return false;
    }
    // t=0 and 2 create credits due at 5 and 7. Service at 3 fills the
    // existing tail to two units due at 8. The oldest credit is still due at 5.
    for can_run in black_box([true, false, true, true]) {
        if server.tick(can_run) != Ok(can_run) {
            return false;
        }
    }
    if server.used != 2
        || server.replenishments
            != [
                Replenishment { due: 5, amount: 1 },
                Replenishment { due: 8, amount: 2 },
            ]
        || server.remaining != 0
    {
        return false;
    }
    // t=4..15: credits return on their due tick, also while the server is
    // idle. In particular no unit appears at the superseded due time t=7.
    for run in [
        false, true, false, false, true, true, true, false, false, false, false, false,
    ] {
        if server.tick(true) != Ok(run) {
            return false;
        }
        let pending: u32 = server.replenishments[..server.used]
            .iter()
            .map(|r| r.amount)
            .sum();
        if server.remaining + pending != quota {
            return false;
        }
    }
    server.used == 0 && server.remaining == quota && server.backlog == 0
}

fn error_checks() -> bool {
    use ServerError::*;
    use ServerKind::*;

    let (start, period) = black_box((u64::MAX - 4, 3));
    if AperiodicServer::<0>::new(Sporadic, 0, 1, 2).is_some()
        || AperiodicServer::<1>::new(Polling, 0, 1, 0).is_some()
        || AperiodicServer::<1>::new(Deferrable, 0, 0, period).is_some()
        || AperiodicServer::<1>::new(Sporadic, 0, 4, period).is_some()
    {
        return false;
    }
    let Some(mut server) = AperiodicServer::<1>::new(Sporadic, start, 1, period) else {
        return false;
    };
    if server.release(0) != Err(EmptyRelease)
        || server.release(u32::MAX).is_err()
        || server.release(1) != Err(BacklogOverflow)
        || server.backlog != u32::MAX
        || server.tick(true) != Ok(true)
        || server.tick(true) != Ok(false)
        || server.tick(true) != Ok(false)
    {
        return false;
    }
    // The old credit is due now, but charging its replacement would overflow.
    // A preempted retry at the same boundary can still recover the old credit.
    server.tick(true) == Err(TimeOverflow)
        && server.now == u64::MAX - 1
        && server.remaining == 0
        && server.used == 1
        && server.replenishments[0]
            == (Replenishment {
                due: u64::MAX - 1,
                amount: 1,
            })
        && server.tick(false) == Ok(false)
        && server.tick(true) == Err(TimeOverflow)
        && server.now == u64::MAX
        && server.remaining == 1
        && server.used == 0
        && server.backlog == u32::MAX - 1
}

pub(crate) fn bench_checks() -> bool {
    service_trace_checks()
        && periodic_boundary_checks()
        && sporadic_queue_checks()
        && error_checks()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_releases_distinguish_the_three_servers() {
        assert!(service_trace_checks());
    }

    #[test]
    fn periodic_servers_handle_preemption_drain_and_boundary() {
        assert!(periodic_boundary_checks());
    }

    #[test]
    fn full_replenishment_queue_delays_credit_and_preserves_budget() {
        assert!(sporadic_queue_checks());
    }

    #[test]
    fn invalid_parameters_and_overflows_preserve_state() {
        assert!(error_checks());
    }

    #[test]
    fn live_grant_remains_backlogged_until_observed_surrender() {
        for kind in [ServerKind::Polling, ServerKind::Deferrable] {
            let mut s = AperiodicServer::<8>::new_live(kind, 0, 3, 5, 0).unwrap();
            assert_eq!(s.tick_live(true, true), Ok(true));
            assert_eq!(s.tick_live(true, true), Ok(true));
            assert_eq!(s.remaining_quota(), 1);
            s.surrender_live();
            assert_eq!(s.tick_live(true, true), Ok(kind == ServerKind::Deferrable));
            s.skip_live_to(5, false).unwrap();
            assert_eq!(s.tick_live(false, true), Ok(false));
            assert_eq!(s.tick_live(true, true), Ok(kind == ServerKind::Deferrable));
        }
    }

    #[test]
    fn configured_credit_limit_merges_youngest_and_matures_at_exact_due() {
        for cap in 1..=8 {
            let mut s = AperiodicServer::<8>::new_live(ServerKind::Sporadic, 0, 9, 11, cap).unwrap();
            for k in 0..9 {
                assert_eq!(s.tick_live(true, true), Ok(true));
                assert_eq!(s.remaining_quota() + s.pending_replenishments().iter().map(|x| x.amount).sum::<u32>(), 9);
                assert_eq!(s.pending_replenishments().len(), (k+1).min(cap));
            }
            assert_eq!(s.pending_replenishments()[cap-1], Replenishment { due: 19, amount: 10-cap as u32 });
            s.surrender_live();
            s.skip_live_to(19, false).unwrap();
            assert_eq!(s.pending_replenishments().len(), 1);
            assert_eq!(s.remaining_quota(), cap as u32 - 1);
            assert_eq!(s.tick_live(false, true), Ok(false));
            assert_eq!(s.remaining_quota(), 9);
        }
    }

    #[test]
    fn skipped_live_cells_match_unavailable_individual_ticks() {
        for kind in [ServerKind::Polling, ServerKind::Deferrable, ServerKind::Sporadic] {
            for end in 20..35 {
                for runnable in [true, false] {
                    let cap = if kind == ServerKind::Sporadic { 2 } else { 0 };
                    let make = || AperiodicServer::<8>::new_live(kind, 17, 3, 5, cap).unwrap();
                    let (mut batched, mut stepped) = (make(), make());
                    for s in [&mut batched, &mut stepped] {
                        for _ in 0..3 { assert_eq!(s.tick_live(true, true), Ok(true)); }
                    }
                    batched.skip_live_to(end, runnable).unwrap();
                    while stepped.now() < end { assert_eq!(stepped.tick_live(runnable, false), Ok(false)); }
                    assert_eq!(batched.remaining_quota(), stepped.remaining_quota());
                    assert_eq!(batched.pending_replenishments(), stepped.pending_replenishments());
                    assert_eq!(batched.skip_live_to(end-1, runnable), Err(ServerError::TimeRegression));
                    assert_eq!(batched.now(), end);
                }
            }
        }
        for cap in [0, 9] {
            assert!(AperiodicServer::<8>::new_live(ServerKind::Sporadic, 0, 2, 5, cap).is_none());
        }
        assert!(AperiodicServer::<1>::new_live(ServerKind::Sporadic, 0, 2, 5, 2).is_none());
        assert!(AperiodicServer::<8>::new_live(ServerKind::Polling, 0, 2, 5, 1).is_none());
    }

}
