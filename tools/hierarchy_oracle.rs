//! Compare nested grants with an independent ledger of occupied CPU ticks.

use super::hierarchy_policy::Hierarchy;

type Path = (usize, usize, usize);
type Grant = (usize, usize, usize, u64);

struct Calendar<const P: usize, const C: usize, const L: usize> {
    origin: u64,
    period: u64,
    next: u64,
    parents: [u64; P],
    children: [(usize, u64); C],
    leaves: [(usize, u64); L],
    occupied: Vec<(u64, Path)>,
}

impl<const P: usize, const C: usize, const L: usize> Calendar<P, C, L> {
    fn new(
        origin: u64,
        period: u64,
        parents: [u64; P],
        children: [(usize, u64); C],
        leaves: [(usize, u64); L],
    ) -> Self {
        Self {
            origin,
            period,
            next: origin,
            parents,
            children,
            leaves,
            occupied: Vec::new(),
        }
    }

    fn model(&self) -> Hierarchy<P, C, L> {
        Hierarchy::new(
            self.origin,
            self.period,
            self.parents,
            self.children,
            self.leaves,
        )
        .unwrap()
    }

    fn window(&self, t: u64) -> u64 {
        (t - self.origin) / self.period
    }

    fn balance(&self, t: u64, path: Path) -> (u64, u64, u64) {
        let (p, c, l) = path;
        let mut left = (self.parents[p], self.children[c].1, self.leaves[l].1);
        for &(tick, (owner_p, owner_c, owner_l)) in &self.occupied {
            if self.window(tick) == self.window(t) {
                left.0 -= u64::from(owner_p == p);
                left.1 -= u64::from(owner_c == c);
                left.2 -= u64::from(owner_l == l);
            }
        }
        left
    }

    fn owner(&self, t: u64, ready: &[bool; L]) -> Option<Path> {
        for parent in 0..P {
            for (child, &(home, _)) in self.children.iter().enumerate() {
                if home != parent {
                    continue;
                }
                for (leaf, &(branch, _)) in self.leaves.iter().enumerate() {
                    if branch == child && ready[leaf] {
                        let path = (parent, child, leaf);
                        let (p, c, l) = self.balance(t, path);
                        if p > 0 && c > 0 && l > 0 {
                            return Some(path);
                        }
                    }
                }
            }
        }
        None
    }

    fn reserve(&mut self, t: u64, request: u64, ready: &[bool; L]) -> Option<Grant> {
        if t < self.next {
            return None;
        }
        let path = self.owner(t, ready)?;
        let mut tick = t;
        let mut count = 0;
        while count < request && tick < u64::MAX && self.window(tick) == self.window(t) {
            let (parent, child, leaf) = self.balance(tick, path);
            if parent == 0 || child == 0 || leaf == 0 {
                break;
            }
            self.occupied.push((tick, path));
            tick += 1;
            count += 1;
        }
        if count == 0 {
            return None;
        }
        self.next = tick;
        Some((path.0, path.1, path.2, count))
    }

    fn check_balances(&self, model: &Hierarchy<P, C, L>, t: u64) {
        for (leaf, &(child, _)) in self.leaves.iter().enumerate() {
            let parent = self.children[child].0;
            assert_eq!(
                model.remaining_at(t, parent, child, leaf),
                Some(self.balance(t, (parent, child, leaf))),
                "t={t}, leaf={leaf}"
            );
        }
    }

    fn step(
        &mut self,
        model: &mut Hierarchy<P, C, L>,
        t: u64,
        request: u64,
        ready: &[bool; L],
    ) -> Option<Grant> {
        let expected = self.reserve(t, request, ready);
        assert_eq!(
            model.grant_at(t, request, ready),
            expected,
            "t={t}, q={request}, ready={ready:?}"
        );
        if expected.is_some() {
            self.check_balances(model, t);
        }
        self.check_balances(model, self.next);
        expected
    }
}

#[test]
fn guest_workload_consumes_exactly_one_tick_at_each_ancestor_for_two_windows() {
    let mut cal = Calendar::new(
        0,
        6,
        [3, 6],
        [(0, 6), (0, 1), (0, 4), (1, 6)],
        [(0, 6), (1, 6), (2, 6), (3, 6)],
    );
    let mut model = cal.model();
    let ready = [false, true, true, true];
    for base in [0, 6] {
        assert_eq!(cal.step(&mut model, base, 6, &ready), Some((0, 1, 1, 1)));
        assert_eq!(
            cal.step(&mut model, base + 1, 6, &ready),
            Some((0, 2, 2, 2))
        );
        assert_eq!(cal.step(&mut model, base + 2, 6, &[true; 4]), None);
        assert_eq!(
            cal.step(&mut model, base + 3, 6, &ready),
            Some((1, 3, 3, 3))
        );
    }
    let trace: Vec<_> = cal.occupied.iter().map(|&(_, (_, _, leaf))| leaf).collect();
    assert_eq!(trace, [1, 2, 2, 3, 3, 3, 1, 2, 2, 3, 3, 3]);
}

#[test]
fn exhaustive_pairs_of_ready_sets_and_requests_update_only_the_awarded_path() {
    // Parent/child priority disagrees with leaf ID. Child 3 has no leaves
    // and parent 2 has no children. Alternate caps make each level decisive.
    for (parent_quota, child_quota, leaf_quota) in [(1, 3, 3), (3, 1, 3), (3, 3, 1)] {
        for first in 0u8..16 {
            for second in 0u8..16 {
                for r1 in [0, 1, u64::MAX] {
                    for r2 in [0, 1, u64::MAX] {
                        let mut cal = Calendar::new(
                            7,
                            3,
                            [parent_quota, 3, 2],
                            [(1, 3), (0, child_quota), (0, 2), (1, 2)],
                            [(0, 2), (2, 3), (1, leaf_quota), (1, 2)],
                        );
                        let mut model = cal.model();
                        let mask = |bits: u8| core::array::from_fn(|id| bits & (1 << id) != 0);
                        let _ = cal.step(&mut model, 7, r1, &mask(first));
                        let t = cal.next;
                        let _ = cal.step(&mut model, t, r2, &mask(second));
                        let t = cal.next;
                        let _ = cal.step(&mut model, t, u64::MAX, &[true; 4]);
                    }
                }
            }
        }
    }
}

#[test]
fn unit_chunks_and_one_contiguous_grant_agree_when_the_same_owner_remains_ready() {
    let ready = [true, true, true];
    for offset in 0..5 {
        for request in 1..=8 {
            let mut cal = Calendar::new(23, 5, [5, 3], [(1, 2), (0, 4)], [(0, 2), (1, 3), (1, 4)]);
            let mut batch = cal.model();
            let mut units = cal.model();
            let start = 23 + offset;
            let (p, c, l, count) = cal.step(&mut batch, start, request, &ready).unwrap();
            for tick in start..start + count {
                assert_eq!(units.grant_at(tick, 1, &ready), Some((p, c, l, 1)));
                assert_eq!(units.grant_at(tick, 1, &ready), None);
            }
            cal.check_balances(&units, start + count);
            let t = start + count;
            assert_eq!(
                batch.grant_at(t, request, &ready),
                units.grant_at(t, request, &ready)
            );
        }
    }
}

#[test]
fn skipped_windows_and_unrepresentable_interval_end_never_create_extra_ticks() {
    let mut cal = Calendar::new(9, 5, [4], [(0, 5)], [(0, 3), (0, 2)]);
    let mut model = cal.model();
    for (now, request, ready) in [
        (8, 1, [true, true]),
        (9, 3, [true, false]),
        (u64::MAX, 1, [true, true]),
        (99, 0, [true, true]),
        (99, 3, [false, false]),
        (12, 3, [false, true]),
        (13, 8, [true, true]),
        (u64::MAX - 7, u64::MAX, [false, true]),
        (u64::MAX - 3, u64::MAX, [true, true]),
        (u64::MAX - 1, u64::MAX, [true, true]),
        (u64::MAX, u64::MAX, [true, true]),
    ] {
        let _ = cal.step(&mut model, now, request, &ready);
    }
    assert!(cal.occupied.iter().all(|&(tick, _)| tick < u64::MAX));
}
