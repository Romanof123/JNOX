//! Single-core dispatch policies on an integer tick clock. These backends own
//! policy state. The caller owns task frames, timer interrupts and execution.

use core::hint::black_box;

#[derive(Clone, Copy)]
struct Window {
    partition: Option<usize>,
    end: u64,
}

/// A repeating table of half-open partition windows, anchored at tick zero.
pub(crate) struct CyclicExecutive<const N: usize> {
    frame_ticks: u64,
    windows: [Window; N],
}

impl<const N: usize> CyclicExecutive<N> {
    /// Each slot gives a partition ID (or idle) and a positive duration. The
    /// configured durations must exactly cover the declared major frame.
    pub(crate) fn new(
        partition_count: usize,
        frame_ticks: u64,
        slots: [(Option<usize>, u64); N],
    ) -> Option<Self> {
        if frame_ticks == 0 {
            return None;
        }
        let mut windows = [Window {
            partition: None,
            end: 0,
        }; N];
        let mut end = 0u64;
        for (window, (partition, ticks)) in windows.iter_mut().zip(slots) {
            if ticks == 0 || partition.is_some_and(|id| id >= partition_count) {
                return None;
            }
            end = end.checked_add(ticks)?;
            *window = Window { partition, end };
        }
        (end == frame_ticks).then_some(Self {
            frame_ticks,
            windows,
        })
    }

    /// A blocked partition leaves its window idle. No other partition may
    /// borrow that window. Partition-local thread selection is up to the caller.
    pub(crate) fn dispatch(&self, tick: u64, ready: impl FnOnce(usize) -> bool) -> Option<usize> {
        let phase = tick % self.frame_ticks;
        let partition = self
            .windows
            .iter()
            .find(|window| phase < window.end)?
            .partition?;
        ready(partition).then_some(partition)
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
}

/// Hard constant-bandwidth servers under preemptive uniprocessor EDF. Each
/// reservation has an implicit relative deadline equal to its period and at
/// most one outstanding job. Jobs release at tick boundaries and run until
/// completion or preemption. The caller supplies an admitted reservation set.
/// Equal scheduling deadlines use ascending server ID. No bandwidth reclaiming.
/// See <https://docs.kernel.org/scheduler/sched-deadline.html#main-algorithm>.
pub(crate) struct HardCbs<const N: usize> {
    now: u64,
    servers: [Server; N],
}

impl<const N: usize> HardCbs<N> {
    /// Reservation parameters are (quota, period), in whole ticks.
    pub(crate) fn new(start_tick: u64, reservations: [(u32, u32); N]) -> Option<Self> {
        if reservations
            .iter()
            .any(|&(quota, period)| quota == 0 || quota > period)
        {
            return None;
        }
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
        let time_left = server.deadline.saturating_sub(self.now);
        if time_left == 0
            || u128::from(server.remaining) * u128::from(server.period)
                > u128::from(server.quota) * u128::from(time_left)
        {
            server.deadline = self
                .now
                .checked_add(u64::from(server.period))
                .ok_or(CbsError::TimeOverflow)?;
            server.remaining = server.quota;
        }
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
        let mut winner = None;
        for (id, server) in self.servers.iter().enumerate() {
            if server.work == 0 {
                continue;
            }
            let (deadline, remaining) = if server.remaining == 0 {
                if self.now < server.deadline {
                    continue;
                }
                let deadline = server
                    .deadline
                    .checked_add(u64::from(server.period))
                    .ok_or(CbsError::TimeOverflow)?;
                (deadline, server.quota)
            } else {
                (server.deadline, server.remaining)
            };
            if deadline <= self.now {
                return Err(CbsError::MissedDeadline(id));
            }
            if winner.is_none_or(|(_, earliest, _)| deadline < earliest) {
                winner = Some((id, deadline, remaining));
            }
        }
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
    let Some(mut overloaded) = HardCbs::new(0, black_box([(1, 1), (1, 1)])) else {
        return false;
    };
    if overloaded.release(2, 1) != Err(CbsError::UnknownServer)
        || overloaded.release(0, 0) != Err(CbsError::EmptyJob)
        || overloaded.release(0, 2).is_err()
        || overloaded.release(1, 1).is_err()
        || overloaded.release(0, 3) != Err(CbsError::JobPending)
        || overloaded.tick() != Ok(Some(0))
        || overloaded.tick() != Err(CbsError::MissedDeadline(1))
        || overloaded.now() != 1
        || overloaded.servers[0].work != 1
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
        && HardCbs::new(0, [(0, 1)]).is_none()
        && HardCbs::new(0, [(2, 1)]).is_none()
}

pub(crate) fn bench_checks() -> bool {
    cyclic_checks() && cbs_wake_checks() && cbs_dispatch_checks() && cbs_error_checks()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyclic_frame_boundaries_and_configuration() {
        assert!(cyclic_checks());
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
