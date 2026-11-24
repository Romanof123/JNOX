//! Bounded, allocation-free placement over a modeled snapshot. Capacities,
//! competing load and protected capacity are work units per tick. Wake and
//! deadline margins are ticks; energy per work and wake energy use the same
//! modeled energy unit. Board measurements must supply calibrated inputs.
use super::group_policy::{EnergyRequest, OperatingPoint, energy_placement};

#[derive(Clone, Copy)]
pub(crate) struct CpuLimits {
    pub(crate) available: bool,
    pub(crate) thermal_capacity: u64,
    pub(crate) shared_load: u64,
    pub(crate) reserve_capacity: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) cpu: u8,
    pub(crate) opp: u16,
    pub(crate) capacity: u64,
    pub(crate) run_ticks: u64,
    pub(crate) wake_ticks: u64,
    /// Slack to the original deadline, including the protected deadline reserve.
    pub(crate) margin: u64,
    pub(crate) energy: u128,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Model(Placement),
    Fallback(Placement),
    Idle,
    DeadlineMiss,
}

fn adjusted(point: OperatingPoint, limits: &[CpuLimits], request: EnergyRequest) -> OperatingPoint {
    let cpu_bit = 1u64.checked_shl(u32::from(point.cpu)).unwrap_or(0);
    let capacity = limits
        .get(usize::from(point.cpu))
        .filter(|limit| limit.available && request.allowed_cpus & cpu_bit != 0)
        .map_or(0, |limit| {
            point
                .capacity
                .min(limit.thermal_capacity)
                .saturating_sub(limit.shared_load)
                .saturating_sub(limit.reserve_capacity)
        });
    OperatingPoint { capacity, ..point }
}

fn qualifies(point: &OperatingPoint, minimum: u64) -> bool {
    point.capacity > 0 && point.capacity >= minimum
}

/// The shared energy selector already ranks by energy, CPU, then OPP and
/// checks wake delay and deadline in widened arithmetic. Report its witness.
fn select<const N: usize>(
    request: EnergyRequest,
    reserve: u64,
    points: &[OperatingPoint; N],
) -> Option<Placement> {
    let protected = EnergyRequest {
        deadline: request.deadline.checked_sub(reserve)?,
        ..request
    };
    let (cpu, opp) = energy_placement(protected, points)?;
    let p = points.iter().find(|p| p.cpu == cpu && p.opp == opp)?;
    let run_ticks = request.work.div_ceil(p.capacity);
    // energy_placement proved finish <= deadline <= u64::MAX.
    let finish = request.now + p.wake_ticks + run_ticks;
    Some(Placement {
        cpu,
        opp,
        capacity: p.capacity,
        run_ticks,
        wake_ticks: p.wake_ticks,
        margin: request.deadline - finish,
        energy: u128::from(request.work) * u128::from(p.energy_per_work)
            + u128::from(p.wake_energy),
    })
}

/// Zero work or no permissible capacity yields Idle. If any eligible route
/// has sufficient capacity but misses the protected finish, report DeadlineMiss.
/// Ambiguous (duplicate CPU/OPP) catalogues are unavailable; the independently
/// supplied trusted fallback must pass the same mask, capacity and time checks.
pub(crate) fn place<const N: usize, const C: usize>(
    request: EnergyRequest,
    deadline_reserve: u64,
    points: &[OperatingPoint; N],
    limits: &[CpuLimits; C],
    fallback: Option<OperatingPoint>,
) -> Decision {
    if request.work == 0 {
        return Decision::Idle;
    }
    let mut eligible = false;
    let unique = points
        .iter()
        .enumerate()
        .all(|(i, p)| !points[..i].iter().any(|q| (q.cpu, q.opp) == (p.cpu, p.opp)));
    if unique {
        let candidates = points.map(|p| adjusted(p, limits, request));
        if let Some(placement) = select(request, deadline_reserve, &candidates) {
            return Decision::Model(placement);
        }
        eligible = candidates
            .iter()
            .any(|p| qualifies(p, request.minimum_capacity));
    }
    if let Some(point) = fallback {
        let candidate = adjusted(point, limits, request);
        if let Some(placement) = select(request, deadline_reserve, &[candidate]) {
            return Decision::Fallback(placement);
        }
        eligible |= qualifies(&candidate, request.minimum_capacity);
    }
    if eligible {
        Decision::DeadlineMiss
    } else {
        Decision::Idle
    }
}
