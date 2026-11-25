//! One-shot native jobs. Physical charge endpoints are supplied by the CPU
//! adapter; ready age is measured independently on the absolute counter.
use super::{BatchError, BatchPolicy, DispatchJob, ReadyQueue};

#[derive(Clone, Copy)]
enum State {
    Ready(u64),
    Running,
    Sleeping(u64),
    Exited,
}

#[derive(Clone, Copy)]
struct Job {
    estimate: u64,
    service: u64,
    wait: u64,
    state: State,
}

impl Job {
    fn remaining(self) -> u64 {
        self.estimate.saturating_sub(self.service).max(1)
    }

    fn wait_at(self, now: u64) -> Option<u64> {
        match self.state {
            State::Ready(since) => self.wait.checked_add(now.checked_sub(since)?),
            _ => Some(self.wait),
        }
    }

    fn rank(self, policy: BatchPolicy, now: u64) -> Option<DispatchJob> {
        // The shared comparator takes an original service and a live remaining
        // estimate. Present its ranking coordinate directly: HRRN's virtual
        // release recreates exactly the eligible wait, excluding all sleeps.
        let size = if policy == BatchPolicy::Srtf { self.remaining() } else { self.estimate };
        Some(DispatchJob {
            service: size,
            remaining: size,
            release: now.checked_sub(self.wait_at(now)?)?,
            ..DispatchJob::fixed_priority(0)
        })
    }
}

pub(crate) struct NativeBatchScheduler<const N: usize> {
    policy: BatchPolicy,
    epoch: u64,
    now: u64,
    jobs: [Option<Job>; N],
    ready: ReadyQueue<N>,
    running: Option<usize>,
}

impl<const N: usize> NativeBatchScheduler<N> {
    pub(crate) const fn new(policy: BatchPolicy, start: u64) -> Self {
        Self { policy, epoch: start, now: start, jobs: [None; N], ready: ReadyQueue::new(), running: None }
    }

    pub(crate) fn running(&self) -> Option<usize> { self.running }
    pub(crate) fn ready(&self) -> &[usize] { self.ready.entries() }
    pub(crate) fn remaining(&self, id: usize) -> Option<u64> { Some(self.jobs.get(id)?.as_ref()?.remaining()) }
    pub(crate) fn service(&self, id: usize) -> Option<u64> { Some(self.jobs.get(id)?.as_ref()?.service) }
    pub(crate) fn eligible_wait(&self, now: u64, id: usize) -> Option<u64> {
        if now < self.now { return None }
        self.jobs.get(id)?.as_ref()?.wait_at(now)
    }

    pub(crate) fn release_due(&mut self, now: u64, due: u64, id: usize, estimate: u64) -> Result<(), BatchError> {
        if now < self.now || due < self.epoch || due > now { return Err(BatchError::TimeRegression) }
        let slot = self.jobs.get_mut(id).ok_or(BatchError::InvalidId)?;
        if estimate == 0 { return Err(BatchError::ZeroEstimate) }
        if slot.is_some() { return Err(BatchError::Duplicate) }
        *slot = Some(Job { estimate, service: 0, wait: 0, state: State::Ready(due) });
        assert!(self.ready.push_back(id));
        self.now = now;
        Ok(())
    }

    pub(crate) fn charge(&mut self, id: usize, start: u64, entry: u64) -> Result<(), BatchError> {
        if self.running != Some(id) { return Err(BatchError::NotRunning) }
        if start < self.now || entry < start { return Err(BatchError::TimeRegression) }
        let job = self.jobs[id].as_mut().expect("running job");
        // Positive service intervals do not overlap and all fit on one u64 timeline.
        job.service = job.service.checked_add(entry - start).ok_or(BatchError::TimeRegression)?;
        self.now = entry;
        Ok(())
    }

    fn stop_running(&mut self, now: u64, id: usize, state: State) -> Result<(), BatchError> {
        if now < self.now { return Err(BatchError::TimeRegression) }
        if self.running != Some(id) { return Err(BatchError::NotRunning) }
        self.jobs[id].as_mut().expect("running job").state = state;
        self.running = None;
        self.now = now;
        Ok(())
    }

    pub(crate) fn yield_running(&mut self, now: u64, id: usize) -> Result<(), BatchError> {
        self.stop_running(now, id, State::Ready(now))?;
        assert!(self.ready.push_back(id));
        Ok(())
    }

    pub(crate) fn suspend(&mut self, now: u64, id: usize) -> Result<(), BatchError> {
        self.stop_running(now, id, State::Sleeping(now))
    }

    pub(crate) fn complete(&mut self, now: u64, id: usize) -> Result<(), BatchError> {
        self.stop_running(now, id, State::Exited)
    }

    pub(crate) fn wake_due(&mut self, now: u64, due: u64, id: usize) -> Result<(), BatchError> {
        let job = self.jobs.get_mut(id).ok_or(BatchError::InvalidId)?.as_mut().ok_or(BatchError::NotRunning)?;
        let State::Sleeping(since) = job.state else { return Err(BatchError::NotRunning) };
        if now < self.now || due < since || due > now { return Err(BatchError::TimeRegression) }
        job.state = State::Ready(due);
        assert!(self.ready.push_back(id));
        self.now = now;
        Ok(())
    }

    pub(crate) fn decide(&mut self, now: u64) -> Result<Option<usize>, BatchError> {
        if now < self.now { return Err(BatchError::TimeRegression) }
        self.now = now;
        if let Some(id) = self.running {
            if self.policy != BatchPolicy::Srtf { return Ok(Some(id)) }
            self.jobs[id].as_mut().expect("running job").state = State::Ready(now);
            assert!(self.ready.push_front(id));
        }
        let jobs = &self.jobs;
        let policy = self.policy;
        self.running = policy.ranking().pop_ready(&mut self.ready, now, |id| jobs[id]?.rank(policy, now));
        if let Some(id) = self.running {
            let job = self.jobs[id].as_mut().expect("selected job");
            job.wait = job.wait_at(now).expect("disjoint ready intervals");
            job.state = State::Running;
        }
        Ok(self.running)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underestimation_and_preempted_equal_contenders_retain_their_turn() {
        let mut p = NativeBatchScheduler::<3>::new(BatchPolicy::Srtf, 100);
        p.release_due(100, 100, 0, 50).unwrap();
        assert_eq!(p.decide(100), Ok(Some(0)));
        p.charge(0, 104, 124).unwrap();
        p.release_due(125, 120, 1, 30).unwrap();
        p.release_due(125, 125, 2, 1).unwrap();
        assert_eq!(p.decide(125), Ok(Some(2)));
        p.charge(2, 128, 132).unwrap();
        assert_eq!(p.remaining(2), Some(1));
        assert_eq!(p.service(2), Some(4));
        p.complete(132, 2).unwrap();
        assert_eq!(p.decide(133), Ok(Some(0)));
        assert_eq!(p.service(0), Some(20));
        assert_eq!(p.eligible_wait(133, 0), Some(8));
        assert_eq!(p.eligible_wait(133, 1), Some(13));
    }

    #[test]
    fn sleeping_job_ages_from_declared_wake_without_charging_sleep() {
        let mut p = NativeBatchScheduler::<3>::new(BatchPolicy::Hrrn, 0);
        p.release_due(0, 0, 0, 5).unwrap();
        assert_eq!(p.decide(0), Ok(Some(0)));
        p.charge(0, 1, 11).unwrap();
        p.suspend(12, 0).unwrap();
        p.release_due(90, 89, 1, 5).unwrap();
        p.release_due(90, 90, 2, 1).unwrap();
        p.wake_due(90, 88, 0).unwrap();
        assert_eq!(p.eligible_wait(90, 0), Some(2));
        assert_eq!(p.service(0), Some(10));
        assert_eq!(p.decide(90), Ok(Some(0)));
        p.yield_running(90, 0).unwrap();
        assert_eq!(p.ready(), &[1, 2, 0]);
        assert_eq!(p.decide(90), Ok(Some(0)));
        assert_eq!(p.charge(0, 89, 91), Err(BatchError::TimeRegression));
        assert_eq!(p.service(0), Some(10));
    }
}
