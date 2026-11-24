#[path = "../kernel/src/reservation_jobs.rs"]
mod reservation_jobs;
#[allow(dead_code)]
#[path = "../kernel/src/service_policy.rs"]
mod service_policy;
#[allow(dead_code)]
#[path = "../kernel/src/temporal_policy.rs"]
mod temporal_policy;

use reservation_jobs::{Action, Clock, Error, Jobs, Manifest, Policy, State, admission, charge};

#[test]
fn w_manifest_tracks_observed_traps_separately() {
    for policy in [Policy::Aligned, Policy::Cbs] {
        let mut jobs = Jobs::new(policy, Manifest::W).unwrap();
        let mut b_slots = Vec::new();
        let mut blocks = 0;
        for slot in 0..34 {
            let d = jobs.open(slot).unwrap();
            if let Some(id) = d.owner {
                let action = d.action.unwrap();
                assert!(matches!(d.states[id], State::Ready | State::Saved));
                assert_eq!(d.after[id].q + 1, d.before[id].q);
                assert!(jobs.open(slot + 1).is_err());
                assert!(jobs.trap(1 - id, action).is_err());
                jobs.trap(id, action).unwrap();
                assert!(jobs.trap(id, action).is_err());
                if id == 1 {
                    b_slots.push(slot);
                }
                if action == Action::Block {
                    assert_eq!(id, 0);
                    blocks += 1;
                }
            }
            if policy == Policy::Cbs && [4, 5, 13, 22, 23].contains(&slot) {
                let want = match slot {
                    4 => (1, 8),
                    5 => (0, 8),
                    13 => (2, 21),
                    22 => (2, 30),
                    23 => (1, 30),
                    _ => unreachable!(),
                };
                assert_eq!((d.before[0].q, d.before[0].d), want);
            }
        }
        assert_eq!(blocks, 5);
        let fourth = if policy == Policy::Cbs { 13 } else { 14 };
        assert_eq!(b_slots, [1, 5, 9, fourth, 17, 21, 25, 29, 33]);
        assert!(jobs.open(34).is_err());
    }
}

#[test]
fn saturation_exits_only_on_matching_trap() {
    for policy in [Policy::Aligned, Policy::Cbs] {
        let mut jobs = Jobs::new(policy, Manifest::S).unwrap();
        for t in 0..10 {
            let d = jobs.open(t).unwrap();
            if t < 8 {
                let id = (t % 2) as usize;
                assert_eq!(d.owner, Some(id));
                let action = if t >= 6 { Action::Exit } else { Action::Long };
                assert_eq!(d.action, Some(action));
                assert!(jobs.trap(id, Action::Block).is_err());
                jobs.trap(id, action).unwrap();
                assert!(jobs.trap(id, Action::Exit).is_err());
            } else {
                assert_eq!(d.owner, None);
                assert_eq!(d.states, [State::Exited; 2]);
            }
        }
        assert!(matches!(
            Jobs::new(policy, Manifest::R),
            Err(Error::OverCapacity)
        ));
    }
}

#[test]
fn refuse_invalid_overcapacity_and_unrepresentable_exact_fractions() {
    assert_eq!(admission(&[(1, 2), (1, 2)]), Ok(()));
    assert_eq!(
        admission(&[(1, 2), (1, 2), (1, 8)]),
        Err(Error::OverCapacity)
    );
    for bad in [(0, 1), (1, 0), (2, 1)] {
        assert_eq!(admission(&[bad]), Err(Error::Invalid));
    }
    assert_eq!(
        admission(&[(1, 4294967291), (1, 4294967279), (1, 4294967231)]),
        Err(Error::Overflow)
    );
}

#[test]
fn preflight_and_live_arithmetic_at_limits() {
    let max = u64::MAX;
    let w = Clock::new(max - 2000, 1000, Manifest::W).unwrap();
    assert_eq!(w.boundary(40), Ok(max));
    assert!(matches!(
        Clock::new(max - 1999, 1000, Manifest::W),
        Err(Error::Overflow)
    ));
    let s = Clock::new(max - 502, 1000, Manifest::S).unwrap();
    assert_eq!(s.boundary(10), Ok(max - 2));
    assert_eq!(s.cutoff(9), Ok(max - 3));
    assert!(matches!(
        Clock::new(max - 501, 1000, Manifest::S),
        Err(Error::Overflow)
    ));
    assert!(matches!(
        Clock::new(0, 1001, Manifest::S),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        Clock::new(0, max - max % 1000, Manifest::W),
        Err(Error::Overflow)
    ));
    assert_eq!(charge(max - 2, 1, 3), Ok(max));
    assert_eq!(charge(max - 1, 1, 3), Err(Error::Overflow));
    assert_eq!(charge(0, 2, 1), Err(Error::Unexpected));
}

#[test]
fn first_proposal_is_conditional_and_matches_synchronous_open() {
    for policy in [Policy::Aligned, Policy::Cbs] {
        for manifest in [Manifest::W, Manifest::S] {
            let mut live = Jobs::new(policy, manifest).unwrap();
            let mut reference = Jobs::new(policy, manifest).unwrap();
            let expected = reference.open(0).unwrap();
            let candidate = live.prepare_first().unwrap();
            let stale = live.prepare_first().unwrap();
            assert!(live.trap(0, Action::Long).is_err());
            let got = live.commit_first(candidate).unwrap();
            assert_eq!(
                (
                    got.owner,
                    got.action,
                    got.before,
                    got.after,
                    got.demand,
                    got.states,
                    got.releases
                ),
                (
                    expected.owner,
                    expected.action,
                    expected.before,
                    expected.after,
                    expected.demand,
                    expected.states,
                    expected.releases
                )
            );
            assert!(live.commit_first(stale).is_err());
            assert!(live.open(0).is_err());
            assert!(live.prepare_first().is_err());
            live.trap(got.owner.unwrap(), got.action.unwrap()).unwrap();
            assert!(live.prepare_first().is_err());
            reference
                .trap(expected.owner.unwrap(), expected.action.unwrap())
                .unwrap();
            assert_eq!(
                live.open(1).unwrap().before,
                reference.open(1).unwrap().before
            );
        }
    }
}

#[test]
fn stale_after_block_or_exit_and_incompatible_origin() {
    for manifest in [Manifest::W, Manifest::S] {
        let mut live = Jobs::new(Policy::Cbs, manifest).unwrap();
        let stale = live.prepare_first().unwrap();
        let mut other = Jobs::new(Policy::Aligned, manifest).unwrap();
        assert!(other.commit_first(live.prepare_first().unwrap()).is_err());
        let count = if manifest == Manifest::W { 1 } else { 8 };
        for slot in 0..count {
            let d = live.open(slot).unwrap();
            live.trap(d.owner.unwrap(), d.action.unwrap()).unwrap();
        }
        assert!(live.commit_first(stale).is_err());
    }
}

#[test]
fn opening_and_counter_accounting_boundaries() {
    let clock = Clock::new(10_000, 62_500_000, Manifest::S).unwrap();
    assert!(!clock.within_opening(9_999));
    assert!(clock.within_opening(10_000));
    assert!(clock.within_opening(72_500));
    assert!(!clock.within_opening(72_501));
    let end = clock.boundary(1).unwrap();
    let returned = clock.epoch + 100;
    let trap = end + 125_000;
    assert_eq!(charge(0, returned, trap), Ok(trap - returned));
    assert_eq!(charge(0, end, trap), Ok(125_000));
    assert!(charge(0, trap, returned).is_err());
}

#[test]
fn opening_requires_matching_idle_return_and_owned_frames() {
    use reservation_jobs::first_return_matches;
    let f = [16, 32];
    let witness = [96, 128, 7];
    assert!(first_return_matches(Some(witness), 96, 128, 7, f, f));
    assert!(!first_return_matches(None, 96, 128, 7, f, f));
    assert!(!first_return_matches(
        Some(witness),
        96,
        128,
        7,
        f,
        [32, 16]
    ));
    assert!(!first_return_matches(Some(witness), 96, 132, 7, f, f));
    assert!(!first_return_matches(Some(witness), 96, 128, 6, f, f));
}

#[test]
fn compact_destination_keeps_pristine_origin_and_single_pending_debit() {
    for policy in [Policy::Aligned, Policy::Cbs] {
        for manifest in [Manifest::W,Manifest::S] {
            let origin=Jobs::new(policy,manifest).unwrap();
            let mut destination=origin.clone();
            let expected=destination.prepare_destination().unwrap();
            assert!(origin.pristine());
            assert!(origin.same_configuration(&destination));
            assert!(destination.matches_first_decision(&expected));
            assert!(destination.prepare_destination().is_err());
            assert!(origin.pending_grant().is_none());
            let (id,action)=destination.pending_grant().unwrap();
            assert_eq!(id,0);
            assert!(destination.trap(1,action).is_err());
            destination.trap(id,action).unwrap();
            assert!(destination.trap(id,action).is_err());
            assert!(origin.pristine());
        }
    }
}
