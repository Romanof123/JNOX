//! Linked ordinary entries and the finite admission model.
use super::deadline_policy::{DeadlinePolicy, DeadlineScheduler};
use core::sync::atomic::{AtomicU64, Ordering};

pub const N: usize = 3;
pub const HORIZON: u64 = 16;
pub static PRODUCT: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
pub static PROGRESS: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];

#[derive(Clone, Copy)]
pub struct Job {
    pub id: usize,
    pub entry: Option<extern "C" fn(u64)>,
    pub release: u64,
    pub deadline: u64,
    pub cost: u64,
    pub milliseconds: u64,
}

// A finite stream of independent arithmetic blocks with a wall-clock stop.
// The job never reads scheduler state or asks to yield.
#[inline(always)]
fn work(id: usize, duration: u64) {
    let end = super::ordinary_deadline::counter()
        .checked_add(duration)
        .unwrap();
    work_until(id, end);
}

#[inline(always)]
fn work_until(id: usize, end: u64) {
    let mut value = id as u64 + 1;
    while super::ordinary_deadline::counter() < end {
        for _ in 0..64 {
            value = core::hint::black_box(value.wrapping_mul(33).wrapping_add(1));
        }
        PROGRESS[id].fetch_add(1, Ordering::Relaxed);
    }
    if cfg!(any(
        jnox_app = "ordinary-edf-supply",
        jnox_app = "ordinary-llf-supply"
    )) {
        PRODUCT[id].store(value, Ordering::Relaxed);
    }
    core::hint::black_box(value);
}
#[unsafe(no_mangle)]
pub extern "C" fn deadline_worker_a(duration: u64) {
    work(0, duration);
}
#[unsafe(no_mangle)]
pub extern "C" fn deadline_worker_b(duration: u64) {
    work(1, duration);
}
#[unsafe(no_mangle)]
pub extern "C" fn deadline_worker_c(duration: u64) {
    work(2, duration);
}

const fn job(id: usize, release: u64, deadline: u64, cost: u64, milliseconds: u64) -> Job {
    let entries: [extern "C" fn(u64); N] =
        [deadline_worker_a, deadline_worker_b, deadline_worker_c];
    Job {
        id,
        entry: Some(entries[id]),
        release,
        deadline,
        cost,
        milliseconds,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn deadline_worker_a_until(stop: u64) {
    work_until(0, stop);
}

pub static SUPPLIED: [Job; 2] = [
    Job {
        entry: Some(deadline_worker_a_until),
        ..job(0, 0, 5, 2, 250)
    },
    job(1, 1, 7, 2, 30),
];
pub static TIGHT: [Job; 1] = [job(0, 0, 2, 2, 250)];
pub const RESERVED: u64 = 1 << 1;

pub static EDF: [Job; N] = [
    job(
        0,
        0,
        8,
        2,
        if cfg!(jnox_app = "ordinary-edf-overrun") {
            370
        } else {
            175
        },
    ),
    job(1, 0, 12, 8, 100),
    job(2, 1, 4, 1, 20),
];
pub static LLF: [Job; N] = [
    job(0, 0, 8, 1, 25),
    job(1, 0, 10, 8, 850),
    job(2, 7, 8, 3, 25),
];

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

/// Maximum exception entries (and hence returning owner changes) in every
/// integer-counter window of this width, assuming one grid IRQ in its own
/// cell and at most one asynchronous completion per admitted one-shot job.
pub fn owner_window_bound(
    q: u64,
    cells: u64,
    width: u64,
    completions: u64,
) -> Result<u64, Refusal> {
    let domain = q.checked_mul(cells).ok_or(Refusal::Overflow)?;
    if width == 0 || width > domain {
        return Err(Refusal::Grid);
    }
    let touched = (width - 1).div_ceil(q).checked_add(1).ok_or(Refusal::Overflow)?;
    touched.min(cells).checked_add(completions).ok_or(Refusal::Overflow)
}

pub fn admit(policy: DeadlinePolicy, jobs: &[Job], hz: u64, origin: u64) -> Result<(), Refusal> {
    admit_with_supply(policy, jobs, hz, origin, 0)
}

pub fn available(mask: u64, start: u64, end: u64) -> Result<u64, Refusal> {
    if mask >> HORIZON != 0 || start > end || end > HORIZON {
        return Err(Refusal::Grid);
    }
    Ok((start..end).filter(|k| mask & (1 << k) == 0).count() as u64)
}

pub fn admit_with_supply(
    policy: DeadlinePolicy,
    jobs: &[Job],
    hz: u64,
    origin: u64,
    mask: u64,
) -> Result<(), Refusal> {
    available(mask, 0, HORIZON)?;
    if jobs.is_empty() || jobs.len() > N {
        return Err(Refusal::Table);
    }
    if hz == 0 || hz % 10 != 0 {
        return Err(Refusal::Grid);
    }
    absolute(origin, hz / 10, HORIZON + 1)?;
    let mut seen = [false; N];
    for j in jobs {
        if j.id >= N {
            return Err(Refusal::Table);
        }
        if core::mem::replace(&mut seen[j.id], true) {
            return Err(Refusal::Duplicate);
        }
        if j.entry.is_none() {
            return Err(Refusal::Entry);
        }
        if j.cost == 0 {
            return Err(Refusal::Cost);
        }
        absolute(origin, hz / 10, j.deadline)?;
        if j.deadline <= j.release || j.deadline > HORIZON {
            return Err(Refusal::Deadline);
        }
        let duration = hz.checked_mul(j.milliseconds).ok_or(Refusal::Overflow)?;
        if duration == 0 || duration % 1000 != 0 {
            return Err(Refusal::Grid);
        }
        absolute(absolute(origin, hz / 10, HORIZON)?, 1, duration / 1000)?;
    }
    if policy == DeadlinePolicy::Edf {
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
    }
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
