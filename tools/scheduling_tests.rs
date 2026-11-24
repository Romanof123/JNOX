//! Native tests for the policy sources used by the kernel.

#[path = "../kernel/src/ready_queue.rs"]
mod ready_queue;

// Import each backend and register its guest check as a separate host test.
macro_rules! policy_tests {
    ($($module:ident => $path:literal),+ $(,)?) => {
        $(#[path = $path] mod $module;)+
        mod guest_checks {
            $(#[test]
            fn $module() {
                assert!(super::$module::bench_checks());
            })+
        }
    };
}

policy_tests! {
    dispatch_policy => "../kernel/src/dispatch_policy.rs",
    service_policy => "../kernel/src/service_policy.rs",
    temporal_policy => "../kernel/src/temporal_policy.rs",
    fair_policy => "../kernel/src/fair_policy.rs",
    feedback_policy => "../kernel/src/feedback_policy.rs",
    aperiodic_policy => "../kernel/src/aperiodic_policy.rs",
    domain_policy => "../kernel/src/domain_policy.rs",
    group_policy => "../kernel/src/group_policy.rs",
}

use dispatch_policy::{DispatchJob, DispatchPolicy};
use ready_queue::ReadyQueue;

// The fixture's positive burst lengths divide 60, so HRRN maps exactly onto
// this integer scale. Lower keys win. Signed arithmetic also represents overdue jobs.
fn small_rank(policy: DispatchPolicy, job: DispatchJob, now: u64) -> Option<i128> {
    use DispatchPolicy::*;
    let age = i128::from(now) - i128::from(job.release);
    let service = i128::from(job.service);
    let remaining = i128::from(job.remaining);
    let received = service - remaining;
    if remaining == 0 || received < 0 || age < received {
        return None;
    }
    let deadline = i128::from(job.relative_deadline);
    match policy {
        FirstComeFirstServed => Some(i128::from(job.release)),
        FixedPriority => Some(-i128::from(job.priority)),
        ShortestJobFirst => Some(service),
        ShortestRemainingTimeFirst => Some(remaining),
        HighestResponseRatioNext => {
            assert_eq!(60 % service, 0);
            Some(-(age + remaining) * (60 / service))
        }
        RateMonotonic => (job.period > 0).then_some(i128::from(job.period)),
        DeadlineMonotonic => (deadline > 0).then_some(deadline),
        EarliestDeadlineFirst => (deadline > 0).then_some(i128::from(job.release) + deadline),
        LeastLaxityFirst => (deadline > 0).then_some(deadline - age - remaining),
    }
}

#[test]
fn dispatch_matches_stable_sort_for_all_ordered_triples() {
    use DispatchPolicy::*;
    let job = |release, service, remaining, priority, period, relative_deadline| {
        Some(DispatchJob { release, service, remaining, priority, period, relative_deadline })
    };
    let catalog = [
        job(0, 6, 1, 2, 7, 2),
        job(1, 4, 3, 1, 3, 5),
        job(3, 2, 2, 3, 9, 3),
        job(5, 3, 3, 2, 4, 1),
        job(4, 1, 1, 4, 0, 7),
        job(2, 5, 4, 1, 6, 0),
        job(0, 1, 2, 4, 1, 1),
        job(0, 2, 0, 4, 1, 1),
        None,
    ];
    // Repetition includes equal ranks in each queue position. The stable sort
    // predicts the entire drain, including the order of IDs left ineligible.
    for policy in [
        FirstComeFirstServed, FixedPriority, ShortestJobFirst, ShortestRemainingTimeFirst,
        HighestResponseRatioNext, RateMonotonic, DeadlineMonotonic, EarliestDeadlineFirst,
        LeastLaxityFirst,
    ] {
        for now in [0, 2, 5, 10] {
            for a in 0..catalog.len() {
                for b in 0..catalog.len() {
                    for c in 0..catalog.len() {
                        let indices = [a, b, c];
                        let jobs = indices.map(|i| catalog[i]);
                        let ranks = jobs.map(|j| j.and_then(|j| small_rank(policy, j, now)));
                        let mut queue = ReadyQueue::<3>::new();
                        let order = [2, 0, 1];
                        for id in order {
                            assert!(queue.push_back(id));
                        }
                        let mut expected = order;
                        expected.sort_by_key(|&id| ranks[id]);
                        let pop = |q: &mut ReadyQueue<3>| policy.pop_ready(q, now, |id| jobs[id]);
                        for id in expected.into_iter().filter(|&id| ranks[id].is_some()) {
                            assert_eq!(pop(&mut queue), Some(id),
                                "policy {} tick {now} snapshot {indices:?}", policy as u8);
                        }
                        assert_eq!(pop(&mut queue), None);
                        for id in order.into_iter().filter(|&id| ranks[id].is_none()) {
                            assert_eq!(queue.pop_preferred(|_, _| false), Some(id));
                        }
                        assert_eq!(queue.pop_preferred(|_, _| false), None);
                    }
                }
            }
        }
    }
}

#[test]
fn hrrn_distinguishes_adjacent_ratios_at_the_clock_limit() {
    // Each response ratio is 2 - 1/service. They round to the same f64.
    let jobs = [u64::MAX - 1, u64::MAX].map(|service| DispatchJob {
        release: u64::MAX - service + 1,
        remaining: service,
        service,
        ..DispatchJob::fixed_priority(0)
    });
    for order in [[0, 1], [1, 0]] {
        let mut queue = ReadyQueue::<2>::new();
        for id in order {
            assert!(queue.push_back(id));
        }
        let winner = DispatchPolicy::HighestResponseRatioNext
            .pop_ready(&mut queue, u64::MAX, |id| Some(jobs[id]));
        assert_eq!(winner, Some(1));
    }
}

#[test]
fn weighted_and_eligible_deadline_selection_in_twelfths() {
    use fair_policy::{FairRequest, pick_eevdf, pick_weighted, weighted_reference};
    // For these weights a virtual tick is exactly twelve integer units.
    let specs = [
        (1u32, 1u64, 0u32, 2u64, 1u64),
        (2, 0, 1, 3, 2),
        (3, 0, 1, 5, 2),
        (4, 1, 2, 1, 0),
        (2, 1, 0, 1, 0),
    ];
    let requests = specs.map(|(w, start, rem, size, used)| {
        FairRequest::new(w, start, rem, size, used).unwrap()
    });
    for a in 0..specs.len() {
        for b in 0..specs.len() {
            for c in 0..specs.len() {
                let indices = [a, b, c];
                let ready = indices.map(|i| requests[i]);
                let times = indices.map(|i| {
                    let (w, start, rem, size, used) = specs[i];
                    let weight = u64::from(w);
                    let unit = 12 / weight;
                    let base = 12 * start + u64::from(rem) * unit;
                    (weight, base + used * unit, base + size * unit)
                });
                let weights: u64 = times.iter().map(|&(w, _, _)| w).sum();
                let sum: u64 = times.iter().map(|&(w, time, _)| w * time).sum();
                let minimum = (0..3).min_by_key(|&i| times[i].1);
                let eligible = (0..3).filter(|&i| times[i].1 * weights <= sum);
                let earliest = eligible.min_by_key(|&i| times[i].2);
                assert_eq!(pick_weighted(&ready), minimum, "snapshot {indices:?}");
                let reference = weighted_reference(&ready).unwrap();
                assert_eq!(pick_eevdf(&ready, reference), earliest, "snapshot {indices:?}");
            }
        }
    }
}

#[test]
fn saturated_hard_server_runs_only_its_quota_each_period() {
    use temporal_policy::HardCbs;
    for period in 1..=9 {
        for quota in 1..=period {
            for start in [0, 7] {
                let mut server = HardCbs::new(start, [(quota, period)]).unwrap();
                server.release(0, 4 * quota).unwrap();
                for elapsed in 0..4 * period {
                    let expected = (elapsed % period < quota).then_some(0);
                    assert_eq!(server.tick(), Ok(expected),
                        "Q={quota} P={period} start={start} elapsed={elapsed}");
                }
                assert_eq!(server.tick(), Ok(None));
            }
        }
    }
}

#[test]
fn cbs_admission_and_service_match_demand_over_common_frame() {
    use temporal_policy::HardCbs;

    // Sixty ticks is divisible by every tested period. Count requested CPU
    // ticks directly instead of adding fractions as the constructor does.
    const FRAME: u32 = 60;
    let choices: Vec<_> = (1..=5)
        .flat_map(|period| (1..=period).map(move |quota| (quota, period)))
        .collect();
    for &a in &choices {
        for &b in &choices {
            for &c in &choices {
                let reservations = [a, b, c];
                let demand: u32 = reservations.iter().map(|(q, p)| q * (FRAME / p)).sum();
                let schedule = HardCbs::new(7, reservations);
                assert_eq!(schedule.is_some(), demand <= FRAME, "{reservations:?}");
                let Some(mut schedule) = schedule else { continue };
                for id in 0..reservations.len() {
                    schedule.release(id, FRAME * 2 + 1).unwrap();
                }
                let mut consumed = [0; 3];
                for elapsed in 1..=FRAME * 2 {
                    if let Some(id) = schedule.tick().unwrap() {
                        consumed[id] += 1;
                    }
                    for (id, (quota, period)) in reservations.iter().enumerate() {
                        assert!(consumed[id] <= *quota, "{reservations:?} at {elapsed}");
                        if elapsed % period == 0 {
                            assert_eq!(consumed[id], *quota, "{reservations:?} at {elapsed}");
                            consumed[id] = 0;
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn cyclic_decisions_match_expanded_slots() {
    use temporal_policy::{CyclicExecutive, WindowDecision};

    let choices: Vec<_> = [None, Some(0), Some(1)].into_iter()
        .flat_map(|owner| (1..=3).map(move |ticks| (owner, ticks)))
        .collect();
    for &a in &choices {
        for &b in &choices {
            for &c in &choices {
                let slots = [a, b, c];
                let frame = a.1 + b.1 + c.1;
                let table = CyclicExecutive::new(2, frame, slots).unwrap();
                let mut start = 0;
                for (owner, duration) in slots.into_iter().cycle().take(9) {
                    let end_tick = start + duration;
                    for tick in start..end_tick {
                        for mask in 0..4 {
                            let mut queried = None;
                            let decision = table.decision(tick, |id| {
                                queried = Some(id);
                                mask & (1 << id) != 0
                            });
                            let runnable = owner.filter(|&id| mask & (1 << id) != 0);
                            assert_eq!(queried, owner);
                            assert_eq!(decision, Some(WindowDecision { owner, runnable, end_tick }),
                                "slots={slots:?} tick={tick} mask={mask}");
                        }
                    }
                    start = end_tick;
                }
            }
        }
    }
}

#[test]
fn cyclic_window_end_at_clock_limit() {
    use temporal_policy::CyclicExecutive;

    let table = CyclicExecutive::new(1, u64::MAX, [(Some(0), u64::MAX)]).unwrap();
    assert_eq!(table.decision(u64::MAX - 1, |_| true).unwrap().end_tick, u64::MAX);
    assert_eq!(table.decision(u64::MAX, |_| true), None);
    assert_eq!(table.dispatch(u64::MAX, |_| true), Some(0));

    let table = CyclicExecutive::new(1, 4, [(Some(0), 4)]).unwrap();
    assert_eq!(table.decision(u64::MAX - 3, |_| true), None);
    assert_eq!(table.dispatch(u64::MAX - 3, |_| true), Some(0));
}
