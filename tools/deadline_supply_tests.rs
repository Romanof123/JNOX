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
