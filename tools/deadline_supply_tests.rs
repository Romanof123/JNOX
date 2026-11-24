#![allow(dead_code)]
#[path = "../kernel/src/deadline_jobs.rs"]
mod deadline_jobs;
#[path = "../kernel/src/deadline_policy.rs"]
mod deadline_policy;
#[path = "../kernel/src/ready_queue.rs"]
mod ready_queue;
mod ordinary_deadline {
    pub fn counter() -> u64 {
        unreachable!()
    }
}
use deadline_jobs::*;
use deadline_policy::*;

#[test]
fn profiles_refuse_only_with_interference() {
    for policy in [DeadlinePolicy::Edf, DeadlinePolicy::Llf] {
        assert_eq!(
            admit_with_supply(policy, &SUPPLIED, 1000, 123, RESERVED),
            Ok(())
        );
        assert_eq!(admit(policy, &TIGHT, 1000, 123), Ok(()));
        let refusal = if policy == DeadlinePolicy::Edf {
            Refusal::Demand
        } else {
            Refusal::Miss
        };
        assert_eq!(
            admit_with_supply(policy, &TIGHT, 1000, 123, RESERVED),
            Err(refusal)
        );
    }
}
#[test]
fn endpoints_and_overflow() {
    assert_eq!(available(RESERVED, 0, 1), Ok(1));
    assert_eq!(available(RESERVED, 1, 2), Ok(0));
    assert_eq!(available(RESERVED, 2, 2), Ok(0));
    assert_eq!(available(1 << HORIZON, 0, 1), Err(Refusal::Grid));
    assert_eq!(available(0, 1, 0), Err(Refusal::Grid));
    assert_eq!(
        admit_with_supply(DeadlinePolicy::Edf, &TIGHT, 1000, u64::MAX, RESERVED),
        Err(Refusal::Overflow)
    );
    let mut overloaded = SUPPLIED;
    overloaded[0].cost = u64::MAX;
    overloaded[1].release = 0;
    assert_eq!(
        admit_with_supply(DeadlinePolicy::Edf, &overloaded, 1000, 0, RESERVED),
        Err(Refusal::Demand)
    );
    overloaded[1].deadline = overloaded[0].deadline;
    assert_eq!(
        admit_with_supply(DeadlinePolicy::Edf, &overloaded, 1000, 0, RESERVED),
        Err(Refusal::Overflow)
    );
}
#[test]
fn withheld_incumbent_keeps_its_turn_and_remaining() {
    for policy in [DeadlinePolicy::Edf, DeadlinePolicy::Llf] {
        let mut model = DeadlineScheduler::<3>::new(policy, 0, 1).unwrap();
        model.release(0, 2, 3, 8).unwrap();
        assert_eq!(model.decide(0), Ok(Some(2)));
        model.withhold(1).unwrap();
        model.release(1, 1, 2, 8).unwrap();
        model.release(1, 0, 2, 8).unwrap();
        assert_eq!(model.decide(2), Ok(Some(2)));
        assert_eq!(model.remaining(2), Some(2));
        model.complete(3, 2).unwrap();
        assert_eq!(model.decide(3), Ok(Some(1)));
    }
}

#[test]
fn owner_window_dimensions_and_checked_arithmetic() {
    for (q, cells, width) in [(0, 2, 1), (2, 0, 1), (2, 2, 0), (2, 2, 5)] {
        assert_eq!(owner_window_bound(q, cells, width, 0), Err(Refusal::Grid));
    }
    assert_eq!(owner_window_bound(u64::MAX, 2, 1, 0), Err(Refusal::Overflow));
    assert_eq!(owner_window_bound(1, 1, 1, u64::MAX), Err(Refusal::Overflow));
    assert_eq!(owner_window_bound(1, u64::MAX, u64::MAX, 0), Ok(u64::MAX));
    for jobs in 0..=N as u64 {
        for (q, cells, width, timers) in [
            (1, 4, 1, 1), (1, 4, 3, 3), (4, 3, 1, 1),
            (4, 3, 4, 2), (4, 3, 5, 2), (4, 3, 6, 3), (4, 3, 12, 3),
        ] {
            assert_eq!(owner_window_bound(q, cells, width, jobs), Ok(timers + jobs));
        }
    }
}

#[test]
fn owner_window_exhaustive_irq_offsets_and_early_completions() {
    // Enumerate every placement, including completions at identical counter
    // ticks and on either side of a timer. Such coincidences enlarge the
    // admissible set, making this a conservative event-count oracle.
    for q in 1..=3u64 {
        for cells in 1..=3u32 {
            let domain = q * u64::from(cells);
            for jobs in 0..=2u32 {
                let mut peaks = vec![0u64; domain as usize + 1];
                for offsets in 0..q.pow(cells) {
                    for completions in 0..domain.pow(jobs) {
                        let mut counts = vec![0u64; domain as usize];
                        for cell in 0..cells {
                            let tick = u64::from(cell) * q + offsets / q.pow(cell) % q;
                            counts[tick as usize] += 1;
                        }
                        for job in 0..jobs {
                            let tick = completions / domain.pow(job) % domain;
                            counts[tick as usize] += 1;
                        }
                        for start in 0..counts.len() {
                            let mut events = 0;
                            for (offset, count) in counts[start..].iter().enumerate() {
                                events += count;
                                peaks[offset + 1] = peaks[offset + 1].max(events);
                            }
                        }
                    }
                }
                for width in 1..=domain {
                    assert_eq!(
                        owner_window_bound(q, u64::from(cells), width, u64::from(jobs)),
                        Ok(peaks[width as usize]),
                        "q={q} cells={cells} width={width} jobs={jobs}",
                    );
                }
            }
        }
    }
}
