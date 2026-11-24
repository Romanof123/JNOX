//! Bounded single-CPU EEVDF. Each task has one physical-service request.
//! Partial execution advances runtime and keeps its original virtual deadline.
//! Only the running task may consume service. Membership and configuration
//! change at scheduling boundaries; the caller controls those boundaries.

use super::fair_policy::{pick_eevdf, weighted_reference, FairRequest};

const MAX_TASKS: usize = 32;
const MAX_WEIGHT: u32 = 16;
// This bounds the sum of N runtime numerators in the externally observable V.
const MAX_NUMERATOR: u64 = u64::MAX / MAX_TASKS as u64;

#[derive(Clone, Copy)]
struct Task {
    weight: u32,
    slice: u32,
    start: u64, // virtual request start × weight
    used: u32,
    ready: bool,
}

impl Task {
    fn runtime(self) -> u64 {
        self.start + u64::from(self.used)
    }

    fn deadline(self) -> u64 {
        self.start + u64::from(self.slice)
    }

    fn request(self) -> Option<FairRequest> {
        let weight = u64::from(self.weight);
        FairRequest::new(
            self.weight,
            self.start / weight,
            (self.start % weight) as u32,
            u64::from(self.slice),
            u64::from(self.used),
        )
    }
}

pub(crate) struct Eevdf<const N: usize> {
    tasks: [Task; N],
    running: Option<usize>,
    // Remember the last runnable entity's V when the CPU goes idle.
    idle_reference: (u64, u64),
}

impl<const N: usize> Eevdf<N> {
    pub(crate) fn new(weights: [u32; N], slices: [u32; N]) -> Option<Self> {
        if N == 0
            || N > MAX_TASKS
            || weights.iter().any(|w| !(1..=MAX_WEIGHT).contains(w))
            || slices.contains(&0)
        {
            return None;
        }
        let tasks = core::array::from_fn(|id| Task {
            weight: weights[id],
            slice: slices[id],
            start: 0,
            used: 0,
            ready: false,
        });
        Some(Self { tasks, running: None, idle_reference: (0, 1) })
    }

    /// Both virtual times are supplied as numerators over the returned weight.
    pub(crate) fn snapshot(&self, id: usize) -> Option<(u64, u64, u32)> {
        let task = *self.tasks.get(id)?;
        Some((task.runtime(), task.deadline(), task.weight))
    }

    pub(crate) fn remaining(&self, id: usize) -> Option<u32> {
        let task = self.tasks.get(id)?;
        Some(task.slice - task.used)
    }

    /// Exact V, unreduced: sum(weight × runtime) / sum(weight) of all ready
    /// tasks including the incumbent. There is no live reference when idle.
    pub(crate) fn reference(&self) -> Option<(u64, u64)> {
        let mut numerator = 0;
        let mut denominator = 0;
        for task in self.tasks.iter().filter(|task| task.ready) {
            numerator += task.runtime();
            denominator += u64::from(task.weight);
        }
        (denominator != 0).then_some((numerator, denominator))
    }

    /// Clamp stale sleeper credit to the ready set's V, rounding upward to
    /// the waking weight's lattice. Keep positive service debt and move a
    /// partial request's deadline by exactly the same displacement as runtime.
    pub(crate) fn wake(&mut self, id: usize) -> bool {
        let Some(&task) = self.tasks.get(id) else { return false };
        if self.running.is_some() || task.ready {
            return false;
        }
        let (n, d) = self.reference().unwrap_or(self.idle_reference);
        let floor = (u128::from(n) * u128::from(task.weight)).div_ceil(u128::from(d));
        let shift = floor.saturating_sub(u128::from(task.runtime()));
        if u128::from(task.deadline()) + shift > u128::from(MAX_NUMERATOR) {
            return false;
        }
        self.tasks[id].start += shift as u64;
        self.tasks[id].ready = true;
        true
    }

    pub(crate) fn sleep(&mut self, id: usize) -> bool {
        let Some(&task) = self.tasks.get(id) else { return false };
        if self.running.is_some() || !task.ready {
            return false;
        }
        self.tasks[id].ready = false;
        if !self.tasks.iter().any(|other| other.ready) {
            self.idle_reference = (task.runtime(), u64::from(task.weight));
        }
        true
    }

    /// A changed slice or weight replaces the outstanding request at its
    /// current runtime. Already received CPU service is never undone. For a
    /// reweight, preserve signed service lag w*V-runtime_n relative to the
    /// pre-change V. Round the new numerator upward by less than one physical
    /// service tick. An unrepresentable negative start or horizon rejects the
    /// operation atomically. An identical configuration keeps a partial request.
    pub(crate) fn configure(&mut self, id: usize, weight: u32, slice: u32) -> bool {
        let Some(&old) = self.tasks.get(id) else { return false };
        if self.running.is_some() || !(1..=MAX_WEIGHT).contains(&weight) || slice == 0 {
            return false;
        }
        if (weight, slice) == (old.weight, old.slice) {
            return true;
        }
        let (n, d) = self.reference().unwrap_or(self.idle_reference);
        let target = i128::from(old.runtime()) * i128::from(d)
            + (i128::from(weight) - i128::from(old.weight)) * i128::from(n);
        if target < 0 {
            return false;
        }
        let start = (target as u128).div_ceil(u128::from(d));
        if start + u128::from(slice) > u128::from(MAX_NUMERATOR) {
            return false;
        }
        self.tasks[id] = Task { weight, slice, start: start as u64, used: 0, ready: old.ready };
        true
    }

    /// Lower task ID breaks equal eligible deadlines. A single immutable
    /// snapshot supplies both the shared reference and the eligibility test.
    pub(crate) fn dispatch(&mut self) -> Option<usize> {
        if self.running.is_some() {
            return None;
        }
        let first = self.tasks.iter().find(|task| task.ready)?.request()?;
        let mut requests = [first; N];
        let mut ids = [0; N];
        let mut len = 0;
        for (id, task) in self.tasks.iter().enumerate().filter(|(_, task)| task.ready) {
            requests[len] = task.request()?;
            ids[len] = id;
            len += 1;
        }
        let runnable = &requests[..len];
        let position = pick_eevdf(runnable, weighted_reference(runnable)?)?;
        self.running = Some(ids[position]);
        self.running
    }

    /// Charge a measured interval in full even if IRQ delivery crossed a
    /// request boundary. Only the current owner accrues the overshoot. Check
    /// the final deadline before changing state, independent of renewal count.
    #[cfg(any(jnox_eevdf_app, test))]
    pub(crate) fn stop_elapsed(&mut self, id: usize, elapsed: u64) -> bool {
        if self.running != Some(id) || elapsed == 0 {
            return false;
        }
        let task = self.tasks[id];
        if elapsed <= u64::from(task.slice - task.used) {
            return self.stop(id, elapsed as u32);
        }
        let service = u128::from(task.used) + u128::from(elapsed);
        let slice = u128::from(task.slice);
        let partial = service % slice;
        let start = u128::from(task.start) + service - partial;
        if start + slice > u128::from(MAX_NUMERATOR) {
            return false;
        }
        self.tasks[id].start = start as u64;
        self.tasks[id].used = partial as u32;
        self.running = None;
        true
    }

    /// Close the current interval once, charging its actual service. A fully
    /// served request renews at its old deadline with the configured slice.
    /// Excess service and a renewal beyond the horizon leave the interval open.
    pub(crate) fn stop(&mut self, id: usize, service: u32) -> bool {
        if self.running != Some(id) {
            return false;
        }
        let task = &mut self.tasks[id];
        if service > task.slice - task.used {
            return false;
        }
        if service == task.slice - task.used {
            let next = task.deadline();
            if next + u64::from(task.slice) > MAX_NUMERATOR {
                return false;
            }
            task.start = next;
            task.used = 0;
        } else {
            task.used += service;
        }
        self.running = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Eevdf, MAX_NUMERATOR};

    #[test]
    fn elapsed_service_preserves_the_owner_across_late_renewals() {
        let mut policy = Eevdf::new([2], [5]).unwrap();
        assert!(policy.wake(0));
        assert_eq!(policy.dispatch(), Some(0));
        assert!(policy.stop_elapsed(0, 2));
        assert_eq!(policy.dispatch(), Some(0));
        let before = policy.snapshot(0);
        assert!(!policy.stop(0, 15));
        assert!(!policy.stop_elapsed(1, 15));
        assert!(!policy.stop_elapsed(0, 0));
        assert!(!policy.stop_elapsed(0, u64::MAX));
        assert_eq!(policy.snapshot(0), before);
        assert!(policy.stop_elapsed(0, 15));
        assert_eq!(policy.snapshot(0), Some((17, 20, 2)));
        assert_eq!(policy.remaining(0), Some(3));
        assert!(!policy.stop_elapsed(0, 3));
        assert!(policy.sleep(0));
        assert_eq!(policy.dispatch(), None);
        assert!(policy.wake(0));
        assert_eq!(policy.dispatch(), Some(0));
        assert!(policy.stop_elapsed(0, 3));
        assert_eq!(policy.snapshot(0), Some((20, 25, 2)));
    }

    #[test]
    fn an_unrepresentable_renewal_reweight_or_wake_is_atomic() {
        let mut policy = Eevdf::new([1, 16], [2, 1]).unwrap();
        policy.tasks[0].start = MAX_NUMERATOR - 2;
        assert!(policy.wake(0));
        assert!(!policy.configure(0, 1, 3));
        assert_eq!(policy.dispatch(), Some(0));
        assert!(!policy.stop(0, 2));
        assert_eq!(policy.remaining(0), Some(2));
        assert!(policy.stop(0, 1));
        assert_eq!(policy.snapshot(0), Some((MAX_NUMERATOR - 1, MAX_NUMERATOR, 1)));
        assert!(policy.sleep(0));
        assert!(!policy.wake(1));
        assert_eq!(policy.snapshot(1), Some((0, 1, 16)));
        assert_eq!(policy.dispatch(), None);
    }
}
