//! Single-CPU dispatch ranking for ready CPU bursts. The caller releases jobs,
//! accounts for execution, and chooses when a running job can be preempted.
//! FCFS, SJF and HRRN run to completion in their classical nonpreemptive forms.

use super::ready_queue::ReadyQueue;

#[derive(Clone, Copy)]
pub(crate) enum DispatchPolicy {
    FirstComeFirstServed,
    FixedPriority,
    ShortestJobFirst,
    ShortestRemainingTimeFirst,
    HighestResponseRatioNext,
    RateMonotonic,
    DeadlineMonotonic,
    EarliestDeadlineFirst,
    LeastLaxityFirst,
}

/// One CPU burst. All durations and timestamps use the same monotonic ticks.
#[derive(Clone, Copy)]
pub(crate) struct DispatchJob {
    pub(crate) release: u64,
    /// Original burst length, including service already received.
    pub(crate) service: u64,
    pub(crate) remaining: u64,
    /// Larger values have higher fixed priority.
    pub(crate) priority: u8,
    /// Zero denotes an unspecified period.
    pub(crate) period: u64,
    /// Relative to release. Zero denotes an unspecified deadline.
    pub(crate) relative_deadline: u64,
}

impl DispatchJob {
    /// Ready-task adapter for fixed priority when no burst estimate is tracked.
    pub(crate) const fn fixed_priority(priority: u8) -> Self {
        Self {
            release: 0,
            service: 1,
            remaining: 1,
            priority,
            period: 0,
            relative_deadline: 0,
        }
    }

    fn absolute_deadline(self) -> i128 {
        i128::from(self.release) + i128::from(self.relative_deadline)
    }

    fn laxity(self, now: u64) -> i128 {
        self.absolute_deadline() - i128::from(now) - i128::from(self.remaining)
    }

    fn wait(self, now: u64) -> u64 {
        // Eligibility establishes both subtraction bounds.
        (now - self.release) - (self.service - self.remaining)
    }
}

impl DispatchPolicy {
    /// Selects and removes one runnable job. Return None from `job` for a
    /// blocked or absent task. Future, finished or inconsistent jobs stay
    /// queued. Equal ranks keep the existing ready-queue order.
    pub(crate) fn pop_ready<const N: usize>(
        self,
        queue: &mut ReadyQueue<N>,
        now: u64,
        job: impl Fn(usize) -> Option<DispatchJob>,
    ) -> Option<usize> {
        queue.pop_ranked(
            |id| job(id).filter(|job| self.eligible(*job, now)),
            |candidate, best| self.prefers(candidate, best, now),
        )
    }

    fn eligible(self, job: DispatchJob, now: u64) -> bool {
        if job.release > now
            || job.remaining == 0
            || job.remaining > job.service
            || job.service - job.remaining > now - job.release
        {
            return false;
        }
        match self {
            Self::RateMonotonic => job.period != 0,
            Self::DeadlineMonotonic | Self::EarliestDeadlineFirst | Self::LeastLaxityFirst => {
                job.relative_deadline != 0
            }
            _ => true,
        }
    }

    fn prefers(self, candidate: DispatchJob, best: DispatchJob, now: u64) -> bool {
        match self {
            Self::FirstComeFirstServed => candidate.release < best.release,
            Self::FixedPriority => candidate.priority > best.priority,
            Self::ShortestJobFirst => candidate.service < best.service,
            Self::ShortestRemainingTimeFirst => candidate.remaining < best.remaining,
            Self::HighestResponseRatioNext => {
                // (wait + service) / service = 1 + wait / service. Dropping
                // the common 1 keeps each exact cross product within u128.
                u128::from(candidate.wait(now)) * u128::from(best.service)
                    > u128::from(best.wait(now)) * u128::from(candidate.service)
            }
            Self::RateMonotonic => candidate.period < best.period,
            Self::DeadlineMonotonic => candidate.relative_deadline < best.relative_deadline,
            Self::EarliestDeadlineFirst => candidate.absolute_deadline() < best.absolute_deadline(),
            Self::LeastLaxityFirst => candidate.laxity(now) < best.laxity(now),
        }
    }
}

/// Replay the dispatch decisions on target through the same queue as the
/// scheduler. Opaque input snapshots keep the rank comparisons in the image.
pub(crate) fn bench_checks() -> bool {
    use DispatchPolicy::*;

    let base = DispatchJob {
        release: 90,
        service: 8,
        remaining: 8,
        priority: 1,
        period: 200,
        relative_deadline: 150,
    };
    // At tick 100, each selector has a different unique best job.
    let jobs = core::hint::black_box([
        DispatchJob {
            release: 0,
            service: 32,
            remaining: 32,
            ..base
        },
        DispatchJob {
            priority: u8::MAX,
            ..base
        },
        DispatchJob {
            release: 100,
            service: 2,
            remaining: 2,
            ..base
        },
        DispatchJob {
            service: 10,
            remaining: 1,
            ..base
        },
        DispatchJob {
            release: 5,
            service: 3,
            remaining: 3,
            ..base
        },
        DispatchJob { period: 16, ..base },
        DispatchJob {
            release: 100,
            service: 3,
            remaining: 3,
            relative_deadline: 3,
            ..base
        },
        // Deadline 35, laxity -70 at tick 100.
        DispatchJob {
            release: 20,
            service: 5,
            remaining: 5,
            relative_deadline: 15,
            ..base
        },
        // Deadline 50, laxity -80 at tick 100.
        DispatchJob {
            release: 10,
            service: 30,
            remaining: 30,
            relative_deadline: 40,
            ..base
        },
    ]);
    for (winner, policy) in [
        FirstComeFirstServed,
        FixedPriority,
        ShortestJobFirst,
        ShortestRemainingTimeFirst,
        HighestResponseRatioNext,
        RateMonotonic,
        DeadlineMonotonic,
        EarliestDeadlineFirst,
        LeastLaxityFirst,
    ]
    .into_iter()
    .enumerate()
    {
        let mut queue = ReadyQueue::<9>::new();
        // Place each expected winner last so simply taking the head must fail.
        for offset in 1..=jobs.len() {
            if !queue.push_back((winner + offset) % jobs.len()) {
                return false;
            }
        }
        if policy.pop_ready(&mut queue, 100, |id| Some(jobs[id])) != Some(winner)
            || pick(policy, 100, [base, base], [1, 0]) != Some(1)
        {
            return false;
        }
    }

    // Service already received reduces actual waiting. The two wait/service
    // fractions are 5/6 and 2/2, although the first job arrived ten ticks ago.
    let partly_served = DispatchJob {
        release: 0,
        service: 6,
        remaining: 1,
        ..base
    };
    let waiting = DispatchJob {
        release: 8,
        service: 2,
        remaining: 2,
        ..base
    };
    if pick(
        HighestResponseRatioNext,
        10,
        [partly_served, waiting],
        [0, 1],
    ) != Some(1)
    {
        return false;
    }

    let huge = DispatchJob {
        service: u64::MAX,
        remaining: u64::MAX,
        ..DispatchJob::fixed_priority(1)
    };
    let shorter = DispatchJob {
        service: u64::MAX - 1,
        remaining: u64::MAX - 1,
        ..huge
    };
    if pick(HighestResponseRatioNext, u64::MAX, [huge, shorter], [0, 1]) != Some(1) {
        return false;
    }

    // Both deadlines exceed u64::MAX. Saturating them would create a tie.
    let far = DispatchJob {
        release: u64::MAX - 2,
        relative_deadline: 8,
        ..DispatchJob::fixed_priority(1)
    };
    let near = DispatchJob {
        relative_deadline: 4,
        ..far
    };
    for policy in [EarliestDeadlineFirst, LeastLaxityFirst] {
        if pick(policy, u64::MAX, [far, near], [0, 1]) != Some(1) {
            return false;
        }
    }

    let untimed = DispatchJob::fixed_priority(1);
    for policy in [
        RateMonotonic,
        DeadlineMonotonic,
        EarliestDeadlineFirst,
        LeastLaxityFirst,
    ] {
        if pick(policy, 0, [untimed, untimed], [1, 0]).is_some() {
            return false;
        }
    }

    let jobs = core::hint::black_box([
        None,
        Some(DispatchJob {
            remaining: 0,
            ..untimed
        }),
        Some(DispatchJob {
            service: 0,
            ..untimed
        }),
        Some(DispatchJob {
            service: 8,
            ..untimed
        }),
        Some(DispatchJob {
            release: 3,
            ..untimed
        }),
        Some(untimed),
    ]);
    let mut queue = ReadyQueue::<6>::new();
    for id in 0..jobs.len() {
        if !queue.push_back(id) {
            return false;
        }
    }
    let snapshot = |id: usize| jobs[id];
    FixedPriority.pop_ready(&mut queue, 2, snapshot) == Some(5)
        && FixedPriority.pop_ready(&mut queue, 2, snapshot).is_none()
        && FixedPriority.pop_ready(&mut queue, 3, snapshot) == Some(4)
        && queue.pop_preferred(|_, _| false) == Some(0)
        && queue.pop_highest(|id| id as u8) == Some(3)
}

fn pick(
    policy: DispatchPolicy,
    now: u64,
    jobs: [DispatchJob; 2],
    order: [usize; 2],
) -> Option<usize> {
    let jobs = core::hint::black_box(jobs);
    let mut queue = ReadyQueue::<2>::new();
    if !queue.push_back(order[0]) || !queue.push_back(order[1]) {
        return None;
    }
    policy.pop_ready(&mut queue, now, |id| Some(jobs[id]))
}
