//! Linked ordinary entries and the finite admission model.
use super::deadline_policy::{DeadlinePolicy, numeric::{self, NumericJob}};
pub use super::deadline_policy::numeric::{absolute, available, Refusal, HORIZON, N};
use core::sync::atomic::{AtomicU64, Ordering};

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
        jnox_deadline_recurring,
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

#[cfg(any(test, jnox_deadline_recurring))]
pub static RECURRING: [Job; 2] = [
    Job { entry: Some(deadline_worker_a_until), ..job(0, 0, 13, 9, 1050) },
    job(1, 10, 16, 1, 30),
];
#[cfg(any(test, jnox_deadline_recurring))]
pub static RECURRING_TIGHT: [Job; 1] = [job(0, 0, 10, 9, 1050)];

/// Compile the finite calendar directly from cell membership. Subtraction
/// only follows the offset check; the final occurrence is clipped at H.
#[cfg(any(test, jnox_deadline_recurring))]
pub fn recurring_mask(offset: u64, period: u64, width: u64) -> Result<u64, Refusal> {
    if offset >= HORIZON || period == 0 || width == 0 || width > period {
        return Err(Refusal::Grid);
    }
    let mut mask = 0;
    for k in offset..HORIZON {
        if (k - offset) % period < width {
            mask |= 1 << k;
        }
    }
    Ok(mask)
}

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

pub fn admit_with_supply(
    policy: DeadlinePolicy,
    jobs: &[Job],
    hz: u64,
    origin: u64,
    mask: u64,
) -> Result<(), Refusal> {
    let q = numeric::validate_table(jobs.len(), hz, mask)?;
    absolute(origin, q, HORIZON + 1)?;
    let mut seen = [false; N];
    let mut numbers = [NumericJob::EMPTY; N];
    for (index, j) in jobs.iter().enumerate() {
        numeric::validate_id(j.id, &mut seen)?;
        if j.entry.is_none() {
            return Err(Refusal::Entry);
        }
        let descriptor = NumericJob { id: j.id, release: j.release, deadline: j.deadline, cost: j.cost };
        descriptor.validate(origin, q)?;
        let duration = hz.checked_mul(j.milliseconds).ok_or(Refusal::Overflow)?;
        if duration == 0 || duration % 1000 != 0 {
            return Err(Refusal::Grid);
        }
        absolute(absolute(origin, q, HORIZON)?, 1, duration / 1000)?;
        numbers[index] = descriptor;
    }
    numeric::simulate(policy, &numbers[..jobs.len()], mask)
}
