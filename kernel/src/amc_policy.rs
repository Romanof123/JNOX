//! Fixed-priority mixed-criticality dispatch for one CPU. One grant is one
//! virtual service slot. The caller retires it after saving the timer frame.

use super::domain_policy::{CriticalityDomain, CriticalityMode, ExecutionBudget};

#[derive(Clone, Copy)]
pub(crate) struct TaskSpec {
    pub(crate) budget: ExecutionBudget,
    pub(crate) priority: u8,
    pub(crate) relative_deadline: u64,
}

#[derive(Clone, Copy)]
struct Job {
    demand: u64,
    remaining: u64,
    deadline: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Dispatch {
    Idle,
    Run { task: usize, discarded: u32 },
    Exhausted { task: usize },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SlotEnd {
    pub(crate) task: usize,
    pub(crate) end: u64,
    pub(crate) used: u64,
    pub(crate) remaining: u64,
    pub(crate) deadline: u64,
}

impl SlotEnd {
    pub(crate) fn deadline_missed(&self) -> bool {
        self.end > self.deadline || (self.end == self.deadline && self.remaining > 0)
    }
}

pub(crate) struct AmcRuntime<const N: usize> {
    domain: CriticalityDomain<N>,
    specs: [TaskSpec; N],
    jobs: [Option<Job>; N],
    running: Option<usize>,
    now: u64,
}

impl<const N: usize> AmcRuntime<N> {
    pub(crate) fn new(specs: [TaskSpec; N]) -> Option<Self> {
        if specs.iter().any(|s| s.relative_deadline == 0) {
            return None;
        }
        Some(Self {
            domain: CriticalityDomain::new(specs.map(|s| s.budget))?,
            specs,
            jobs: [None; N],
            running: None,
            now: 0,
        })
    }

    pub(crate) fn now(&self) -> u64 {
        self.now
    }

    pub(crate) fn mode(&self) -> CriticalityMode {
        self.domain.mode()
    }

    pub(crate) fn remaining(&self, id: usize) -> Option<u64> {
        Some(self.jobs.get(id)?.as_ref()?.remaining)
    }

    /// A release uses the current virtual boundary. A pending slot retains its
    /// owner until the next timer. Demand may exceed budget to expose faults.
    pub(crate) fn release(&mut self, id: usize, demand: u64) -> bool {
        let Some(spec) = self.specs.get(id) else {
            return false;
        };
        let Some(deadline) = self.now.checked_add(spec.relative_deadline) else {
            return false;
        };
        if demand == 0 || !self.domain.release(id) {
            return false;
        }
        self.jobs[id] = Some(Job {
            demand,
            remaining: demand,
            deadline,
        });
        true
    }

    /// Highest priority wins, with lower task ID first on ties. Only a
    /// positive grant past a HI job's LO allowance triggers the transition.
    /// A denied grant retires its faulting job without advancing time.
    pub(crate) fn start_slot(&mut self) -> Option<Dispatch> {
        if self.running.is_some() {
            return None;
        }
        let next = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| job.is_some())
            .min_by_key(|(id, _)| (core::cmp::Reverse(self.specs[*id].priority), *id));
        let Some((task, _)) = next else {
            return Some(Dispatch::Idle);
        };
        self.now.checked_add(1)?;
        let previous = self.domain.mode();
        if self.domain.grant(task, 1)? == 0 {
            self.domain.finish(task);
            self.jobs[task] = None;
            return Some(Dispatch::Exhausted { task });
        }
        let mut discarded = 0;
        if self.domain.mode() != previous {
            for (id, (spec, job)) in self.specs.iter().zip(&mut self.jobs).enumerate() {
                if matches!(spec.budget, ExecutionBudget::Low(_)) && job.take().is_some() {
                    discarded |= 1 << id;
                }
            }
        }
        self.running = Some(task);
        Some(Dispatch::Run { task, discarded })
    }

    /// Retire exactly the outstanding grant. Completion on the LO boundary
    /// finishes the job in its current mode without requesting excess service.
    pub(crate) fn finish_slot(&mut self) -> Option<SlotEnd> {
        let task = self.running.take()?;
        let job = self.jobs[task].as_mut()?;
        job.remaining -= 1;
        self.now += 1;
        let record = SlotEnd {
            task,
            end: self.now,
            used: job.demand - job.remaining,
            remaining: job.remaining,
            deadline: job.deadline,
        };
        if job.remaining == 0 {
            self.domain.finish(task);
            self.jobs[task] = None;
        }
        Some(record)
    }

    pub(crate) fn reset(&mut self, approved: bool) -> bool {
        self.domain.reset(approved)
    }
}
