//! Bounded policy decisions over caller-supplied snapshots. The caller owns
//! task execution, hardware operating points and enforcement of grant expiry.

use core::hint::black_box;

#[derive(Clone, Copy)]
struct Budget {
    limit: u64,
    left: u64,
}

impl Budget {
    fn new(limit: u64) -> Self {
        Self { limit, left: limit }
    }
}

/// One CPU, two budget levels and a common period aligned to the start tick.
/// Lower group IDs have priority, then lower child IDs within a group.
pub(crate) struct GroupScheduler<const G: usize, const T: usize> {
    groups: [Budget; G],
    children: [(usize, Budget); T],
    period: u64,
    window_start: u64,
    earliest_start: u64,
}

impl<const G: usize, const T: usize> GroupScheduler<G, T> {
    /// Each child supplies its group ID and per-period budget in ticks.
    pub(crate) fn new(
        start: u64,
        period: u64,
        groups: [u64; G],
        children: [(usize, u64); T],
    ) -> Option<Self> {
        let valid = |limit: u64| limit > 0 && limit <= period;
        if period == 0
            || groups.iter().any(|&limit| !valid(limit))
            || children.iter().any(|&(g, limit)| g >= G || !valid(limit))
        {
            return None;
        }
        Some(Self {
            groups: groups.map(Budget::new),
            children: children.map(|(g, limit)| (g, Budget::new(limit))),
            period,
            window_start: start,
            earliest_start: start,
        })
    }

    /// Reserve one contiguous interval for a ready child. Returns (group,
    /// child, ticks). The whole interval is charged once at both levels.
    /// Calls before the last observed time or committed end have no effect.
    pub(crate) fn grant_at(
        &mut self,
        now: u64,
        requested: u64,
        ready: &[bool; T],
    ) -> Option<(usize, usize, u64)> {
        if now < self.earliest_start {
            return None;
        }
        self.earliest_start = now;
        let elapsed = now - self.window_start;
        let phase = elapsed % self.period;
        if elapsed >= self.period {
            self.window_start = now - phase;
            for budget in &mut self.groups {
                budget.left = budget.limit;
            }
            for (_, budget) in &mut self.children {
                budget.left = budget.limit;
            }
        }
        let available = requested.min(self.period - phase).min(u64::MAX - now);
        if available == 0 {
            return None;
        }
        let (group, child) = self
            .children
            .iter()
            .enumerate()
            .filter(|(id, (group, budget))| {
                ready[*id] && budget.left > 0 && self.groups[*group].left > 0
            })
            .map(|(id, &(group, _))| (group, id))
            .min()?;
        let ticks = available
            .min(self.groups[group].left)
            .min(self.children[child].1.left);
        self.groups[group].left -= ticks;
        self.children[child].1.left -= ticks;
        self.earliest_start = now + ticks;
        Some((group, child, ticks))
    }
}

fn cpu_bit(cpu: u8) -> u64 {
    1u64.checked_shl(u32::from(cpu)).unwrap_or(0)
}

/// Capacity is work units per tick. Energy values share one caller-defined unit.
#[derive(Clone, Copy)]
pub(crate) struct OperatingPoint {
    pub(crate) cpu: u8,
    pub(crate) opp: u16,
    pub(crate) capacity: u64,
    pub(crate) energy_per_work: u64,
    pub(crate) wake_energy: u64,
    pub(crate) wake_ticks: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct EnergyRequest {
    pub(crate) allowed_cpus: u64,
    pub(crate) work: u64,
    pub(crate) minimum_capacity: u64,
    pub(crate) now: u64,
    pub(crate) deadline: u64,
}

/// Select an eligible CPU/OPP that can finish by the absolute deadline.
/// Compare work * energy_per_work + wake_energy, breaking ties by CPU then OPP.
/// Missing or infeasible candidates yield no placement.
pub(crate) fn energy_placement<const N: usize>(
    request: EnergyRequest,
    points: &[OperatingPoint; N],
) -> Option<(u8, u16)> {
    if request.work == 0 {
        return None;
    }
    points
        .iter()
        .filter_map(|point| {
            if request.allowed_cpus & cpu_bit(point.cpu) == 0
                || point.capacity == 0
                || point.capacity < request.minimum_capacity
            {
                return None;
            }
            let run_ticks = request.work.div_ceil(point.capacity);
            let finish =
                u128::from(request.now) + u128::from(point.wake_ticks) + u128::from(run_ticks);
            if finish > u128::from(request.deadline) {
                return None;
            }
            let cost = u128::from(request.work) * u128::from(point.energy_per_work)
                + u128::from(point.wake_energy);
            Some((cost, point.cpu, point.opp))
        })
        .min()
        .map(|(_, cpu, opp)| (cpu, opp))
}

/// Trusted task-slot snapshot. Generations must remain distinct for as long
/// as a proposal for an earlier occupant can be presented.
#[derive(Clone, Copy)]
pub(crate) struct DelegatedTask {
    pub(crate) generation: u64,
    pub(crate) ready: bool,
    pub(crate) allowed_cpus: u64,
    pub(crate) grant_limit: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Proposal {
    pub(crate) task: usize,
    pub(crate) generation: u64,
    pub(crate) cpu: u8,
    pub(crate) ticks: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GateDecision {
    Accepted(Proposal),
    Fallback(Proposal),
}

/// Validate a data-only proposal against one trusted snapshot. On absent or
/// invalid input grant one tick to the lowest eligible task on its lowest
/// permitted CPU. No eligible task or zero global allowance leaves the CPU idle.
pub(crate) fn gate_decision<const N: usize>(
    tasks: &[DelegatedTask; N],
    eligible_cpus: u64,
    max_grant: u64,
    proposal: Option<Proposal>,
) -> Option<GateDecision> {
    if max_grant == 0 || eligible_cpus == 0 {
        return None;
    }
    if let Some(p) = proposal {
        if tasks.get(p.task).is_some_and(|task| {
            task.ready
                && task.generation == p.generation
                && task.allowed_cpus & eligible_cpus & cpu_bit(p.cpu) != 0
                && p.ticks > 0
                && p.ticks <= max_grant.min(task.grant_limit)
        }) {
            return Some(GateDecision::Accepted(p));
        }
    }
    tasks.iter().enumerate().find_map(|(id, task)| {
        let cpus = task.allowed_cpus & eligible_cpus;
        (task.ready && task.grant_limit > 0 && cpus != 0).then(|| {
            GateDecision::Fallback(Proposal {
                task: id,
                generation: task.generation,
                cpu: cpus.trailing_zeros() as u8,
                ticks: 1,
            })
        })
    })
}

fn group_checks() -> bool {
    let Some(mut groups) = GroupScheduler::new(
        10,
        8,
        black_box([3, 4]),
        black_box([(1, 4), (0, 2), (0, 3)]),
    ) else {
        return false;
    };
    let all = black_box([true; 3]);
    groups.grant_at(9, 1, &all).is_none()
        && groups.grant_at(10, 8, &all) == Some((0, 1, 2))
        && groups.grant_at(11, 8, &all).is_none()
        && groups.grant_at(12, 8, &all) == Some((0, 2, 1))
        && groups.grant_at(13, 8, &all) == Some((1, 0, 4))
        && groups.grant_at(17, 8, &all).is_none()
        && groups.grant_at(18, 8, &[false; 3]).is_none()
        && groups.grant_at(18, 1, &[false, false, true]) == Some((0, 2, 1))
        && groups.grant_at(19, 2, &all) == Some((0, 1, 2))
        && groups.grant_at(21, 1, &[false, true, true]).is_none()
        && groups.grant_at(20, 1, &all).is_none()
        && groups.grant_at(41, 8, &all) == Some((0, 1, 1))
        && groups.grant_at(42, 8, &all) == Some((0, 1, 2))
        && groups.grant_at(u64::MAX - 1, 8, &all) == Some((0, 1, 1))
        && groups.grant_at(u64::MAX, 8, &all).is_none()
        && GroupScheduler::new(0, 8, [2], [(1, 1)]).is_none()
        && GroupScheduler::new(0, 8, [0], [(0, 1)]).is_none()
        && GroupScheduler::new(0, 8, [2], [(0, 9)]).is_none()
}

fn energy_checks() -> bool {
    let slow = OperatingPoint {
        cpu: 2,
        opp: 1,
        capacity: 2,
        energy_per_work: 2,
        wake_energy: 0,
        wake_ticks: 0,
    };
    let fast = OperatingPoint {
        cpu: 1,
        opp: 3,
        capacity: 5,
        energy_per_work: 5,
        ..slow
    };
    let request = black_box(EnergyRequest {
        allowed_cpus: 0b110,
        work: 9,
        minimum_capacity: 1,
        now: 5,
        deadline: 10,
    });
    let points = black_box([fast, slow]);
    let last = EnergyRequest {
        now: u64::MAX - 1,
        deadline: u64::MAX,
        work: u64::MAX,
        ..request
    };
    let expensive = OperatingPoint {
        capacity: u64::MAX,
        energy_per_work: u64::MAX,
        ..fast
    };
    let cheaper = OperatingPoint {
        cpu: slow.cpu,
        energy_per_work: u64::MAX - 1,
        ..expensive
    };
    let tighter = EnergyRequest {
        deadline: 9,
        ..request
    };
    let higher_capacity = EnergyRequest {
        minimum_capacity: 3,
        ..request
    };
    let impossible = EnergyRequest {
        deadline: 6,
        ..request
    };
    let costly_wake = OperatingPoint {
        wake_energy: 30,
        ..slow
    };
    let late_wake = OperatingPoint {
        wake_ticks: 1,
        ..slow
    };
    let zero_capacity = OperatingPoint {
        capacity: 0,
        ..slow
    };
    let late_last = OperatingPoint {
        wake_ticks: 1,
        ..cheaper
    };
    let tied = [
        slow,
        OperatingPoint {
            cpu: 1,
            opp: 5,
            ..slow
        },
        OperatingPoint {
            cpu: 1,
            opp: 2,
            ..slow
        },
    ];
    energy_placement(request, &points) == Some((2, 1))
        && energy_placement(tighter, &points) == Some((1, 3))
        && energy_placement(higher_capacity, &points) == Some((1, 3))
        && energy_placement(request, &[costly_wake, fast]) == Some((1, 3))
        && energy_placement(request, &[late_wake, fast]) == Some((1, 3))
        && energy_placement(request, &[OperatingPoint { cpu: 0, ..slow }, fast]) == Some((1, 3))
        && energy_placement(request, &[OperatingPoint { cpu: 64, ..slow }, fast]) == Some((1, 3))
        && energy_placement(request, &tied) == Some((1, 2))
        && energy_placement(request, &[zero_capacity]).is_none()
        && energy_placement(impossible, &points).is_none()
        && energy_placement(last, &[expensive, cheaper]) == Some((2, 3))
        && energy_placement(last, &[late_last]).is_none()
}

fn gate_checks() -> bool {
    let task = DelegatedTask {
        generation: 7,
        ready: true,
        allowed_cpus: 0b110,
        grant_limit: 3,
    };
    let tasks = black_box([
        DelegatedTask {
            ready: false,
            ..task
        },
        DelegatedTask {
            grant_limit: 0,
            ..task
        },
        task,
        DelegatedTask {
            generation: 8,
            ..task
        },
    ]);
    let p = black_box(Proposal {
        task: 3,
        generation: 8,
        cpu: 2,
        ticks: 2,
    });
    let fallback = Proposal {
        task: 2,
        generation: 7,
        cpu: 1,
        ticks: 1,
    };
    if gate_decision(&tasks, 0b111, 2, Some(p)) != Some(GateDecision::Accepted(p)) {
        return false;
    }
    for invalid in [
        None,
        Some(Proposal { generation: 7, ..p }),
        Some(Proposal {
            task: usize::MAX,
            ..p
        }),
        Some(Proposal {
            task: 0,
            generation: 7,
            ..p
        }),
        Some(Proposal {
            task: 1,
            generation: 7,
            ..p
        }),
        Some(Proposal { cpu: 0, ..p }),
        Some(Proposal { cpu: 64, ..p }),
        Some(Proposal { ticks: 0, ..p }),
        Some(Proposal { ticks: 3, ..p }),
    ] {
        if gate_decision(&tasks, 0b111, 2, invalid) != Some(GateDecision::Fallback(fallback)) {
            return false;
        }
    }
    gate_decision(&tasks, 0b010, 2, Some(p)) == Some(GateDecision::Fallback(fallback))
        && gate_decision(&tasks, 0b111, 8, Some(Proposal { ticks: 4, ..p }))
            == Some(GateDecision::Fallback(fallback))
        && gate_decision(&tasks, 0b001, 2, Some(p)).is_none()
        && gate_decision(&tasks, u64::MAX, 0, Some(p)).is_none()
}

pub(crate) fn bench_checks() -> bool {
    group_checks() && energy_checks() && gate_checks()
}
