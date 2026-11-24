//! Mutex ownership and priority accepted at the public lock-set boundary.
use crate::mutex_policy::{
    Acquire, LockSet, LockSpec,
    Protocol::{Immediate, Inherit},
};

// Role numbers describe a chain independently of the actual task IDs.
// Starting at every potential donor, carry its local priority along each
// reachable inheritance edge. A ceiling contributes only at its current owner.
#[derive(Debug)]
struct Chain {
    ids: [usize; 4],
    base: [u8; 4],
    ceilings: [Option<u8>; 3],
    owners: [Option<usize>; 3],
    waits: [Option<usize>; 4],
}

impl Chain {
    fn check(&self, locks: &LockSet<4, 3>) {
        let mut local = self.base;
        for lock in 0..3 {
            if let (Some(owner), Some(ceiling)) = (self.owners[lock], self.ceilings[lock]) {
                local[owner] = local[owner].max(ceiling);
            }
            assert_eq!(
                locks.owner(lock),
                self.owners[lock].map(|r| self.ids[r]),
                "{self:?}"
            );
        }
        let mut priority = local;
        for (source, donation) in local.into_iter().enumerate() {
            let mut task = source;
            while let Some(lock) = self.waits[task] {
                if self.ceilings[lock].is_some() {
                    break;
                }
                task = self.owners[lock].unwrap();
                priority[task] = priority[task].max(donation);
            }
        }
        for role in 0..4 {
            assert_eq!(locks.waiting(self.ids[role]), self.waits[role], "{self:?}");
            assert_eq!(
                locks.effective(self.ids[role]),
                Some(priority[role]),
                "{self:?}"
            );
        }
    }

    fn block(&mut self, locks: &mut LockSet<4, 3>, lock: usize) {
        let role = lock + 1;
        assert!(matches!(
            locks.acquire(self.ids[role], lock),
            Some(Acquire::Blocked)
        ));
        self.waits[role] = Some(lock);
        self.check(locks);
    }
}

fn chain_trial(ids: [usize; 4], base: [u8; 4], protocols: u8, join_order: [usize; 3]) {
    let mut tasks = [0; 4];
    for i in 0..4 {
        tasks[ids[i]] = base[i];
    }
    let ceilings = std::array::from_fn(|l| {
        (protocols & (1 << l) != 0).then(|| base[l].max(base[l + 1]) + 1 + l as u8)
    });
    let specs = std::array::from_fn(|l| LockSpec {
        users: (1 << ids[l]) | (1 << ids[l + 1]),
        protocol: ceilings[l].map_or(Inherit, Immediate),
    });
    let mut locks = LockSet::<4, 3>::new(tasks, specs).unwrap();
    let mut model = Chain {
        ids,
        base,
        ceilings,
        owners: [None; 3],
        waits: [None; 4],
    };
    for lock in 0..3 {
        assert!(matches!(
            locks.acquire(ids[lock], lock),
            Some(Acquire::Held)
        ));
        model.owners[lock] = Some(lock);
        model.check(&locks);
    }
    for lock in join_order {
        model.block(&mut locks, lock);
    }
    // Remove and restore each edge while its other donors and locks remain.
    for role in [2, 3, 1] {
        assert!(locks.timeout(ids[role]));
        model.waits[role] = None;
        model.check(&locks);
        model.block(&mut locks, role - 1);
    }
    for lock in 0..3 {
        let winner = lock + 1;
        assert_eq!(locks.release(ids[lock], lock), Some(Some(ids[winner])));
        model.owners[lock] = Some(winner);
        model.waits[winner] = None;
        model.check(&locks);
        assert_eq!(locks.release(ids[winner], lock), Some(None));
        model.owners[lock] = None;
        model.check(&locks);
    }
}

#[test]
fn transitive_priorities_match_reachable_donors_across_mixed_protocol_chains() {
    let join_orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                if first == second || first == third || second == third {
                    continue;
                }
                let ids = [first, second, third, 6 - first - second - third];
                for base in [[1, 3, 2, 11], [9, 1, 7, 2], [4; 4]] {
                    for protocols in 0..8 {
                        for joins in join_orders {
                            chain_trial(ids, base, protocols, joins);
                        }
                    }
                }
            }
        }
    }
}

fn inheritance<const L: usize>(users: u32) -> [LockSpec; L] {
    std::array::from_fn(|_| LockSpec {
        users,
        protocol: Inherit,
    })
}

#[test]
fn transfer_uses_current_effective_priority_and_original_arrival_among_ties() {
    // Task 4 is weaker than task 2 until a donor blocks behind its other lock.
    // In the tie case, gaining the donation must not assign a new queue position.
    for (stronger_base, expected) in [(8, [4, 2]), (11, [2, 4])] {
        let mut locks =
            LockSet::<5, 2>::new([1, 11, stronger_base, 3, 2], inheritance(0b11111)).unwrap();
        assert!(matches!(locks.acquire(0, 0), Some(Acquire::Held)));
        assert!(matches!(locks.acquire(4, 1), Some(Acquire::Held)));
        for id in [2, 4] {
            assert!(matches!(locks.acquire(id, 0), Some(Acquire::Blocked)));
        }
        assert!(matches!(locks.acquire(1, 1), Some(Acquire::Blocked)));
        assert_eq!(locks.effective(4), Some(11));
        assert_eq!(locks.effective(0), Some(11));
        assert_eq!(locks.release(0, 0), Some(Some(expected[0])));
        assert_eq!(locks.owner(0), Some(expected[0]));
        assert_eq!(locks.waiting(expected[0]), None);
        assert_eq!(locks.waiting(expected[1]), Some(0));
        assert_eq!(locks.release(expected[0], 0), Some(Some(expected[1])));
        assert_eq!(locks.release(expected[1], 0), Some(None));
        assert_eq!(locks.effective(0), Some(1));
        assert_eq!(locks.release(4, 1), Some(Some(1)));
        assert_eq!(locks.effective(4), Some(2));
    }
}

#[test]
fn timeout_and_rejoin_puts_an_equal_waiter_at_the_back() {
    for protocol in [Inherit, Immediate(7)] {
        let mut locks = LockSet::<4, 1>::new(
            [7; 4],
            [LockSpec {
                users: 15,
                protocol,
            }],
        )
        .unwrap();
        assert!(matches!(locks.acquire(1, 0), Some(Acquire::Held)));
        for id in [3, 0, 2] {
            assert!(matches!(locks.acquire(id, 0), Some(Acquire::Blocked)));
        }
        assert!(locks.timeout(3));
        assert!(!locks.timeout(3));
        assert!(matches!(locks.acquire(3, 0), Some(Acquire::Blocked)));
        for (owner, next) in [(1, 0), (0, 2), (2, 3)] {
            assert_eq!(locks.release(owner, 0), Some(Some(next)));
            assert_eq!(locks.owner(0), Some(next));
            assert_eq!(locks.waiting(next), None);
        }
        assert_eq!(locks.release(3, 0), Some(None));
    }
}

#[test]
fn a_waiter_that_rises_to_a_tie_keeps_its_earlier_arrival() {
    let mut locks = LockSet::<4, 2>::new([1, 9, 9, 2], inheritance(15)).unwrap();
    assert!(matches!(locks.acquire(0, 0), Some(Acquire::Held)));
    assert!(matches!(locks.acquire(3, 1), Some(Acquire::Held)));
    assert!(matches!(locks.acquire(3, 0), Some(Acquire::Blocked)));
    assert!(matches!(locks.acquire(1, 0), Some(Acquire::Blocked)));
    assert!(matches!(locks.acquire(2, 1), Some(Acquire::Blocked)));
    assert_eq!(locks.effective(3), Some(9));
    assert_eq!(locks.release(0, 0), Some(Some(3)));
    assert_eq!(locks.release(3, 0), Some(Some(1)));
}

#[test]
fn cycles_are_rejected_before_any_priority_or_wait_state_changes() {
    // The dependency cycle is illegal even if one edge stops inheritance.
    for middle in [Inherit, Immediate(11)] {
        let mut locks = LockSet::<3, 3>::new(
            [2, 5, 11],
            [
                LockSpec {
                    users: 7,
                    protocol: Inherit,
                },
                LockSpec {
                    users: 7,
                    protocol: middle,
                },
                LockSpec {
                    users: 7,
                    protocol: Inherit,
                },
            ],
        )
        .unwrap();
        for id in 0..3 {
            assert!(matches!(locks.acquire(id, id), Some(Acquire::Held)));
        }
        assert!(matches!(locks.acquire(0, 1), Some(Acquire::Blocked)));
        assert!(matches!(locks.acquire(1, 2), Some(Acquire::Blocked)));
        let priorities = [0, 1, 2].map(|id| locks.effective(id));
        assert!(locks.acquire(2, 0).is_none());
        assert_eq!([0, 1, 2].map(|id| locks.effective(id)), priorities);
        assert_eq!(
            [0, 1, 2].map(|id| locks.owner(id)),
            [Some(0), Some(1), Some(2)]
        );
        assert_eq!(
            [0, 1, 2].map(|id| locks.waiting(id)),
            [Some(1), Some(2), None]
        );
        assert!(!locks.timeout(2));
        assert!(locks.timeout(0));
        assert!(matches!(locks.acquire(2, 0), Some(Acquire::Blocked)));
        assert_eq!(locks.release(0, 0), Some(Some(2)));
    }
}

#[test]
fn nested_ceilings_depth_lifo_and_access_control_restore_each_prior_boost() {
    let mut locks = LockSet::<3, 3>::new(
        [1, 4, 250],
        [
            LockSpec {
                users: 0b011,
                protocol: Immediate(8),
            },
            LockSpec {
                users: 0b011,
                protocol: Immediate(10),
            },
            LockSpec {
                users: 0b011,
                protocol: Inherit,
            },
        ],
    )
    .unwrap();
    assert!(matches!(locks.acquire(0, 0), Some(Acquire::Held)));
    assert_eq!(locks.effective(0), Some(8));
    assert!(locks.acquire(0, 0).is_none());
    assert!(matches!(locks.acquire(0, 1), Some(Acquire::Held)));
    assert_eq!(locks.effective(0), Some(10));
    assert!(locks.acquire(0, 2).is_none());
    assert!(locks.acquire(2, 2).is_none());
    assert!(locks.release(1, 1).is_none());
    assert!(locks.release(0, 0).is_none());
    assert_eq!(locks.owner(2), None);
    assert_eq!(locks.effective(0), Some(10));
    assert!(matches!(locks.acquire(1, 0), Some(Acquire::Blocked)));
    assert_eq!(locks.effective(1), Some(4));
    assert_eq!(locks.release(0, 1), Some(None));
    assert_eq!(locks.effective(0), Some(8));
    assert_eq!(locks.release(0, 0), Some(Some(1)));
    assert_eq!(locks.effective(0), Some(1));
    assert_eq!(locks.effective(1), Some(8));
    assert_eq!(locks.release(1, 0), Some(None));
    assert_eq!(locks.effective(1), Some(4));
}

#[test]
fn configuration_and_id_checks_cover_the_full_user_mask() {
    assert!(LockSet::<0, 1>::new([], inheritance(1)).is_none());
    assert!(LockSet::<33, 1>::new([0; 33], inheritance(1)).is_none());
    assert!(LockSet::<1, 0>::new([0], []).is_none());
    assert!(LockSet::<2, 1>::new([0; 2], inheritance(0)).is_none());
    assert!(LockSet::<2, 1>::new([0; 2], inheritance(0b101)).is_none());
    assert!(
        LockSet::<2, 1>::new(
            [3, 11],
            [LockSpec {
                users: 3,
                protocol: Immediate(10)
            }]
        )
        .is_none()
    );
    assert!(
        LockSet::<2, 1>::new(
            [3, 11],
            [LockSpec {
                users: 1,
                protocol: Immediate(3)
            }]
        )
        .is_some()
    );
    let mut base = [0; 32];
    base[31] = u8::MAX;
    let mut locks = LockSet::<32, 2>::new(
        base,
        [
            LockSpec {
                users: 1u32 << 31,
                protocol: Immediate(u8::MAX),
            },
            LockSpec {
                users: u32::MAX,
                protocol: Inherit,
            },
        ],
    )
    .unwrap();
    assert_eq!(locks.effective(usize::MAX), None);
    assert_eq!(locks.owner(usize::MAX), None);
    assert_eq!(locks.waiting(usize::MAX), None);
    assert!(!locks.timeout(32));
    assert!(locks.acquire(30, 0).is_none());
    assert!(locks.acquire(32, 1).is_none());
    assert!(locks.acquire(31, 2).is_none());
    assert!(locks.release(0, usize::MAX).is_none());
    assert!(locks.release(usize::MAX, 1).is_none());
    assert!(matches!(locks.acquire(31, 0), Some(Acquire::Held)));
    assert!(matches!(locks.acquire(0, 1), Some(Acquire::Held)));
    assert!(matches!(locks.acquire(31, 1), Some(Acquire::Blocked)));
    assert_eq!(locks.effective(0), Some(u8::MAX));
    assert_eq!(locks.release(0, 1), Some(Some(31)));
    assert_eq!(locks.effective(0), Some(0));
    assert_eq!(locks.release(31, 1), Some(None));
    assert_eq!(locks.release(31, 0), Some(None));
}

#[test]
fn ordinary_pi_transfers_to_equal_waiters_in_both_admission_orders() {
    for [first, second] in [[1, 2], [2, 1]] {
        let mut locks = LockSet::<4, 1>::new([2, 7, 7, 0], inheritance(0b0111)).unwrap();
        assert!(matches!(locks.acquire(0, 0), Some(Acquire::Held)));
        for id in [first, second] {
            assert!(matches!(locks.acquire(id, 0), Some(Acquire::Blocked)));
            assert_eq!(locks.effective(0), Some(7));
        }
        assert_eq!(locks.release(0, 0), Some(Some(first)));
        assert_eq!(locks.effective(0), Some(2));
        assert_eq!(locks.waiting(second), Some(0));
        assert_eq!(locks.release(first, 0), Some(Some(second)));
        assert_eq!(locks.release(second, 0), Some(None));
    }
}
