//! Bounded CPU placement, mixed-criticality service and lock-priority decisions.
//! Callers supply ready order, job releases and a consistent lock-state snapshot.

const MAX_TASKS: usize = u32::BITS as usize;
const MAX_CPUS: usize = u8::BITS as usize;

/// An immutable allowed-CPU mask per task ID. A singleton fixes a permanent
/// home, a cluster mask permits migration inside the cluster, and a global
/// mask permits all configured CPUs. Offline CPUs can remain configured.
pub(crate) struct AffinityDomain<const N: usize> {
    allowed: [u8; N],
}

impl<const N: usize> AffinityDomain<N> {
    pub(crate) fn new(allowed: [u8; N]) -> Option<Self> {
        if N > MAX_TASKS || allowed.contains(&0) {
            return None;
        }
        Some(Self { allowed })
    }

    #[cfg(any(test, jnox_bench = "smp"))]
    pub(crate) fn allows(&self, cpu: u8, task: u8) -> bool {
        cpu < MAX_CPUS as u8
            && self.allowed.get(usize::from(task)).is_some_and(|mask| mask & (1 << cpu) != 0)
    }

    /// Maps online CPUs to distinct ready tasks for one simultaneous slot.
    /// Within that slot the augmenting paths maximize occupancy, preserving
    /// the inclusion of earlier-ranked tasks at the expense of later ones.
    /// CPU IDs break search ties. The caller owns release and admission.
    pub(crate) fn place(&self, online: u8, ranked: &[u8]) -> Option<[Option<u8>; MAX_CPUS]> {
        if ranked.len() > N {
            return None;
        }
        let mut seen_tasks = 0u32;
        let mut owners = [None; MAX_CPUS];
        for &id in ranked {
            if usize::from(id) >= N || seen_tasks & (1u32 << id) != 0 {
                return None;
            }
            seen_tasks |= 1u32 << id;
            let mut seen_cpus = 0;
            self.augment(id, online, &mut owners, &mut seen_cpus);
        }
        Some(owners)
    }

    /// Each search visits a CPU at most once and recurses through at most
    /// MAX_CPUS owners. A failed search leaves all existing owners in place.
    fn augment(
        &self,
        id: u8,
        online: u8,
        owners: &mut [Option<u8>; MAX_CPUS],
        seen_cpus: &mut u8,
    ) -> bool {
        let allowed = self.allowed[usize::from(id)] & online;
        while allowed & !*seen_cpus != 0 {
            let cpu = (allowed & !*seen_cpus).trailing_zeros() as usize;
            *seen_cpus |= 1 << cpu;
            if let Some(previous) = owners[cpu] {
                if !self.augment(previous, online, owners, seen_cpus) {
                    continue;
                }
            }
            owners[cpu] = Some(id);
            return true;
        }
        false
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ExecutionBudget {
    Low(u64),
    High { lo: u64, hi: u64 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CriticalityMode {
    Low,
    High,
}

/// Two-criticality, serialized service accounting for at most one current
/// job per task. The caller releases and finishes jobs, and consumes exactly
/// the granted service before the next accounting decision.
pub(crate) struct CriticalityDomain<const N: usize> {
    budgets: [ExecutionBudget; N],
    used: [Option<u64>; N],
    mode: CriticalityMode,
}

impl<const N: usize> CriticalityDomain<N> {
    pub(crate) fn new(budgets: [ExecutionBudget; N]) -> Option<Self> {
        if N > MAX_TASKS
            || budgets.iter().any(|budget| match *budget {
                ExecutionBudget::Low(lo) => lo == 0,
                ExecutionBudget::High { lo, hi } => lo == 0 || hi < lo,
            })
        {
            return None;
        }
        Some(Self {
            budgets,
            used: [None; N],
            mode: CriticalityMode::Low,
        })
    }

    pub(crate) fn mode(&self) -> CriticalityMode {
        self.mode
    }

    pub(crate) fn release(&mut self, id: usize) -> bool {
        let Some(&budget) = self.budgets.get(id) else {
            return false;
        };
        if self.used[id].is_some()
            || (self.mode == CriticalityMode::High && matches!(budget, ExecutionBudget::Low(_)))
        {
            return false;
        }
        self.used[id] = Some(0);
        true
    }

    pub(crate) fn finish(&mut self, id: usize) -> bool {
        self.used.get_mut(id).and_then(Option::take).is_some()
    }

    /// Commits up to the requested service. In LO mode a HI job reaching its
    /// LO bound stops there. A subsequent positive grant beyond the bound
    /// switches mode before that service and discards all active LO jobs.
    /// An exhausted job gets zero. Unknown/inactive jobs and unrepresentable
    /// cumulative requests return None without changing any state.
    pub(crate) fn grant(&mut self, id: usize, requested: u64) -> Option<u64> {
        let used = (*self.used.get(id)?)?;
        let requested_end = used.checked_add(requested)?;
        let budget = self.budgets[id];
        let limit = match budget {
            ExecutionBudget::Low(lo) => lo,
            ExecutionBudget::High { lo, .. } if self.mode == CriticalityMode::Low && used < lo => {
                lo
            }
            ExecutionBudget::High { hi, .. } => hi,
        };
        let end = requested_end.min(limit);
        if self.mode == CriticalityMode::Low
            && matches!(budget, ExecutionBudget::High { lo, .. } if end > lo)
        {
            self.mode = CriticalityMode::High;
            for (used, budget) in self.used.iter_mut().zip(&self.budgets) {
                if matches!(budget, ExecutionBudget::Low(_)) {
                    *used = None;
                }
            }
        }
        self.used[id] = Some(end);
        Some(end - used)
    }

    /// The caller supplies external restart approval. All surviving jobs
    /// must finish before a new LO-mode workload can be released.
    pub(crate) fn reset(&mut self, approved: bool) -> bool {
        if !approved || self.mode != CriticalityMode::High || self.used.iter().any(Option::is_some)
        {
            return false;
        }
        self.mode = CriticalityMode::Low;
        true
    }
}

#[derive(Clone, Copy)]
pub(crate) struct LockPriority {
    pub(crate) base: u8,
    /// Maximum of the declared ceilings on locks currently held by this task.
    pub(crate) held_ceiling: Option<u8>,
    /// Owner of the mutex this task currently waits on, if any.
    pub(crate) waits_for: Option<u8>,
}

/// Recomputes effective priorities from the entire snapshot. Each wait edge
/// donates the waiter's effective priority to its owner. A topological pass
/// rejects cycles, including cycles in disconnected components. The mutex
/// implementation owns acquisition, blocking, timeout and unlock.
pub(crate) fn effective_priorities<const N: usize>(locks: &[LockPriority; N]) -> Option<[u8; N]> {
    if N > MAX_TASKS {
        return None;
    }
    let mut effective = [0; N];
    let mut donors = [0u8; N];
    let mut queue = [0u8; N];
    for (id, lock) in locks.iter().enumerate() {
        effective[id] = lock.base.max(lock.held_ceiling.unwrap_or(0));
        if let Some(owner) = lock.waits_for {
            *donors.get_mut(usize::from(owner))? += 1;
        }
    }
    let mut tail = 0;
    for (id, &count) in donors.iter().enumerate() {
        if count == 0 {
            queue[tail] = id as u8;
            tail += 1;
        }
    }
    let mut head = 0;
    while head < tail {
        let id = usize::from(queue[head]);
        head += 1;
        if let Some(owner) = locks[id].waits_for {
            let owner = usize::from(owner);
            effective[owner] = effective[owner].max(effective[id]);
            donors[owner] -= 1;
            if donors[owner] == 0 {
                queue[tail] = owner as u8;
                tail += 1;
            }
        }
    }
    (head == N).then_some(effective)
}

fn placement_checks() -> bool {
    use core::hint::black_box;
    // Global, clustered, fixed-home, and contending fixed-home tasks.
    let Some(domain) = AffinityDomain::new(black_box([0b111, 0b011, 0b001, 0b001])) else {
        return false;
    };
    let ranked = black_box([0, 1, 2, 3]);
    let all = [Some(2), Some(1), Some(0), None, None, None, None, None];
    // If global gets the first available CPU permanently, the third task
    // cannot run. The third task needs both earlier owners rearranged.
    let reversed = [Some(3), Some(1), Some(0), None, None, None, None, None];
    let reduced = [Some(1), None, Some(0), None, None, None, None, None];
    let top = [None, None, None, None, None, None, None, Some(0)];
    domain.place(0b111, &ranked) == Some(all)
        && domain.place(0b111, &black_box([3, 2, 1, 0])) == Some(reversed)
        && domain.place(0b101, &ranked) == Some(reduced)
        && domain.place(0, &ranked) == Some([None; MAX_CPUS])
        && domain.place(0b111, &black_box([0, 1, 2, 2])).is_none()
        && domain.place(0b111, &black_box([4])).is_none()
        && AffinityDomain::new(black_box([0u8])).is_none()
        && AffinityDomain::new(black_box([1u8; MAX_TASKS + 1])).is_none()
        && AffinityDomain::new(black_box([0b1000_0000])).and_then(|d| d.place(0xff, &[0]))
            == Some(top)
}

fn criticality_checks() -> bool {
    use ExecutionBudget::{High, Low};
    use core::hint::black_box;
    let Some(mut domain) = CriticalityDomain::new(black_box([
        Low(3),
        High { lo: 2, hi: 5 },
        High { lo: 1, hi: 4 },
    ])) else {
        return false;
    };
    if !(domain.release(0) && domain.release(1) && domain.release(2))
        || domain.release(0)
        || domain.release(3)
        || domain.grant(0, 8) != Some(3)
        || domain.grant(0, 1) != Some(0)
        || !domain.finish(0)
        || !domain.release(0)
        || domain.grant(0, 1) != Some(1)
        || domain.grant(1, 4) != Some(2)
        || domain.grant(1, 0) != Some(0)
        || domain.grant(1, u64::MAX).is_some()
        || domain.mode() != CriticalityMode::Low
        || domain.grant(1, 1) != Some(1)
        || domain.mode() != CriticalityMode::High
        || domain.grant(0, 1).is_some()
        || domain.finish(0)
        || domain.release(0)
        || domain.grant(1, 10) != Some(2)
        || domain.grant(1, 1) != Some(0)
        || domain.grant(2, 4) != Some(4)
        || !domain.finish(1)
        || domain.reset(true)
        || !domain.finish(2)
        || domain.reset(false)
        || !domain.reset(true)
        || domain.mode() != CriticalityMode::Low
        || domain.grant(0, 1).is_some()
        || !domain.release(0)
        || domain.grant(0, 1) != Some(1)
    {
        return false;
    }
    let Some(mut edge) = CriticalityDomain::new(black_box([High {
        lo: u64::MAX - 1,
        hi: u64::MAX,
    }])) else {
        return false;
    };
    edge.release(0)
        && edge.grant(0, u64::MAX) == Some(u64::MAX - 1)
        && edge.grant(0, 2).is_none()
        && edge.mode() == CriticalityMode::Low
        && edge.grant(0, 1) == Some(1)
        && edge.mode() == CriticalityMode::High
        && edge.grant(0, 1).is_none()
        && CriticalityDomain::new(black_box([Low(0)])).is_none()
        && CriticalityDomain::new(black_box([High { lo: 0, hi: 1 }])).is_none()
        && CriticalityDomain::new(black_box([High { lo: 3, hi: 2 }])).is_none()
}

fn priority_checks() -> bool {
    use core::hint::black_box;
    let mut locks = black_box([
        LockPriority {
            base: 1,
            held_ceiling: Some(6),
            waits_for: None,
        },
        LockPriority {
            base: 3,
            held_ceiling: None,
            waits_for: Some(0),
        },
        LockPriority {
            base: 2,
            held_ceiling: Some(9),
            waits_for: Some(1),
        },
        LockPriority {
            base: 7,
            held_ceiling: None,
            waits_for: Some(1),
        },
    ]);
    if effective_priorities(&locks) != Some([9, 9, 9, 7]) {
        return false;
    }
    locks[2].waits_for = None; // timeout removes the strongest donation
    if effective_priorities(&locks) != Some([7, 7, 9, 7]) {
        return false;
    }
    locks[1].waits_for = None; // owner 0 retains its held ceiling
    if effective_priorities(&locks) != Some([6, 7, 9, 7]) {
        return false;
    }
    locks[0].held_ceiling = None;
    if effective_priorities(&locks) != Some([1, 7, 9, 7]) {
        return false;
    }
    locks[1].waits_for = Some(3);
    if effective_priorities(&locks).is_some() {
        return false;
    }
    locks[1].waits_for = Some(1);
    if effective_priorities(&locks).is_some() {
        return false;
    }
    locks[1].waits_for = Some(4);
    effective_priorities(&locks).is_none()
}

pub(crate) fn bench_checks() -> bool {
    placement_checks() && criticality_checks() && priority_checks()
}
