//! Single-CPU strict-class MLQ and three-level MLFQ. A scheduling boundary
//! ends one dispatch and charges the task's measured service for that dispatch.
//! The caller supplies service units and decides when to trigger a global boost.

use super::feedback_policy::FeedbackTask;
use super::ready_queue::ReadyQueue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    Yield,
    Sleep,
    Expire,
}

enum Ranking<const N: usize> {
    Fixed { classes: [u8; N], quantum: u64 },
    Feedback([FeedbackTask; N]),
}

impl<const N: usize> Ranking<N> {
    fn priority(&self, id: usize) -> u8 {
        match self {
            Self::Fixed { classes, .. } => classes[id],
            Self::Feedback(tasks) => tasks[id].priority(),
        }
    }
}

pub(crate) struct Multilevel<const N: usize> {
    ranking: Ranking<N>,
    ready: ReadyQueue<N>,
    asleep: [bool; N],
    running: Option<usize>,
    #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
    pub(crate) resets: u64,
}

impl<const N: usize> Multilevel<N> {
    fn new(ranking: Ranking<N>) -> Self {
        Self {
            ranking,
            ready: ReadyQueue::new(),
            asleep: [true; N],
            running: None,
            #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
            resets: 0,
        }
    }

    /// A larger class number wins every dispatch. A continuously ready higher
    /// class can therefore deny all service to a lower class. Each fixed-class
    /// dispatch gets a fresh quantum, and equal classes rotate in FIFO order.
    pub(crate) fn fixed(classes: [u8; N], quantum: u64) -> Option<Self> {
        if N == 0 || quantum == 0 {
            return None;
        }
        Some(Self::new(Ranking::Fixed { classes, quantum }))
    }

    /// Allotments are specified from highest level to lowest. The remaining
    /// allotment at the current level bounds the next dispatch grant.
    pub(crate) fn feedback(allotments: [u64; 3]) -> Option<Self> {
        if N == 0 {
            return None;
        }
        FeedbackTask::new(allotments)?;
        let tasks = core::array::from_fn(|_| {
            FeedbackTask::new(allotments).expect("validated feedback allotments")
        });
        Some(Self::new(Ranking::Feedback(tasks)))
    }

    /// Initially all tasks are asleep. A wake appends to the ready queue and
    /// leaves the current dispatch and the waking task's accounting intact.
    pub(crate) fn wake(&mut self, id: usize) -> bool {
        if self.asleep.get(id) != Some(&true) || !self.ready.push_back(id) {
            return false;
        }
        self.asleep[id] = false;
        true
    }

    pub(crate) fn priority(&self, id: usize) -> Option<u8> {
        (id < N).then(|| self.ranking.priority(id))
    }

    pub(crate) fn remaining(&self, id: usize) -> Option<u64> {
        if id >= N {
            return None;
        }
        Some(match &self.ranking {
            Ranking::Fixed { quantum, .. } => *quantum,
            Ranking::Feedback(tasks) => tasks[id].remaining(),
        })
    }

    /// Removes the earliest ready task at the highest rank. Until stop closes
    /// this dispatch, a second dispatch cannot open another service interval.
    pub(crate) fn dispatch(&mut self) -> Option<usize> {
        if self.running.is_some() {
            return None;
        }
        let ranking = &self.ranking;
        self.running = self.ready.pop_highest(|id| ranking.priority(id));
        self.running
    }

    /// Elapsed is service consumed since this dispatch started. The caller
    /// bounds that interval by the grant and excludes other tasks and idle
    /// time. Expiry consumes the full grant. A yield or sleep may also exhaust
    /// the level exactly. Only this transition charges the running task.
    pub(crate) fn stop(&mut self, id: usize, elapsed: u64, why: Stop) -> bool {
        if self.running != Some(id) {
            return false;
        }
        let grant = self.remaining(id).expect("running task has a grant");
        if elapsed > grant || (why == Stop::Expire && elapsed != grant) {
            return false;
        }
        if let Ranking::Feedback(tasks) = &mut self.ranking {
            assert!(tasks[id].charge(elapsed));
        }
        if why == Stop::Sleep {
            self.asleep[id] = true;
        } else {
            // A running task owns the slot vacated by dispatch.
            assert!(self.ready.push_back(id));
        }
        self.running = None;
        true
    }

    #[cfg(jnox_multilevel)]
    pub(crate) fn close_interval(&mut self, id: usize, elapsed: u64, blocked: bool) -> bool {
        if self.running != Some(id) { return false; }
        if let Ranking::Feedback(tasks) = &mut self.ranking {
            tasks[id].charge_interval(elapsed);
        }
        if blocked { self.asleep[id] = true; }
        else { assert!(self.ready.push_back(id)); }
        self.running = None;
        true
    }

    #[cfg(any(jnox_multilevel_trace, jnox_mlfq_wake))]
    pub(crate) fn queued(&self) -> &[usize] { self.ready.entries() }

    #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
    pub(crate) fn asleep(&self) -> &[bool; N] { &self.asleep }

    /// At a caller-selected scheduling boundary, reset every feedback task
    /// (including sleepers). Existing ready order breaks the newly formed ties.
    pub(crate) fn boost(&mut self) -> bool {
        if self.running.is_some() {
            return false;
        }
        let Ranking::Feedback(tasks) = &mut self.ranking else {
            return false;
        };
        #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
        { self.resets = self.resets.checked_add(1).expect("reset count overflow"); }
        for task in tasks {
            task.boost();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Multilevel, Stop};

    #[test]
    fn saturated_high_class_rotates_in_arrival_order_and_excludes_low() {
        let mut q = Multilevel::fixed([1, 9, 9], 2).unwrap();
        for id in [0, 2, 1] {
            assert!(q.wake(id));
        }
        for id in [2, 1, 2, 1] {
            assert_eq!(q.dispatch(), Some(id));
            assert!(q.stop(id, 2, Stop::Expire));
        }
        for id in [2, 1] {
            assert_eq!(q.dispatch(), Some(id));
            assert!(q.stop(id, 1, Stop::Sleep));
            assert_eq!(q.remaining(id), Some(2));
        }
        assert!(!q.boost());
        assert_eq!(q.dispatch(), Some(0));
    }

    #[test]
    fn feedback_carries_yields_and_sleep_and_preserves_order_on_boost() {
        let mut q = Multilevel::<3>::feedback([3, 4, 5]).unwrap();
        for id in [2, 1, 0] {
            assert!(q.wake(id));
        }
        assert_eq!(q.dispatch(), Some(2));
        assert!(q.stop(2, 1, Stop::Yield));
        assert_eq!(q.dispatch(), Some(1));
        assert!(q.stop(1, 1, Stop::Sleep));
        assert_eq!(q.dispatch(), Some(0));
        assert!(q.stop(0, 3, Stop::Sleep));
        assert_eq!(q.priority(0), Some(1));

        // The two early yields consumed separate pieces of one top allotment.
        assert_eq!(q.dispatch(), Some(2));
        assert!(q.stop(2, 1, Stop::Yield));
        assert_eq!(q.remaining(2), Some(1));
        assert_eq!(q.dispatch(), Some(2));
        assert!(q.stop(2, 1, Stop::Expire));
        assert_eq!(q.priority(2), Some(1));
        assert!(q.wake(1));
        assert_eq!(q.dispatch(), Some(1));
        assert_eq!(q.remaining(1), Some(2));
        assert!(q.stop(1, 0, Stop::Sleep));
        assert_eq!(q.dispatch(), Some(2));
        assert!(q.stop(2, 4, Stop::Expire));
        assert_eq!(q.priority(2), Some(0));
        assert_eq!(q.dispatch(), Some(2));
        assert!(q.stop(2, 5, Stop::Expire));
        assert_eq!(q.priority(2), Some(0));
        assert_eq!(q.remaining(2), Some(5));

        assert!(q.wake(1));
        assert!(q.boost());
        for id in 0..3 {
            assert_eq!((q.priority(id), q.remaining(id)), (Some(2), Some(3)));
        }
        assert!(q.wake(0));
        for id in [2, 1, 0] {
            assert_eq!(q.dispatch(), Some(id));
            assert!(q.stop(id, 0, Stop::Sleep));
        }
        assert_eq!(q.dispatch(), None);
    }

    #[test]
    fn invalid_or_repeated_stop_leaves_the_open_interval_unchanged() {
        assert!(Multilevel::<0>::fixed([], 1).is_none());
        assert!(Multilevel::<0>::feedback([1; 3]).is_none());
        assert!(Multilevel::<2>::fixed([1; 2], 0).is_none());
        assert!(Multilevel::<2>::feedback([1, 0, 1]).is_none());
        let mut q = Multilevel::<2>::feedback([2, 4, u64::MAX]).unwrap();
        assert_eq!(q.dispatch(), None);
        assert!(!q.wake(2));
        assert!(q.wake(1));
        assert!(!q.wake(1));
        assert_eq!(q.dispatch(), Some(1));
        assert_eq!(q.dispatch(), None);
        assert!(!q.wake(1));
        assert!(!q.boost());
        assert!(!q.stop(0, 1, Stop::Sleep));
        assert!(!q.stop(1, 1, Stop::Expire));
        assert!(!q.stop(1, u64::MAX, Stop::Yield));
        assert_eq!(q.remaining(1), Some(2));
        assert!(q.stop(1, 1, Stop::Yield));
        assert!(!q.stop(1, 1, Stop::Yield));
        assert_eq!(q.remaining(1), Some(1));
        assert_eq!(q.dispatch(), Some(1));
        assert!(q.stop(1, 1, Stop::Yield));
        assert_eq!((q.priority(1), q.remaining(1)), (Some(1), Some(4)));
        assert_eq!((q.priority(2), q.remaining(2)), (None, None));
    }
}
