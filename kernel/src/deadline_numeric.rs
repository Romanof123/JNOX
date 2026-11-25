//! Finite one-shot job admission in logical grid units.
use super::{DeadlinePolicy, DeadlineScheduler};

pub const N: usize = 3;
pub const HORIZON: u64 = 16;

#[derive(Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Refusal {
    Table = 1,
    Duplicate,
    Entry,
    Cost,
    Deadline,
    Grid,
    Overflow,
    Demand,
    Miss,
}

pub fn absolute(origin: u64, q: u64, k: u64) -> Result<u64, Refusal> {
    origin
        .checked_add(q.checked_mul(k).ok_or(Refusal::Overflow)?)
        .ok_or(Refusal::Overflow)
}

pub fn available(mask: u64, start: u64, end: u64) -> Result<u64, Refusal> {
    if mask >> HORIZON != 0 || start > end || end > HORIZON {
        return Err(Refusal::Grid);
    }
    Ok((start..end).filter(|k| mask & (1 << k) == 0).count() as u64)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NumericJob {
    pub(crate) id: usize,
    pub(crate) release: u64,
    pub(crate) deadline: u64,
    pub(crate) cost: u64,
}

impl NumericJob {
    pub(crate) const EMPTY: Self = Self { id: 0, release: 0, deadline: 0, cost: 0 };

    pub(crate) fn validate(&self, origin: u64, q: u64) -> Result<(), Refusal> {
        if self.cost == 0 { return Err(Refusal::Cost); }
        absolute(origin, q, self.deadline)?;
        if self.deadline <= self.release || self.deadline > HORIZON {
            return Err(Refusal::Deadline);
        }
        Ok(())
    }
}

pub(crate) fn grid_quantum(hz: u64) -> Result<u64, Refusal> {
    if hz == 0 || hz % 10 != 0 { return Err(Refusal::Grid); }
    Ok(hz / 10)
}

pub(crate) fn validate_table(count: usize, hz: u64, mask: u64) -> Result<u64, Refusal> {
    available(mask, 0, HORIZON)?;
    if !(1..=N).contains(&count) { return Err(Refusal::Table); }
    grid_quantum(hz)
}

pub(crate) fn validate_id(id: usize, seen: &mut [bool; N]) -> Result<(), Refusal> {
    if id >= N { return Err(Refusal::Table); }
    if core::mem::replace(&mut seen[id], true) { return Err(Refusal::Duplicate); }
    Ok(())
}

pub(crate) fn admit_edf(jobs: &[NumericJob], hz: u64, origin: u64) -> Result<(), Refusal> {
    admit_native(DeadlinePolicy::Edf, jobs, hz, origin)
}

pub(crate) fn admit_native(policy: DeadlinePolicy, jobs: &[NumericJob], hz: u64, origin: u64) -> Result<(), Refusal> {
    let q = validate_table(jobs.len(), hz, 0)?;
    absolute(origin, q, HORIZON)?;
    let mut seen = [false; N];
    for job in jobs {
        validate_id(job.id, &mut seen)?;
        job.validate(origin, q)?;
    }
    if jobs.iter().enumerate().any(|(slot, job)| job.id != slot) {
        return Err(Refusal::Table);
    }
    check_demand(jobs, 0)?;
    simulate_dispatch(policy, jobs, 0)
}

/// Callers have validated IDs, costs, deadlines and the supply mask.
pub(crate) fn simulate(policy: DeadlinePolicy, jobs: &[NumericJob], mask: u64) -> Result<(), Refusal> {
    if policy == DeadlinePolicy::Edf { check_demand(jobs, mask)?; }
    simulate_dispatch(policy, jobs, mask)
}

fn check_demand(jobs: &[NumericJob], mask: u64) -> Result<(), Refusal> {
    for start in jobs {
        for end in jobs.iter().filter(|j| j.deadline > start.release) {
            let mut demand = 0u64;
            for j in jobs
                .iter()
                .filter(|j| j.release >= start.release && j.deadline <= end.deadline)
            {
                demand = demand.checked_add(j.cost).ok_or(Refusal::Overflow)?;
            }
            if demand > available(mask, start.release, end.deadline)? {
                return Err(Refusal::Demand);
            }
        }
    }
    Ok(())
}

fn simulate_dispatch(policy: DeadlinePolicy, jobs: &[NumericJob], mask: u64) -> Result<(), Refusal> {
    let mut model = DeadlineScheduler::<N>::new(policy, 0, 1).unwrap();
    let mut left = [0; N];
    for tick in 0..=HORIZON {
        if jobs.iter().any(|j| j.deadline == tick && left[j.id] != 0) {
            return Err(Refusal::Miss);
        }
        let withheld = mask & (1 << tick) != 0;
        if withheld {
            model.withhold(tick).unwrap();
        }
        for j in jobs.iter().filter(|j| j.release == tick) {
            model.release(tick, j.id, j.cost, j.deadline).unwrap();
            left[j.id] = j.cost;
        }
        if withheld {
            continue;
        }
        if let Some(id) = model.decide(tick).unwrap() {
            left[id] -= 1;
            if left[id] == 0 {
                model.complete(tick + 1, id).unwrap();
            }
        }
    }
    Ok(())
}
