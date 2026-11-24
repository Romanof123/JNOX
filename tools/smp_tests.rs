use crate::smp_policy::{Disposition, Lease, Ownership};

#[test]
fn construction_checks_every_mask_and_task_limit() {
    assert!(Ownership::<32>::new([0xff; 32]).is_some());
    assert!(Ownership::<33>::new([1; 33]).is_none());
    assert!(Ownership::new([1, 0]).is_none());
}

#[test]
fn release_validates_the_entire_selection_before_mutating() {
    let mut state = Ownership::new([1, 1, 1]).unwrap();
    assert!(!state.release(0));
    assert!(!state.release(0b1001));
    assert!(state.release(1));
    assert!(!state.release(3));
    let lease = state.claim(0, 0).unwrap();
    assert!(!state.release(3));
    assert!(state.release(6));
    assert!(state.retire(lease, 0, Disposition::Dormant));
    assert!(state.release(1));
}

#[test]
fn invalid_rank_and_claim_inputs_preserve_ready_tasks() {
    let mut state = Ownership::new([2, 1]).unwrap();
    assert!(state.release(1));
    for rank in [&[0, 1][..], &[0, 0], &[2]] {
        assert!(state.plan(0xff, rank).is_none());
    }
    assert!(state.claim(8, 0).is_none());
    assert!(state.claim(0, 0).is_none());
    assert!(state.claim(0, 2).is_none());
    assert_eq!(state.charged(2), None);
    assert_eq!(state.plan(0, &[0]).unwrap(), [None; 8]);
    assert!(state.claim(1, 0).is_some());
    assert!(state.plan(0xff, &[0]).is_none());
}

#[test]
fn migrating_task_has_one_owner_and_rejects_stale_leases() {
    let mut state = Ownership::new([3, 3]).unwrap();
    assert!(state.release(3));
    let first: Lease = state.claim(0, 0).unwrap();
    let copy = first;
    assert_eq!(copy, first);
    assert_eq!((first.task, first.cpu), (0, 0));
    assert!(state.claim(1, 0).is_none());
    assert!(state.claim(0, 1).is_none());
    assert_eq!(state.plan(3, &[1]).unwrap()[..2], [None, Some(1)]);
    assert!(state.retire(first, 7, Disposition::Ready));
    let migrated = state.claim(1, 0).unwrap();
    assert_eq!((migrated.task, migrated.cpu), (0, 1));
    assert!(!state.retire(copy, 100, Disposition::Dormant));
    assert!(state.retire(migrated, 2, Disposition::Ready));
    let back = state.claim(0, 0).unwrap();
    assert_ne!(first.sequence, back.sequence);
    assert!(!state.retire(first, 4, Disposition::Dormant));
    assert!(state.retire(back, 1, Disposition::Dormant));
    assert!(!state.retire(back, 1, Disposition::Ready));
    assert_eq!(state.charged(0), Some(10));
    assert_eq!(state.charged(1), Some(0));
}

#[test]
fn charge_overflow_preserves_both_owner_and_balance() {
    let mut state = Ownership::new([1, 1]).unwrap();
    assert!(state.release(3));
    let first = state.claim(0, 0).unwrap();
    assert!(state.retire(first, u64::MAX - 1, Disposition::Ready));
    let second = state.claim(0, 0).unwrap();
    assert!(!state.retire(second, 2, Disposition::Dormant));
    assert_eq!(state.charged(0), Some(u64::MAX - 1));
    assert!(state.claim(0, 1).is_none());
    assert!(!state.release(1));
    assert!(state.retire(second, 1, Disposition::Dormant));
    assert_eq!(state.charged(0), Some(u64::MAX));
    assert!(state.claim(0, 1).is_some());
}

#[test]
fn clustered_singletons_leave_the_flexible_task_on_another_cpu() {
    let mut state = Ownership::new([0x81, 1, 1, 1]).unwrap();
    assert!(state.release(15));
    let expected = [Some(2), None, None, None, None, None, None, Some(0)];
    assert_eq!(state.plan(0x81, &[0, 2, 3, 1]), Some(expected));
}

#[test]
fn highest_task_and_cpu_bits_support_a_full_lease_cycle() {
    let mut state = Ownership::new([0x80; 32]).unwrap();
    assert!(state.release(1 << 31));
    assert_eq!(state.plan(0x80, &[31]).unwrap()[7], Some(31));
    let lease = state.claim(7, 31).unwrap();
    assert!(state.retire(lease, 3, Disposition::Dormant));
    assert_eq!(state.charged(31), Some(3));
}

#[test]
fn two_cpu_plans_match_exhaustive_ranked_subset_oracle() {
    let rank = [2u8, 0, 1];
    for a in 1..=3 {
        for b in 1..=3 {
            for c in 1..=3 {
                let masks = [a, b, c];
                let mut state = Ownership::new(masks).unwrap();
                assert!(state.release(7));
                for online in 0..=3 {
                    let plan = state.plan(online, &rank).unwrap();
                    let mut actual = 0u8;
                    for (cpu, slot) in plan.iter().enumerate() {
                        if let Some(task) = *slot {
                            assert_ne!(online & masks[task as usize] & (1 << cpu), 0);
                            let place = rank.iter().position(|&id| id == task).unwrap();
                            let bit = 4 >> place;
                            assert_eq!(actual & bit, 0);
                            actual |= bit;
                        }
                    }
                    // Enumerate an empty slot or each rank index for both CPUs.
                    let mut best = (0, 0);
                    for left in 0..=3 {
                        for right in 0..=3 {
                            if left == right && left < 3 {
                                continue;
                            }
                            let mut chosen = 0u8;
                            let mut valid = true;
                            for (cpu, index) in [left, right].iter().copied().enumerate() {
                                if index < 3 {
                                    valid &= online & masks[rank[index] as usize] & (1 << cpu) != 0;
                                    chosen |= 4 >> index;
                                }
                            }
                            if valid {
                                best = best.max((chosen.count_ones(), chosen));
                            }
                        }
                    }
                    assert_eq!((actual.count_ones(), actual), best);
                }
            }
        }
    }
}
