use super::amc_policy::{AmcRuntime, Dispatch, SlotEnd, TaskSpec};
use super::domain_policy::{CriticalityMode, ExecutionBudget};
use ExecutionBudget::{High, Low};

fn spec(budget: ExecutionBudget, priority: u8, relative_deadline: u64) -> TaskSpec {
    TaskSpec {
        budget,
        priority,
        relative_deadline,
    }
}

fn start<const N: usize>(rt: &mut AmcRuntime<N>, task: usize, discarded: u32) {
    assert_eq!(rt.start_slot(), Some(Dispatch::Run { task, discarded }));
}

fn end<const N: usize>(
    rt: &mut AmcRuntime<N>,
    task: usize,
    time: u64,
    used: u64,
    remaining: u64,
    deadline: u64,
) -> SlotEnd {
    let actual = rt.finish_slot().unwrap();
    assert_eq!(
        actual,
        SlotEnd {
            task,
            end: time,
            used,
            remaining,
            deadline
        }
    );
    assert_eq!(rt.now(), time);
    actual
}

#[test]
fn maximum_live_carry_in_discards_every_low_job_and_retains_high_service() {
    let mut rt = AmcRuntime::new([
        spec(Low(2), 1, 3),
        spec(High { lo: 1, hi: 3 }, 4, 4),
        spec(Low(2), 2, 3),
        spec(High { lo: 1, hi: 1 }, 3, 5),
    ])
    .unwrap();
    assert!(rt.release(0, 1));
    start(&mut rt, 0, 0);
    end(&mut rt, 0, 1, 1, 0, 3);
    for (id, demand) in [(0, 2), (2, 2), (1, 4), (3, 1)] {
        assert!(rt.release(id, demand));
    }
    start(&mut rt, 1, 0);
    end(&mut rt, 1, 2, 1, 3, 5);
    assert!(rt.mode() == CriticalityMode::Low);
    assert_eq!(
        [0, 1, 2, 3].map(|id| rt.remaining(id)),
        [Some(2), Some(3), Some(2), Some(1)]
    );
    start(&mut rt, 1, 0b0101);
    assert!(rt.mode() == CriticalityMode::High);
    assert_eq!(
        [0, 1, 2, 3].map(|id| rt.remaining(id)),
        [None, Some(3), None, Some(1)]
    );
    assert!(!rt.release(2, 1));
    assert!(!rt.reset(true));
    end(&mut rt, 1, 3, 2, 2, 5);
    start(&mut rt, 1, 0);
    end(&mut rt, 1, 4, 3, 1, 5);
    assert_eq!(rt.start_slot(), Some(Dispatch::Exhausted { task: 1 }));
    assert_eq!(rt.now(), 4);
    assert_eq!(rt.remaining(1), None);
    assert!(!rt.reset(true));
    start(&mut rt, 3, 0);
    assert!(!end(&mut rt, 3, 5, 1, 0, 6).deadline_missed());
    assert_eq!(rt.start_slot(), Some(Dispatch::Idle));
    assert!(!rt.reset(false));
    assert!(!rt.release(0, 1));
    assert!(rt.reset(true));
    assert!(rt.mode() == CriticalityMode::Low);
    assert!(rt.release(0, 1));
    start(&mut rt, 0, 0);
    end(&mut rt, 0, 6, 1, 0, 8);
}

#[test]
fn exact_lo_completion_keeps_lower_criticality_jobs_and_deadline_is_inclusive() {
    let mut rt = AmcRuntime::new([spec(High { lo: 1, hi: 2 }, 8, 1), spec(Low(1), 8, 1)]).unwrap();
    assert!(rt.release(1, 1));
    assert!(rt.release(0, 1));
    start(&mut rt, 0, 0); // Equal priority resolves to task 0.
    assert!(!end(&mut rt, 0, 1, 1, 0, 1).deadline_missed());
    assert!(rt.mode() == CriticalityMode::Low);
    start(&mut rt, 1, 0);
    assert!(end(&mut rt, 1, 2, 1, 0, 1).deadline_missed());
}

#[test]
fn denied_service_cannot_trigger_a_high_mode_transition() {
    for budget in [Low(1), High { lo: 1, hi: 1 }] {
        let mut rt = AmcRuntime::new([spec(budget, 2, 1), spec(Low(2), 1, 3)]).unwrap();
        assert!(rt.release(0, 2));
        assert!(rt.release(1, 2));
        start(&mut rt, 0, 0);
        assert!(end(&mut rt, 0, 1, 1, 1, 1).deadline_missed());
        assert_eq!(rt.start_slot(), Some(Dispatch::Exhausted { task: 0 }));
        assert!(rt.mode() == CriticalityMode::Low);
        start(&mut rt, 1, 0);
        end(&mut rt, 1, 2, 1, 1, 3);
    }
}

#[test]
fn invalid_release_config_and_repeated_dispatch_have_no_effect() {
    let valid = spec(High { lo: 1, hi: 2 }, 4, 2);
    assert!(AmcRuntime::<33>::new([valid; 33]).is_none());
    for bad in [
        spec(Low(0), 1, 1),
        spec(High { lo: 3, hi: 2 }, 1, 1),
        spec(Low(1), 1, 0),
    ] {
        assert!(AmcRuntime::new([bad]).is_none());
    }
    let mut rt = AmcRuntime::new([valid, spec(Low(1), 5, u64::MAX)]).unwrap();
    assert!(!rt.reset(true));
    assert!(rt.finish_slot().is_none());
    assert!(!rt.release(0, 0));
    assert!(!rt.release(2, 1));
    assert_eq!(rt.start_slot(), Some(Dispatch::Idle));
    assert!(rt.release(0, 1));
    start(&mut rt, 0, 0);
    assert!(!rt.release(0, 2));
    assert!(rt.start_slot().is_none());
    end(&mut rt, 0, 1, 1, 0, 2);
    assert!(rt.finish_slot().is_none());
    assert!(!rt.release(1, 1)); // The absolute deadline would wrap.
    assert_eq!(rt.remaining(1), None);
    assert_eq!(rt.start_slot(), Some(Dispatch::Idle));
}

#[test]
fn full_task_mask_and_partial_high_carry_in_survive_the_transition() {
    let mut specs = [spec(Low(1), 0, 9); 32];
    specs[0] = spec(High { lo: 1, hi: 2 }, 4, 9);
    specs[1] = spec(High { lo: 2, hi: 3 }, 2, 9);
    let mut rt = AmcRuntime::new(specs).unwrap();
    assert!(rt.release(1, 3));
    start(&mut rt, 1, 0);
    end(&mut rt, 1, 1, 1, 2, 9);
    for id in 2..32 {
        assert!(rt.release(id, 1));
    }
    assert!(rt.release(0, 2));
    start(&mut rt, 0, 0);
    end(&mut rt, 0, 2, 1, 1, 10);
    start(&mut rt, 0, u32::MAX - 3);
    end(&mut rt, 0, 3, 2, 0, 10);
    assert_eq!(rt.remaining(1), Some(2));
    for id in 2..32 {
        assert_eq!(rt.remaining(id), None);
    }
    start(&mut rt, 1, 0);
    end(&mut rt, 1, 4, 2, 1, 9);
    start(&mut rt, 1, 0);
    end(&mut rt, 1, 5, 3, 0, 9);
    assert!(rt.reset(true));
}

#[test]
fn staggered_idle_baselines_and_carry_demands() {
    // Every admitted product: idle length 1..=4, both carry demands 1..=2.
    for idle in 1..=4 {
        for low in 1..=2 {
            for survivor in 1..=2 {
                let mut rt = AmcRuntime::new([
                    spec(Low(2), 1, 4),
                    spec(High { lo: 1, hi: 3 }, 4, 4),
                    spec(Low(2), 2, 3),
                    spec(High { lo: 1, hi: 2 }, 3, 5),
                ]).unwrap();
                assert!(rt.release(0, 1));
                assert!(!rt.advance_idle(1));
                start(&mut rt, 0, 0);
                assert!(!rt.advance_idle(1));
                end(&mut rt, 0, 1, 1, 0, 4);
                assert_eq!(rt.start_slot(), Some(Dispatch::Idle));
                assert_eq!(rt.now(), 1);
                assert!(!rt.advance_idle(0));
                assert!(rt.advance_idle(idle));
                let b = 1 + idle;
                assert!(rt.release(0, low));
                start(&mut rt, 0, 0);
                end(&mut rt, 0, b+1, 1, low-1, b+4);
                assert!(rt.release(3, survivor));
                start(&mut rt, 3, 0);
                end(&mut rt, 3, b+2, 1, survivor-1, b+6);
                assert!(rt.release(1, 2));
                start(&mut rt, 1, 0);
                end(&mut rt, 1, b+3, 1, 1, b+6);
                start(&mut rt, 1, u32::from(low == 2));
                end(&mut rt, 1, b+4, 2, 0, b+6);
                if survivor == 2 {
                    start(&mut rt, 3, 0);
                    end(&mut rt, 3, b+5, 2, 0, b+6);
                }
                assert_eq!(rt.start_slot(), Some(Dispatch::Idle));
                assert!(rt.advance_idle(u64::MAX - rt.now()));
                assert!(!rt.advance_idle(1));
                assert_eq!(rt.now(), u64::MAX);
                assert!(rt.mode() == CriticalityMode::High);
                assert!(!rt.release(3, 1));
            }
        }
    }
}
