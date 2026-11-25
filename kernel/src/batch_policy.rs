//! Bounded single-CPU batch scheduler. The incumbent receives all CPU time
//! between successive timestamps until explicitly completed or preempted.

use super::dispatch_policy::{DispatchJob, DispatchPolicy};
use super::ready_queue::ReadyQueue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchPolicy {
    Sjf,
    Srtf,
    Hrrn,
}

impl BatchPolicy {
    fn ranking(self) -> DispatchPolicy {
        match self {
            Self::Sjf => DispatchPolicy::ShortestJobFirst,
            Self::Srtf => DispatchPolicy::ShortestRemainingTimeFirst,
            Self::Hrrn => DispatchPolicy::HighestResponseRatioNext,
        }
    }
}

#[cfg(any(test, jnox_native_batch))]
#[path = "native_batch_policy.rs"]
mod native;
#[cfg(any(test, jnox_native_batch))]
pub(crate) use self::native::NativeBatchScheduler;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchError {
    TimeRegression,
    InvalidId,
    ZeroEstimate,
    Duplicate,
    NotRunning,
}

pub(crate) struct BatchScheduler<const N: usize> {
    policy: BatchPolicy,
    now: u64,
    running: Option<usize>,
    jobs: [Option<DispatchJob>; N],
    ready: ReadyQueue<N>,
}

impl<const N: usize> BatchScheduler<N> {
    pub(crate) const fn new(policy: BatchPolicy, start: u64) -> Self {
        Self {
            policy,
            now: start,
            running: None,
            jobs: [None; N],
            ready: ReadyQueue::new(),
        }
    }

    pub(crate) fn now(&self) -> u64 {
        self.now
    }

    pub(crate) fn remaining(&self, id: usize) -> Option<u64> {
        self.jobs.get(id)?.map(|job| job.remaining)
    }

    fn account(&mut self, now: u64) -> Result<(), BatchError> {
        let elapsed = now
            .checked_sub(self.now)
            .ok_or(BatchError::TimeRegression)?;
        if let Some(id) = self.running {
            let job = self.jobs[id].as_mut().expect("running job");
            job.remaining = job.remaining.saturating_sub(elapsed).max(1);
        }
        self.now = now;
        Ok(())
    }

    /// Enqueue without choosing a successor, so simultaneous releases can be
    /// considered together in their arrival order.
    pub(crate) fn release(&mut self, now: u64, id: usize, estimate: u64) -> Result<(), BatchError> {
        self.release_due(now, now, id, estimate)
    }

    /// Charge the incumbent through actual service time, but age waiting work
    /// from its declared release, including time the timer IRQ was masked.
    pub(crate) fn release_due(
        &mut self,
        now: u64,
        due: u64,
        id: usize,
        estimate: u64,
    ) -> Result<(), BatchError> {
        if due > now {
            return Err(BatchError::TimeRegression);
        }
        let slot = self.jobs.get(id).ok_or(BatchError::InvalidId)?;
        if estimate == 0 {
            return Err(BatchError::ZeroEstimate);
        }
        if slot.is_some() {
            return Err(BatchError::Duplicate);
        }
        self.account(now)?;
        // A vacant job ID always has room in the N-entry queue.
        assert!(self.ready.push_back(id));
        self.jobs[id] = Some(DispatchJob {
            release: due,
            service: estimate,
            remaining: estimate,
            ..DispatchJob::fixed_priority(0)
        });
        Ok(())
    }

    pub(crate) fn complete(&mut self, now: u64, id: usize) -> Result<(), BatchError> {
        if id >= N {
            return Err(BatchError::InvalidId);
        }
        if self.running != Some(id) {
            return Err(BatchError::NotRunning);
        }
        self.account(now)?;
        self.jobs[id] = None;
        self.running = None;
        Ok(())
    }

    /// Equal ranks preserve waiting order. For SRTF the incumbent ranks first
    /// among ties and, if preempted, keeps its turn ahead of waiting peers.
    pub(crate) fn decide(&mut self, now: u64) -> Result<Option<usize>, BatchError> {
        self.account(now)?;
        if let Some(id) = self.running {
            if self.policy != BatchPolicy::Srtf {
                return Ok(Some(id));
            }
            assert!(self.ready.push_front(id));
        }
        self.running = self.policy.ranking().pop_ready(&mut self.ready, now, |id| self.jobs[id]);
        Ok(self.running)
    }
}
