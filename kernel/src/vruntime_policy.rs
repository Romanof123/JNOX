//! Single-CPU weighted virtual runtime for at most 32 fixed-weight tasks.
//! All tasks start ready at zero. Each scheduling boundary follows retirement
//! of the previous owner's measured service interval.

use super::fair_policy::VirtualTime;
use core::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lease {
    pub(crate) task: usize,
    pub(crate) sequence: u64,
}

#[derive(Clone, Copy)]
struct Task {
    runtime: VirtualTime,
    service: u64,
    ready: bool,
}

pub(crate) struct VruntimeScheduler<const N: usize> {
    tasks: [Task; N],
    floor: VirtualTime,
    owner: Option<Lease>,
    sequence: u64,
}

impl<const N: usize> VruntimeScheduler<N> {
    pub(crate) fn new(weights: [u32; N]) -> Option<Self> {
        if N == 0 || N > 32 {
            return None;
        }
        let zero = VirtualTime::new(0, 0, 1)?;
        let mut tasks = [Task {
            runtime: zero,
            service: 0,
            ready: true,
        }; N];
        for (task, weight) in tasks.iter_mut().zip(weights) {
            task.runtime = VirtualTime::new(0, 0, u64::from(weight))?;
        }
        Some(Self {
            tasks,
            floor: zero,
            owner: None,
            sequence: 0,
        })
    }

    pub(crate) fn runtime(&self, id: usize) -> Option<VirtualTime> {
        self.tasks.get(id).map(|task| task.runtime)
    }

    pub(crate) fn service(&self, id: usize) -> Option<u64> {
        self.tasks.get(id).map(|task| task.service)
    }

    pub(crate) fn floor(&self) -> VirtualTime {
        self.floor
    }

    /// The ready set includes the owner until its interval has been charged.
    /// Enumerating in ascending ID order resolves equal rational timestamps.
    fn first_ready(&self) -> Option<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| task.ready)
            .min_by(|(_, a), (_, b)| a.runtime.compare(b.runtime))
            .map(|(id, _)| id)
    }

    fn advance_floor(&mut self) {
        if let Some(id) = self.first_ready() {
            let minimum = self.tasks[id].runtime;
            if minimum.compare(self.floor) == Ordering::Greater {
                self.floor = minimum;
            }
        }
    }

    /// Lease sequences identify one outstanding interval within this scheduler.
    pub(crate) fn dispatch(&mut self) -> Option<Lease> {
        if self.owner.is_some() {
            return None;
        }
        let task = self.first_ready()?;
        let sequence = self.sequence.checked_add(1)?;
        let lease = Lease { task, sequence };
        self.owner = Some(lease);
        self.sequence = sequence;
        Some(lease)
    }

    /// Add actual physical counter ticks to the owner and ticks/weight to its
    /// virtual runtime. Failure leaves both the grant and its accounting intact.
    pub(crate) fn charge(&mut self, lease: Lease, ticks: u64) -> bool {
        if ticks == 0 || self.owner != Some(lease) {
            return false;
        }
        let task = &mut self.tasks[lease.task];
        let Some(service) = task.service.checked_add(ticks) else {
            return false;
        };
        let Some(runtime) = task.runtime.after_service(ticks) else {
            return false;
        };
        task.runtime = runtime;
        task.service = service;
        self.owner = None;
        self.advance_floor();
        true
    }

    pub(crate) fn sleep(&mut self, id: usize) -> bool {
        let Some(task) = self.tasks.get_mut(id) else {
            return false;
        };
        if self.owner.is_some() || !task.ready {
            return false;
        }
        task.ready = false;
        self.advance_floor();
        true
    }

    /// Preserve an ahead sleeper's service debt. Place a behind sleeper at the
    /// smallest timestamp on its weight grid at or above the current floor.
    /// Signed service lag is weight * (floor - runtime), in physical ticks.
    pub(crate) fn wake(&mut self, id: usize) -> bool {
        let Some(task) = self.tasks.get_mut(id) else {
            return false;
        };
        if self.owner.is_some() || task.ready {
            return false;
        }
        if task.runtime.compare(self.floor) == Ordering::Less {
            let (whole, numerator, denominator) = self.floor.parts();
            let (_, _, weight) = task.runtime.parts();
            let scaled = u128::from(numerator) * u128::from(weight);
            let remainder = scaled.div_ceil(u128::from(denominator)) as u64;
            // The rounded fractional part lies in [0, weight].
            let Some(whole) = whole.checked_add(remainder / weight) else {
                return false;
            };
            let Some(placed) = VirtualTime::new(whole, remainder % weight, weight) else {
                return false;
            };
            task.runtime = placed;
        }
        task.ready = true;
        self.advance_floor();
        true
    }
}
