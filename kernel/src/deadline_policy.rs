//! Single-CPU deadline dispatch with a fixed preemption grid. Time skipped
//! between observations belongs to the actual incumbent throughout.

use super::ready_queue::ReadyQueue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeadlinePolicy {
    Edf,
    Llf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeadlineError {
    TimeRegression,
    InvalidId,
    ZeroEstimate,
    Duplicate,
    NotRunning,
}

#[derive(Clone, Copy)]
struct Job {
    deadline: u64,
    remaining: u64,
}

impl Job {
    fn laxity(self, now: u64) -> i128 {
        i128::from(self.deadline) - i128::from(now) - i128::from(self.remaining)
    }
}

pub(crate) struct DeadlineScheduler<const N: usize> {
    policy: DeadlinePolicy,
    start: u64,
    quantum: u64,
    now: u64,
    last_boundary: Option<u64>,
    running: Option<usize>,
    jobs: [Option<Job>; N],
    ready: ReadyQueue<N>,
}

impl<const N: usize> DeadlineScheduler<N> {
    pub(crate) const fn new(policy: DeadlinePolicy, start: u64, quantum: u64) -> Option<Self> {
        if quantum == 0 {
            return None;
        }
        Some(Self {
            policy,
            start,
            quantum,
            now: start,
            last_boundary: None,
            running: None,
            jobs: [None; N],
            ready: ReadyQueue::new(),
        })
    }

    pub(crate) fn now(&self) -> u64 {
        self.now
    }

    pub(crate) fn remaining(&self, id: usize) -> Option<u64> {
        self.jobs.get(id)?.map(|job| job.remaining)
    }

    pub(crate) fn laxity(&self, id: usize) -> Option<i128> {
        self.jobs.get(id)?.map(|job| job.laxity(self.now))
    }

    #[cfg(jnox_ordinary_deadline)]
    pub(crate) fn ready(&self) -> &[usize] { self.ready.entries() }

    pub(crate) fn account(&mut self, now: u64) -> Result<(), DeadlineError> {
        let elapsed = now
            .checked_sub(self.now)
            .ok_or(DeadlineError::TimeRegression)?;
        if let Some(id) = self.running {
            let job = self.jobs[id].as_mut().expect("running job");
            job.remaining = job.remaining.saturating_sub(elapsed).max(1);
        }
        self.now = now;
        Ok(())
    }

    pub(crate) fn withhold(&mut self, now: u64) -> Result<(), DeadlineError> {
        self.account(now)?;
        if let Some(id) = self.running.take() {
            assert!(self.ready.push_front(id));
        }
        Ok(())
    }

    /// Queue a release without consuming the decision at this timestamp.
    pub(crate) fn release(
        &mut self,
        now: u64,
        id: usize,
        estimate: u64,
        deadline: u64,
    ) -> Result<(), DeadlineError> {
        let slot = self.jobs.get(id).ok_or(DeadlineError::InvalidId)?;
        if estimate == 0 {
            return Err(DeadlineError::ZeroEstimate);
        }
        if slot.is_some() {
            return Err(DeadlineError::Duplicate);
        }
        self.account(now)?;
        assert!(self.ready.push_back(id));
        self.jobs[id] = Some(Job {
            deadline,
            remaining: estimate,
        });
        Ok(())
    }

    pub(crate) fn complete(&mut self, now: u64, id: usize) -> Result<(), DeadlineError> {
        if id >= N {
            return Err(DeadlineError::InvalidId);
        }
        if self.running != Some(id) {
            return Err(DeadlineError::NotRunning);
        }
        self.account(now)?;
        self.jobs[id] = None;
        self.running = None;
        Ok(())
    }

    /// Idle CPUs can select at any time. An incumbent can be displaced only
    /// on the first decision at a grid boundary. Equal ranks preserve the
    /// incumbent first, then waiting order.
    pub(crate) fn decide(&mut self, now: u64) -> Result<Option<usize>, DeadlineError> {
        self.account(now)?;
        let boundary = (now - self.start) % self.quantum == 0;
        if let Some(id) = self.running {
            if !boundary || self.last_boundary == Some(now) {
                return Ok(Some(id));
            }
            #[cfg(jnox_ordinary_deadline)]
            {
                let rank = |job: Job| match self.policy {
                    DeadlinePolicy::Edf => i128::from(job.deadline),
                    DeadlinePolicy::Llf => job.laxity(now),
                };
                let incumbent = rank(self.jobs[id].unwrap());
                let selected = self.ready.pop_ranked(
                    |candidate| self.jobs[candidate].map(rank).filter(|key| *key < incumbent),
                    |a, b| a < b,
                );
                if let Some(next) = selected {
                    assert!(self.ready.push_back(id));
                    self.running = Some(next);
                }
                self.last_boundary = Some(now);
                return Ok(self.running);
            }
            #[cfg(not(jnox_ordinary_deadline))]
            assert!(self.ready.push_front(id));
        }
        self.running = self.ready.pop_ranked(
            |id| {
                self.jobs[id].map(|job| match self.policy {
                    DeadlinePolicy::Edf => i128::from(job.deadline),
                    DeadlinePolicy::Llf => job.laxity(now),
                })
            },
            |candidate, incumbent| candidate < incumbent,
        );
        if boundary && self.running.is_some() {
            self.last_boundary = Some(now);
        }
        Ok(self.running)
    }
}
