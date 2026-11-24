//! Fixed-depth, single-CPU service allocation. Array order declares priority
//! among siblings (lower index wins), as in the two-level GroupScheduler.
//! The fixed parent -> child group -> leaf shape makes cycles unrepresentable.
//! Readiness belongs to the caller, which must enforce each grant's end.

#[derive(Clone, Copy)]
struct Budget {
    limit: u64,
    left: u64,
    window: u64,
}

impl Budget {
    fn new(limit: u64) -> Self {
        Self {
            limit,
            left: limit,
            window: 0,
        }
    }

    fn remaining(self, window: u64) -> u64 {
        if self.window == window {
            self.left
        } else {
            self.limit
        }
    }

    fn charge(&mut self, window: u64, ticks: u64) {
        self.left = self.remaining(window) - ticks;
        self.window = window;
    }
}

pub(crate) struct Hierarchy<const P: usize, const C: usize, const L: usize> {
    start: u64,
    period: u64,
    last_grant_start: u64,
    committed_end: u64,
    parents: [Budget; P],
    children: [(usize, Budget); C],
    leaves: [(usize, Budget); L],
}

impl<const P: usize, const C: usize, const L: usize> Hierarchy<P, C, L> {
    /// Budgets are per common period. Siblings may collectively request more
    /// than their parent's limit; all draws from that parent share its cap.
    pub(crate) fn new(
        start: u64,
        period: u64,
        parents: [u64; P],
        children: [(usize, u64); C],
        leaves: [(usize, u64); L],
    ) -> Option<Self> {
        let valid = |limit| limit > 0 && limit <= period;
        if P == 0
            || C == 0
            || L == 0
            || parents.iter().any(|&limit| !valid(limit))
            || children
                .iter()
                .any(|&(parent, limit)| parent >= P || !valid(limit))
            || leaves
                .iter()
                .any(|&(child, limit)| child >= C || !valid(limit))
        {
            return None;
        }
        Some(Self {
            start,
            period,
            last_grant_start: start,
            committed_end: start,
            parents: parents.map(Budget::new),
            children: children.map(|(parent, limit)| (parent, Budget::new(limit))),
            leaves: leaves.map(|(child, limit)| (child, Budget::new(limit))),
        })
    }

    /// Pure query for one path at or after the last grant start. A grant
    /// is reserved in full, so its new balances are immediately visible. Refill is
    /// computed from the original grid, including windows skipped while idle.
    pub(crate) fn remaining_at(
        &self,
        now: u64,
        parent: usize,
        child: usize,
        leaf: usize,
    ) -> Option<(u64, u64, u64)> {
        if now < self.last_grant_start {
            return None;
        }
        let window = (now - self.start) / self.period;
        let &(owner, leaf_budget) = self.leaves.get(leaf)?;
        let &(ancestor, child_budget) = self.children.get(child)?;
        if owner != child || ancestor != parent {
            return None;
        }
        Some((
            self.parents.get(parent)?.remaining(window),
            child_budget.remaining(window),
            leaf_budget.remaining(window),
        ))
    }

    /// Reserve one contiguous interval. A denied request changes no balance
    /// or timestamp. A successful request touches only the selected path.
    pub(crate) fn grant_at(
        &mut self,
        now: u64,
        requested: u64,
        ready: &[bool; L],
    ) -> Option<(usize, usize, usize, u64)> {
        if now < self.committed_end {
            return None;
        }
        let elapsed = now - self.start;
        let window = elapsed / self.period;
        let available = requested
            .min(self.period - elapsed % self.period)
            .min(u64::MAX - now);
        if available == 0 {
            return None;
        }

        // A parent/child grant alone cannot be committed until a ready leaf
        // beneath it has capacity. Rank complete eligible paths atomically.
        let (parent, child, leaf) = self
            .leaves
            .iter()
            .enumerate()
            .filter_map(|(leaf, &(child, budget))| {
                let (parent, child_budget) = self.children[child];
                (ready[leaf]
                    && budget.remaining(window) > 0
                    && child_budget.remaining(window) > 0
                    && self.parents[parent].remaining(window) > 0)
                    .then_some((parent, child, leaf))
            })
            .min()?;
        let ticks = available
            .min(self.parents[parent].remaining(window))
            .min(self.children[child].1.remaining(window))
            .min(self.leaves[leaf].1.remaining(window));
        self.parents[parent].charge(window, ticks);
        self.children[child].1.charge(window, ticks);
        self.leaves[leaf].1.charge(window, ticks);
        self.last_grant_start = now;
        self.committed_end = now + ticks;
        Some((parent, child, leaf, ticks))
    }
}
