//! Native tests for the policy sources used by the kernel.

#[path = "../kernel/src/fixed_policy.rs"]
mod fixed_policy;
#[path = "fixed_runtime_tests.rs"]
mod fixed_runtime_tests;
#[path = "../kernel/src/ready_queue.rs"]
mod ready_queue;
#[path = "../kernel/src/smp_policy.rs"]
mod smp_policy;
#[path = "smp_tests.rs"]
mod smp_tests;
#[path = "../kernel/src/mutex_anchor_policy.rs"]
mod mutex_anchor_policy;
#[path = "../kernel/src/mutex_policy.rs"]
mod mutex_policy;
#[path = "mutex_tests.rs"]
mod mutex_tests;
#[path = "../kernel/src/amc_policy.rs"]
mod amc_policy;
#[path = "amc_tests.rs"]
mod amc_tests;
#[path = "../kernel/src/multilevel_policy.rs"]
mod multilevel_policy;
#[path = "multilevel_oracle.rs"]
mod multilevel_oracle;
#[path = "../kernel/src/proportional_policy.rs"]
mod proportional_policy;
#[path = "proportional_tests.rs"]
mod proportional_tests;
#[path = "../kernel/src/vruntime_policy.rs"]
mod vruntime_policy;
#[path = "vruntime_tests.rs"]
mod vruntime_tests;
#[path = "../kernel/src/eevdf_policy.rs"]
mod eevdf_policy;
#[path = "eevdf_tests.rs"]
mod eevdf_tests;
#[path = "../kernel/src/hierarchy_policy.rs"]
mod hierarchy_policy;
#[path = "hierarchy_tests.rs"]
mod hierarchy_tests;
#[path = "hierarchy_oracle.rs"]
mod hierarchy_oracle;
#[path = "../kernel/src/power_policy.rs"]
mod power_policy;
#[path = "power_tests.rs"]
mod power_tests;
#[path = "power_oracle.rs"]
mod power_oracle;
#[path = "../kernel/src/extension_policy.rs"]
mod extension_policy;
#[path = "extension_tests.rs"]
mod extension_tests;

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

#[path = "../kernel/src/event_loop.rs"]
mod event_loop;

#[test]
fn cooperative_turns_preserve_fifo_across_wraparound() {
    let mut events = event_loop::EventLoop::<u8, 3>::new();
    for event in [1, 2, 3] {
        assert_eq!(events.enqueue(event), Ok(()));
    }
    assert_eq!(events.enqueue(99), Err(99));

    let mut trace = Vec::new();
    for expected in 1..=6 {
        assert!(events.dispatch_one(|event, enqueue| {
            trace.push(event);
            if event <= 3 {
                assert_eq!(enqueue(event + 3), Ok(()));
                assert_eq!(enqueue(98), Err(98));
            }
        }));
        assert_eq!(trace.len(), expected);
    }
    assert_eq!(trace, [1, 2, 3, 4, 5, 6]);
    assert!(!events.dispatch_one(|_, _| panic!("empty loop ran a handler")));
}

#[test]
fn cooperative_events_can_repeat_and_enqueue_multiple_successors() {
    let mut events = event_loop::EventLoop::<u8, 4>::new();
    events.enqueue(1).unwrap();
    events.enqueue(2).unwrap();
    assert!(events.dispatch_one(|event, enqueue| {
        assert_eq!(event, 1);
        for next in [1, 3, 1] {
            enqueue(next).unwrap();
        }
    }));
    for expected in [2, 1, 3, 1] {
        assert!(events.dispatch_one(|event, _| assert_eq!(event, expected)));
    }
    assert!(!events.dispatch_one(|_, _| unreachable!()));
}

#[test]
fn zero_capacity_returns_owned_event() {
    let mut events = event_loop::EventLoop::<String, 0>::new();
    assert_eq!(events.enqueue(String::from("pending")).unwrap_err(), "pending");
    assert!(!events.dispatch_one(|_, _| unreachable!()));
}

#[path = "../kernel/src/batch_policy.rs"]
mod batch_policy;

mod batch_tests {
    use super::batch_policy::{BatchError, BatchPolicy, BatchScheduler};

    #[derive(Clone, Copy, Debug)]
    enum Rule {
        Sjf,
        Srtf,
        Hrrn,
    }

    const RULES: [Rule; 3] = [Rule::Sjf, Rule::Srtf, Rule::Hrrn];

    impl Rule {
        fn scheduler<const N: usize>(self, start: u64) -> BatchScheduler<N> {
            let policy = match self {
                Rule::Sjf => BatchPolicy::Sjf,
                Rule::Srtf => BatchPolicy::Srtf,
                Rule::Hrrn => BatchPolicy::Hrrn,
            };
            BatchScheduler::new(policy, start)
        }
    }

    // Account one observed tick at a time. Jobs here are identified by arrival
    // position. Only the backend sees their (independently permuted) public IDs.
    fn check_schedule(
        rule: Rule,
        ids: [usize; 3],
        arrival: [u64; 3],
        estimate: [u64; 3],
        actual: [u64; 3],
    ) {
        let mut scheduler = rule.scheduler::<3>(0);
        let mut ready = Vec::new();
        let mut runner: Option<usize> = None;
        let mut served = [0u64; 3];
        let end = arrival[2] + actual.iter().sum::<u64>();
        for now in 0..=end {
            if let Some(i) = runner {
                served[i] += 1;
                if served[i] == actual[i] {
                    assert!(scheduler.complete(now, ids[i]).is_ok());
                    runner = None;
                }
            }
            for i in 0..3 {
                if arrival[i] == now {
                    assert!(scheduler.release(now, ids[i], estimate[i]).is_ok());
                    ready.push(i);
                }
            }
            // Put the incumbent first in stable order when reconsidering SRTF.
            if matches!(rule, Rule::Srtf) {
                if let Some(i) = runner.take() {
                    ready.insert(0, i);
                }
            }
            let remaining = |i: usize| estimate[i].saturating_sub(served[i]).max(1);
            if runner.is_none() {
                let best = (0..ready.len()).min_by_key(|&position| {
                    let i = ready[position];
                    match rule {
                        Rule::Sjf => estimate[i] as i64,
                        Rule::Srtf => remaining(i) as i64,
                        // Estimates 1, 2, 3 divide six. An HRRN waiter has
                        // never run because its incumbent is nonpreemptible.
                        Rule::Hrrn => -(((now - arrival[i]) * 6 / estimate[i]) as i64),
                    }
                });
                runner = best.map(|position| ready.remove(position));
            }
            let observed = scheduler.decide(now).expect("valid monotonic decision");
            assert_eq!(
                observed,
                runner.map(|i| ids[i]),
                "{rule:?} tick {now}, IDs {ids:?}, arrivals {arrival:?}, estimates {estimate:?}, actual {actual:?}"
            );
            assert_eq!(scheduler.now(), now);
            for i in 0..3 {
                let live = arrival[i] <= now && served[i] < actual[i];
                assert_eq!(
                    scheduler.remaining(ids[i]),
                    live.then(|| remaining(i)),
                    "{rule:?} tick {now}, job {i}, arrivals {arrival:?}, estimates {estimate:?}, actual {actual:?}"
                );
            }
        }
        assert_eq!(served, actual);
    }

    #[test]
    fn all_small_arrivals_estimates_and_id_orders_match_a_tick_calendar() {
        const ORDERS: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for rule in RULES {
            for second in 0..=3 {
                for third in second..=3 {
                    for code in 0..27 {
                        let estimate = [1 + code / 9, 1 + (code / 3) % 3, 1 + code % 3];
                        for ids in ORDERS {
                            for actual in [estimate, [estimate[0] + 2, 1, estimate[2] + 1]] {
                                check_schedule(rule, ids, [0, second, third], estimate, actual);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn release_and_completion_charge_elapsed_time_without_dispatch() {
        for rule in RULES {
            let mut scheduler = rule.scheduler::<3>(4);
            scheduler.release(5, 2, 10).unwrap();
            scheduler.release(8, 1, 20).unwrap();
            assert_eq!(scheduler.remaining(2), Some(10));
            assert_eq!(scheduler.decide(8).unwrap(), Some(2));
            scheduler.release(12, 0, 1).unwrap();
            assert_eq!(scheduler.remaining(2), Some(6));
            scheduler.complete(14, 2).unwrap();
            for waiting in [0, 1] {
                assert!(matches!(
                    scheduler.complete(16, waiting),
                    Err(BatchError::NotRunning)
                ));
            }
            scheduler.release(18, 2, 3).unwrap();
            assert_eq!(scheduler.remaining(0), Some(1));
            assert_eq!(scheduler.remaining(1), Some(20));
            assert_eq!(scheduler.remaining(2), Some(3));
            assert_eq!(scheduler.decide(25).unwrap(), Some(0));
            scheduler.complete(30, 0).unwrap();
            assert_eq!(scheduler.remaining(1), Some(20));
            assert_eq!(scheduler.remaining(2), Some(3));
        }
    }

    #[test]
    fn failed_operations_preserve_clock_budgets_runner_and_fifo_order() {
        use BatchError::*;
        for rule in RULES {
            let mut scheduler = rule.scheduler::<4>(5);
            scheduler.release(5, 3, 3).unwrap();
            assert_eq!(scheduler.decide(5).unwrap(), Some(3));
            scheduler.release(6, 2, 6).unwrap();
            scheduler.release(6, 0, 6).unwrap();

            assert!(matches!(
                scheduler.release(99, usize::MAX, 1),
                Err(InvalidId)
            ));
            assert!(matches!(scheduler.complete(99, 4), Err(InvalidId)));
            assert!(matches!(scheduler.release(99, 1, 0), Err(ZeroEstimate)));
            assert!(matches!(scheduler.release(99, 3, 1), Err(Duplicate)));
            assert!(matches!(scheduler.release(99, 2, 1), Err(Duplicate)));
            assert!(matches!(scheduler.complete(99, 0), Err(NotRunning)));
            assert!(matches!(scheduler.complete(99, 1), Err(NotRunning)));
            assert!(matches!(scheduler.complete(5, 3), Err(TimeRegression)));
            assert!(matches!(scheduler.release(5, 1, 1), Err(TimeRegression)));
            assert!(matches!(scheduler.decide(5), Err(TimeRegression)));

            assert_eq!(scheduler.now(), 6);
            assert_eq!(
                [0, 1, 2, 3, usize::MAX].map(|i| scheduler.remaining(i)),
                [Some(6), None, Some(6), Some(2), None]
            );
            assert_eq!(scheduler.decide(7).unwrap(), Some(3));
            scheduler.complete(8, 3).unwrap();
            scheduler.release(8, 3, 6).unwrap();
            for id in [2, 0, 3] {
                assert_eq!(scheduler.decide(9).unwrap(), Some(id));
                scheduler.complete(9, id).unwrap();
            }
            assert_eq!(scheduler.decide(10).unwrap(), None);
        }
    }

    #[test]
    fn empty_capacity_remains_idle_through_clock_limit() {
        for rule in RULES {
            let mut scheduler = rule.scheduler::<0>(8);
            assert!(matches!(
                scheduler.release(9, 0, 1),
                Err(BatchError::InvalidId)
            ));
            assert!(matches!(
                scheduler.complete(9, 0),
                Err(BatchError::InvalidId)
            ));
            assert_eq!(scheduler.remaining(0), None);
            assert_eq!(scheduler.now(), 8);
            assert_eq!(scheduler.decide(u64::MAX).unwrap(), None);
            assert!(matches!(
                scheduler.decide(u64::MAX - 1),
                Err(BatchError::TimeRegression)
            ));
            assert_eq!(scheduler.now(), u64::MAX);
        }
    }

    #[test]
    fn hrrn_resolves_a_unit_difference_between_full_width_products() {
        let end = u64::MAX;
        let mut scheduler = Rule::Hrrn.scheduler::<2>(0);
        scheduler.release(1, 1, end - 2).unwrap();
        scheduler.release(2, 0, end - 3).unwrap();
        // At MAX, wait = estimate + 1 for each. The smaller estimate's
        // response ratio 2 + 1/estimate wins despite its later arrival.
        assert_eq!(scheduler.decide(end).unwrap(), Some(0));
        scheduler.complete(end, 0).unwrap();
        assert_eq!(scheduler.decide(end).unwrap(), Some(1));
    }

    #[test]
    fn clock_sized_charge_preserves_one_tick_until_explicit_completion() {
        for rule in RULES {
            let mut scheduler = rule.scheduler::<2>(0);
            scheduler.release(0, 1, u64::MAX).unwrap();
            assert_eq!(scheduler.decide(0).unwrap(), Some(1));
            scheduler.release(u64::MAX - 1, 0, 1).unwrap();
            assert_eq!(scheduler.remaining(1), Some(1));
            assert_eq!(scheduler.decide(u64::MAX).unwrap(), Some(1));
            assert_eq!(scheduler.remaining(1), Some(1));
            scheduler.complete(u64::MAX, 1).unwrap();
            assert_eq!(scheduler.decide(u64::MAX).unwrap(), Some(0));
        }
    }
}

#[path = "../kernel/src/deadline_policy.rs"]
mod deadline_policy;
#[path = "deadline_tests.rs"]
mod deadline_tests;

#[path = "../kernel/src/request_server.rs"]
mod request_server;
#[path = "request_server_tests.rs"]
mod request_server_tests;

#[test]
fn late_batch_release_charges_incumbent_and_preserves_contractual_wait() {
    use batch_policy::{BatchPolicy, BatchScheduler};
    let mut batch = BatchScheduler::<3>::new(BatchPolicy::Hrrn, 0);
    batch.release(0, 0, 100).unwrap();
    assert_eq!(batch.decide(0), Ok(Some(0)));
    // Earlier job has waited 19/100 service estimates, later job 10/20.
    // Erasing their distinct due times would choose slot 1 by insertion order.
    batch.release_due(20, 1, 1, 100).unwrap();
    batch.release_due(20, 10, 2, 20).unwrap();
    assert_eq!(batch.remaining(0), Some(80));
    batch.complete(20, 0).unwrap();
    assert_eq!(batch.decide(20), Ok(Some(2)));
}

#[path = "../kernel/src/ordinary_mutex_manifest.rs"]
mod ordinary_mutex_manifest;

#[test]
fn ordinary_mutex_manifests_transfer_the_protected_resource_and_restore_priority() {
    use mutex_policy::Acquire::{Blocked,Held};
    for ceiling in [false,true] {
        let mut locks=ordinary_mutex_manifest::locks(ceiling).unwrap();
        assert_eq!(locks.acquire(0,0),Some(Held));
        let resource=usize::from(ceiling);
        if ceiling { assert_eq!(locks.effective(0),Some(5)); assert_eq!(locks.acquire(0,1),Some(Held)); }
        assert_eq!(locks.effective(0),Some(if ceiling {9}else{2}));
        assert_eq!(locks.acquire(2,resource),Some(Blocked));
        assert_eq!(locks.effective(0),Some(9));
        assert_eq!(locks.release(0,resource),Some(Some(2)));
        assert_eq!(locks.owner(resource),Some(2));
        assert_eq!(locks.effective(0),Some(if ceiling {5}else{2}));
        if ceiling {assert_eq!(locks.release(0,0),Some(None));}
        assert_eq!(locks.effective(0),Some(2));
    }
}
