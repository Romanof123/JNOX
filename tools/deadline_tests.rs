//! Host oracles for fixed-quantum EDF and LLF decisions.

use super::deadline_policy::{DeadlineError, DeadlinePolicy, DeadlineScheduler};

#[derive(Clone, Copy, Debug)]
enum Rule {
    Edf,
    Llf,
}

const RULES: [Rule; 2] = [Rule::Edf, Rule::Llf];

impl Rule {
    fn scheduler<const N: usize>(self, start: u64, quantum: u64) -> Option<DeadlineScheduler<N>> {
        let policy = match self {
            Self::Edf => DeadlinePolicy::Edf,
            Self::Llf => DeadlinePolicy::Llf,
        };
        DeadlineScheduler::new(policy, start, quantum)
    }
}

#[derive(Debug)]
struct Calendar {
    arrival: [u64; 3],
    estimate: [u64; 3],
    deadline: [u64; 3],
    ids: [usize; 3],
}

impl Calendar {
    fn compare(&self, rule: Rule, quantum: u64) {
        const START: u64 = 5;
        // Job 0 outlives its estimate, and job 1 may complete early.
        let actual = [self.estimate[0] + 1, 2, self.estimate[2]];
        let mut scheduler = rule.scheduler::<3>(START, quantum).unwrap();
        let mut used = [0u64; 3];
        let mut waiting = Vec::new();
        let mut running: Option<usize> = None;
        let end = self.arrival[2] + actual.iter().sum::<u64>();
        for elapsed in 0..=end {
            let now = START + elapsed;
            if let Some(i) = running {
                used[i] += 1;
                if used[i] == actual[i] {
                    scheduler.complete(now, self.ids[i]).unwrap();
                    running = None;
                }
            }
            for i in 0..3 {
                if self.arrival[i] == elapsed {
                    scheduler
                        .release(now, self.ids[i], self.estimate[i], self.deadline[i])
                        .unwrap();
                    waiting.push(i);
                }
            }
            let remaining = |i: usize| self.estimate[i].saturating_sub(used[i]).max(1);
            if elapsed % quantum == 0 {
                if let Some(i) = running.take() {
                    waiting.insert(0, i);
                }
            }
            if running.is_none() {
                // Latest start = deadline - outstanding CPU ticks. The common
                // current time cannot affect the ordering of these signed keys.
                let winner = (0..waiting.len()).min_by_key(|&position| {
                    let i = waiting[position];
                    let deadline = i128::from(self.deadline[i]);
                    match rule {
                        Rule::Edf => deadline,
                        Rule::Llf => deadline - i128::from(remaining(i)),
                    }
                });
                running = winner.map(|position| waiting.remove(position));
            }
            let chosen = scheduler.decide(now).unwrap();
            assert_eq!(
                chosen,
                running.map(|i| self.ids[i]),
                "{rule:?} q={quantum}, {now}: {self:?}"
            );
            assert_eq!(scheduler.now(), now);
            for i in 0..3 {
                let live = self.arrival[i] <= elapsed && used[i] < actual[i];
                let debt = live.then(|| i128::from(remaining(i)));
                assert_eq!(
                    scheduler.remaining(self.ids[i]).map(i128::from),
                    debt,
                    "remaining: {rule:?} q={quantum}, {now}: {self:?}"
                );
                let slack =
                    debt.map(|ticks| i128::from(self.deadline[i]) - i128::from(now) - ticks);
                assert_eq!(
                    scheduler.laxity(self.ids[i]),
                    slack,
                    "laxity: {rule:?} q={quantum}, {now}: {self:?}"
                );
            }
        }
        assert_eq!(used, actual);
    }
}

#[test]
fn ordered_arrivals_on_small_tick_calendars() {
    const IDS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for rule in RULES {
        for quantum in 1..=3 {
            for second in 0..=3 {
                for third in second..=3 {
                    for mask in 0u8..8 {
                        let estimate =
                            [0, 1, 2].map(|bit| if mask & (1 << bit) == 0 { 1 } else { 3 });
                        for deadline in [[0, 5, 6], [8, 8, 8], [13, 9, 11], [11, 12, 10]] {
                            for ids in IDS {
                                Calendar {
                                    arrival: [0, second, third],
                                    estimate,
                                    deadline,
                                    ids,
                                }
                                .compare(rule, quantum);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn one_consideration_per_boundary_including_retention_and_idle_dispatch() {
    for rule in RULES {
        let mut scheduler = rule.scheduler::<4>(5, 4).unwrap();
        scheduler.release(5, 3, 20, 99).unwrap();
        assert_eq!(scheduler.decide(5).unwrap(), Some(3));
        scheduler.release(5, 2, 4, 18).unwrap();
        assert_eq!(scheduler.decide(5).unwrap(), Some(3));
        scheduler.release(7, 1, 4, 10).unwrap();
        assert_eq!(scheduler.decide(8).unwrap(), Some(3));
        assert_eq!(scheduler.decide(9).unwrap(), Some(1));
        assert_eq!(scheduler.decide(13).unwrap(), Some(1)); // retains and consumes this boundary
        scheduler.release(13, 0, 8, 0).unwrap();
        assert_eq!(scheduler.decide(13).unwrap(), Some(1));
        scheduler.complete(14, 1).unwrap();
        assert!(matches!(
            scheduler.complete(14, 0),
            Err(DeadlineError::NotRunning)
        ));
        assert_eq!(scheduler.decide(15).unwrap(), Some(0)); // idle dispatch between boundaries
    }
}

#[test]
fn a_switch_consumes_the_boundary_and_skipped_boundaries_are_not_replayed() {
    for rule in RULES {
        let mut scheduler = rule.scheduler::<3>(0, 4).unwrap();
        scheduler.release(0, 2, 20, 80).unwrap();
        assert_eq!(scheduler.decide(0).unwrap(), Some(2));
        scheduler.release(1, 1, 7, 20).unwrap();
        // Boundaries 4 and 8 pass without a dispatch call. At 10 job 2 still owns the CPU.
        assert_eq!(scheduler.decide(10).unwrap(), Some(2));
        assert_eq!(scheduler.remaining(2), Some(10));
        assert_eq!(scheduler.decide(12).unwrap(), Some(1));
        scheduler.release(12, 0, 9, 1).unwrap();
        assert_eq!(scheduler.decide(12).unwrap(), Some(1));
        assert_eq!(scheduler.decide(14).unwrap(), Some(1));
        assert_eq!(scheduler.decide(16).unwrap(), Some(0));
    }
}

#[test]
fn llf_retains_the_incumbent_at_equal_slack_then_reverses() {
    let mut scheduler = Rule::Llf.scheduler::<2>(7, 3).unwrap();
    scheduler.release(7, 1, 7, 25).unwrap();
    scheduler.release(7, 0, 3, 24).unwrap();
    assert_eq!(scheduler.decide(7).unwrap(), Some(1));
    assert_eq!(scheduler.decide(10).unwrap(), Some(1));
    assert_eq!([0, 1].map(|i| scheduler.laxity(i)), [Some(11), Some(11)]);
    assert_eq!(scheduler.decide(13).unwrap(), Some(0));
    assert_eq!([0, 1].map(|i| scheduler.laxity(i)), [Some(8), Some(11)]);
}

#[test]
fn preempted_runner_resumes_ahead_of_an_equal_waiter() {
    for rule in RULES {
        let mut scheduler = rule.scheduler::<3>(0, 3).unwrap();
        scheduler.release(0, 2, 5, 20).unwrap();
        assert_eq!(scheduler.decide(0).unwrap(), Some(2));
        scheduler.release(1, 0, 2, 20).unwrap();
        scheduler.release(2, 1, 1, 0).unwrap();
        assert_eq!(scheduler.decide(3).unwrap(), Some(1));
        scheduler.complete(4, 1).unwrap();
        assert_eq!(scheduler.remaining(2), Some(2));
        assert_eq!(scheduler.laxity(2), scheduler.laxity(0));
        assert_eq!(scheduler.decide(4).unwrap(), Some(2));
    }
}

#[test]
fn rejected_calls_preserve_time_service_queue_and_boundary() {
    use DeadlineError::*;
    for rule in RULES {
        let mut scheduler = rule.scheduler::<4>(4, 3).unwrap();
        scheduler.release(4, 2, 8, 20).unwrap();
        assert_eq!(scheduler.decide(4).unwrap(), Some(2));
        scheduler.release(5, 3, 5, 10).unwrap();
        scheduler.release(5, 0, 5, 10).unwrap();
        assert!(matches!(scheduler.release(7, 4, 1, 0), Err(InvalidId)));
        assert!(matches!(scheduler.complete(7, usize::MAX), Err(InvalidId)));
        assert!(matches!(scheduler.release(7, 1, 0, 0), Err(ZeroEstimate)));
        assert!(matches!(scheduler.release(7, 3, 1, 0), Err(Duplicate)));
        assert!(matches!(scheduler.release(7, 2, 1, 0), Err(Duplicate)));
        assert!(matches!(scheduler.complete(7, 0), Err(NotRunning)));
        assert!(matches!(scheduler.complete(7, 1), Err(NotRunning)));
        assert!(matches!(scheduler.release(4, 1, 1, 0), Err(TimeRegression)));
        assert!(matches!(scheduler.complete(4, 2), Err(TimeRegression)));
        assert!(matches!(scheduler.decide(4), Err(TimeRegression)));
        assert_eq!(scheduler.now(), 5);
        assert_eq!(scheduler.remaining(2), Some(7));
        assert_eq!(scheduler.laxity(2), Some(8));
        assert_eq!(scheduler.laxity(usize::MAX), None);
        assert_eq!(scheduler.remaining(1), None);
        // The first valid decision at 7 can still preempt. The equal rank
        // goes to job 3, which arrived before job 0.
        assert_eq!(scheduler.decide(7).unwrap(), Some(3));
        scheduler.complete(7, 3).unwrap();
        scheduler.release(7, 3, 5, 10).unwrap();
        assert_eq!(scheduler.decide(7).unwrap(), Some(0));
        scheduler.complete(7, 0).unwrap();
        assert_eq!(scheduler.decide(7).unwrap(), Some(3));
        scheduler.complete(7, 3).unwrap();
        assert_eq!(scheduler.decide(7).unwrap(), Some(2));
        assert_eq!(scheduler.remaining(2), Some(5));
    }
}

#[test]
fn zero_capacity_and_zero_quantum() {
    for rule in RULES {
        assert!(rule.scheduler::<2>(9, 0).is_none());
        let mut empty = rule.scheduler::<0>(9, 2).unwrap();
        assert!(matches!(
            empty.release(10, 0, 1, 0),
            Err(DeadlineError::InvalidId)
        ));
        assert!(matches!(
            empty.complete(10, 0),
            Err(DeadlineError::InvalidId)
        ));
        assert_eq!(empty.now(), 9);
        assert_eq!(empty.decide(u64::MAX).unwrap(), None);
        assert_eq!(empty.remaining(0), None);
        assert_eq!(empty.laxity(0), None);
    }
}

#[test]
fn laxity_retains_full_signed_range_for_waiting_jobs() {
    let max = u64::MAX;
    for rule in RULES {
        let mut scheduler = rule.scheduler::<2>(0, max).unwrap();
        scheduler.release(0, 0, 1, max).unwrap();
        scheduler.release(0, 1, max, 0).unwrap();
        assert_eq!(scheduler.laxity(0), Some(i128::from(max) - 1));
        assert_eq!(scheduler.laxity(1), Some(-i128::from(max)));
        assert_eq!(scheduler.decide(max).unwrap(), Some(1));
        assert_eq!(scheduler.laxity(0), Some(-1));
        assert_eq!(scheduler.laxity(1), Some(-2 * i128::from(max)));
        assert_eq!(
            scheduler.remaining(1).map(i128::from),
            Some(i128::from(max))
        );
    }
}

#[test]
fn epoch_near_clock_limit_keeps_only_representable_boundaries() {
    let max = u64::MAX;
    for rule in RULES {
        let mut scheduler = rule.scheduler::<3>(max - 4, 3).unwrap();
        scheduler.release(max - 4, 2, 8, max).unwrap();
        assert_eq!(scheduler.decide(max - 4).unwrap(), Some(2));
        scheduler.release(max - 3, 1, 8, 1).unwrap();
        assert_eq!(scheduler.decide(max - 2).unwrap(), Some(2));
        assert_eq!(scheduler.decide(max - 1).unwrap(), Some(1));
        scheduler.release(max, 0, 16, 0).unwrap();
        assert_eq!(scheduler.decide(max).unwrap(), Some(1));
        assert_eq!(scheduler.remaining(1), Some(7));
        scheduler.complete(max, 1).unwrap();
        assert_eq!(scheduler.decide(max).unwrap(), Some(0));
    }
}

#[test]
fn suspended_job_keeps_deadline_and_cost_and_a_mid_grid_wake_waits_for_arbitration() {
    let mut m = DeadlineScheduler::<2>::new(DeadlinePolicy::Edf, 0, 1).unwrap();
    m.release(0, 0, 6, 9).unwrap();
    m.release(0, 1, 3, 5).unwrap();
    assert_eq!(m.decide(0), Ok(Some(1)));
    assert_eq!(m.suspend(1, 1), Ok(()));
    assert_eq!(m.decide(1), Ok(Some(0)));
    assert_eq!(m.resume(3, 1), Ok(()));
    assert_eq!(m.resume(4, 1), Err(DeadlineError::NotSuspended));
    assert_eq!(m.suspend(4, 1), Err(DeadlineError::NotRunning));
    assert_eq!(m.yield_running(2, 0), Err(DeadlineError::TimeRegression));
    assert_eq!(m.now(), 3);
    assert_eq!(m.remaining(0), Some(4));
    assert_eq!(m.remaining(1), Some(2));
    assert_eq!(m.decide_unsampled(3), Ok(Some(0)));
    assert_eq!(m.decide(4), Ok(Some(1)));
}

#[test]
fn yielding_owner_joins_waiting_equal_deadlines_and_can_win_again_alone() {
    let mut m = DeadlineScheduler::<2>::new(DeadlinePolicy::Edf, 0, 1).unwrap();
    m.release(0, 1, 2, 5).unwrap();
    m.release(0, 0, 2, 5).unwrap();
    assert_eq!(m.decide(0), Ok(Some(1)));
    m.yield_running(0, 1).unwrap();
    assert_eq!(m.decide(0), Ok(Some(0)));
    m.complete(1, 0).unwrap();
    assert_eq!(m.decide(1), Ok(Some(1)));
    m.yield_running(1, 1).unwrap();
    assert_eq!(m.decide(1), Ok(Some(1)));
}

#[test]
fn native_edf_checks_the_last_cell_and_admits_an_exact_deadline_finish() {
    use super::deadline_policy::{admit_edf, NumericJob, Refusal};
    let job = NumericJob { id: 0, release: 14, deadline: 16, cost: 2 };
    assert_eq!(admit_edf(&[job], 10, u64::MAX - 16), Ok(()));
    assert_eq!(admit_edf(&[job], 10, u64::MAX - 15), Err(Refusal::Overflow));
    assert_eq!(admit_edf(&[job], 11, 0), Err(Refusal::Grid));
    assert_eq!(admit_edf(&[NumericJob { cost: 3, ..job }], 10, 0), Err(Refusal::Demand));
    assert_eq!(admit_edf(&[NumericJob { id: 1, ..job }], 10, 0), Err(Refusal::Table));
}
