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
    SwitchCap,
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
    admit_native_supply(policy, jobs, hz, origin, 0)
}

/// One changing IRQ and at most one service receipt per intersected cell,
/// plus at most one exit for each of the two ordinary jobs.
pub(crate) const RECURRING_SWITCH_BOUND: u64 = 2 * (1 + 1) + 2;

pub(crate) fn admit_native_recurring(
    policy: DeadlinePolicy, jobs: &[NumericJob], hz: u64, origin: u64, mask: u64, cap: u64,
) -> Result<(), Refusal> {
    if jobs.len() != 2 { return Err(Refusal::Table); }
    if mask == 0 || mask >> HORIZON != 0 { return Err(Refusal::Grid); }
    if cap < RECURRING_SWITCH_BOUND { return Err(Refusal::SwitchCap); }
    admit_native_supply(policy, jobs, hz, origin, mask)
}

/// A sample at a cell's end belongs to the next cell. The final end is outside.
pub(crate) fn recurring_cell(origin: u64, q: u64, sample: u64) -> Option<u64> {
    sample.checked_sub(origin)?.checked_div(q).filter(|&k| k < HORIZON)
}

/// Counting strictly earlier reserved cells also covers adjacent activations.
pub(crate) fn recurring_ordinal(mask: u64, cell: u64) -> Option<u64> {
    if cell >= HORIZON || mask >> HORIZON != 0 { return None; }
    let bit = 1u64 << cell;
    (mask & bit != 0).then(|| u64::from((mask & (bit - 1)).count_ones()))
}

fn admit_native_supply(
    policy: DeadlinePolicy, jobs: &[NumericJob], hz: u64, origin: u64, mask: u64,
) -> Result<(), Refusal> {
    let q = validate_table(jobs.len(), hz, mask)?;
    absolute(origin, q, HORIZON)?;
    let mut seen = [false; N];
    for job in jobs {
        validate_id(job.id, &mut seen)?;
        job.validate(origin, q)?;
    }
    if jobs.iter().enumerate().any(|(slot, job)| job.id != slot) {
        return Err(Refusal::Table);
    }
    check_demand(jobs, mask)?;
    simulate_dispatch(policy, jobs, mask)
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

#[cfg(test)]
mod recurring_tests {
    use super::*;

    fn pair() -> [NumericJob; 2] {
        [NumericJob { id: 0, release: 0, deadline: 12, cost: 1 },
         NumericJob { id: 1, release: 0, deadline: 13, cost: 4 }]
    }

    #[test]
    fn cap_and_interval_supply_are_independent_for_both_policies() {
        for policy in [DeadlinePolicy::Edf, DeadlinePolicy::Llf] {
            let jobs = pair();
            let admit = |jobs: &[NumericJob], mask, cap|
                admit_native_recurring(policy, jobs, 100, 50, mask, cap);
            assert_eq!(admit(&jobs, 0b1010, 6), Ok(()));
            assert_eq!(admit(&jobs, 0b1010, 5), Err(Refusal::SwitchCap));
            assert_eq!(admit(&jobs[..1], 0b1010, 6), Err(Refusal::Table));
            assert_eq!(admit(&jobs, 0, 6), Err(Refusal::Grid));
            assert_eq!(admit(&jobs, 1 << HORIZON, 6), Err(Refusal::Grid));
            let mut constrained = jobs;
            constrained[0].cost = 2;
            constrained[0].deadline = 2;
            assert_eq!(admit(&constrained, 0b1010, 6), Err(Refusal::Demand));
            // Enough supply globally, but two jobs compete for one available
            // cell strictly inside the horizon.
            for j in &mut constrained { j.release = 6; j.deadline = 8; j.cost = 1; }
            assert_eq!(admit(&constrained, 1 << 7, 6), Err(Refusal::Demand));
            assert_eq!(admit(&constrained, 1 << 8, 6), Ok(()));
            assert_eq!(admit_native_recurring(policy, &jobs, 100, u64::MAX - 159, 1, 6),
                       Err(Refusal::Overflow));
        }
    }

    #[test]
    fn first_last_and_adjacent_cells_use_exclusive_ends_and_distinct_ordinals() {
        let mask = 1 | (1 << 1) | (1 << 14) | (1 << 15);
        let jobs = pair();
        for policy in [DeadlinePolicy::Edf, DeadlinePolicy::Llf] {
            assert_eq!(admit_native_recurring(policy, &jobs, 100, 50, mask, 6), Ok(()));
        }
        assert_eq!(recurring_cell(50, 10, 49), None);
        assert_eq!(recurring_cell(50, 0, 50), None);
        for k in 0..HORIZON {
            let first = 50 + 10 * k;
            assert_eq!(recurring_cell(50, 10, first), Some(k));
            assert_eq!(recurring_cell(50, 10, first + 9), Some(k));
            let ordinal = [0, 1, 14, 15].iter().position(|&reserved| reserved == k);
            assert_eq!(recurring_ordinal(mask, k), ordinal.map(|i| i as u64));
        }
        assert_eq!(recurring_cell(50, 10, 210), None);
        assert_eq!(recurring_ordinal(mask, HORIZON), None);
    }
}
