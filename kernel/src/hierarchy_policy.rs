//! Fixed-depth, single-CPU service allocation. Array order declares priority
//! among siblings (lower index wins), as in the two-level GroupScheduler.
//! The fixed parent -> child group -> leaf shape makes cycles unrepresentable.
//! Readiness belongs to the caller, which must enforce each grant's end.

#[derive(Clone, Copy)]
struct Budget {
    limit: u64,
    left: i128,
    window: u64,
}

impl Budget {
    fn new(limit: u64) -> Self {
        Self {
            limit,
            left: i128::from(limit),
            window: 0,
        }
    }

    fn balance(self, window: u64) -> i128 {
        let credit = i128::from(self.limit);
        (self.left + i128::from(window - self.window) * credit).min(credit)
    }

    fn remaining(self, window: u64) -> u64 {
        self.balance(window).max(0) as u64
    }

    fn charge(&mut self, window: u64, ticks: u64) {
        self.left = self.balance(window) - i128::from(ticks);
        self.window = window;
    }

    // Undo the reservation, then book the actual continuous interval. At each
    // boundary positive credit expires, while negative credit absorbs refill.
    fn settle(&mut self, elapsed: u64, reserved: u64, offset: u64, period: u64) -> u64 {
        let available = self.left + i128::from(reserved);
        let first = period - offset;
        if elapsed < first {
            self.left = available - i128::from(elapsed);
        } else {
            let tail = elapsed - first;
            let full = tail / period;
            let limit = i128::from(self.limit);
            let first_refill = (available - i128::from(first) + limit).min(limit);
            self.left = first_refill
                - i128::from(full) * i128::from(period - self.limit)
                - i128::from(tail % period);
            self.window += 1 + full;
        }
        (-self.left).max(0) as u64
    }

    fn wait_windows(self, window: u64) -> u64 {
        let balance = self.balance(window);
        if balance > 0 {
            0
        } else {
            ((-balance) as u64 / self.limit).saturating_add(1)
        }
    }
}

pub(crate) struct Hierarchy<const P: usize, const C: usize, const L: usize> {
    start: u64,
    period: u64,
    last_grant_start: u64,
    committed_end: u64,
    open: Option<(usize, usize, usize)>,
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
            open: None,
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

    /// Returns actual elapsed ticks, compare overrun, and the three debts.
    pub(crate) fn finish_at(&mut self, now: u64) -> Option<(u64, u64, [u64; 3])> {
        let elapsed = now.checked_sub(self.last_grant_start)?;
        let (p, c, l) = self.open.take()?;
        let reserved = self.committed_end - self.last_grant_start;
        let offset = (self.last_grant_start - self.start) % self.period;
        let settle = |budget: &mut Budget| budget.settle(elapsed, reserved, offset, self.period);
        let debts = [
            settle(&mut self.parents[p]),
            settle(&mut self.children[c].1),
            settle(&mut self.leaves[l].1),
        ];
        self.committed_end = now;
        self.last_grant_start = now;
        Some((elapsed, elapsed.saturating_sub(reserved), debts))
    }

    /// Earliest start of a positive-duration grant for this ready mask.
    /// A query before the committed end cannot describe idle recovery.
    pub(crate) fn next_actionable(&self, now: u64, ready: &[bool; L]) -> Option<u64> {
        if now < self.committed_end || now == u64::MAX {
            return None;
        }
        let elapsed = now - self.start;
        let window = elapsed / self.period;
        let wait = self
            .leaves
            .iter()
            .enumerate()
            .filter(|(id, _)| ready[*id])
            .map(|(_, &(child, leaf))| {
                let (parent, child_budget) = self.children[child];
                leaf.wait_windows(window)
                    .max(child_budget.wait_windows(window))
                    .max(self.parents[parent].wait_windows(window))
            })
            .min()?;
        if wait == 0 {
            return Some(now);
        }
        let offset = window.checked_add(wait)?.checked_mul(self.period)?;
        let target = self.start.checked_add(offset)?;
        (target < u64::MAX).then_some(target)
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
        self.open = Some((parent, child, leaf));
        self.last_grant_start = now;
        self.committed_end = now + ticks;
        Some((parent, child, leaf, ticks))
    }
}
