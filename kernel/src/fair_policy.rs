//! Single-CPU decisions over a caller-ordered snapshot of runnable requests.
//! One service tick advances virtual runtime by 1 / weight. The caller owns
//! wakeup placement, lag preservation on migration/reweighting, the current
//! task, request renewal and the timing of preemption.

use core::cmp::Ordering;

/// An exact nonnegative virtual timestamp in a common epoch. Retaining the
/// fraction makes frequent preemptions safe even for large task weights.
#[derive(Clone, Copy)]
pub(crate) struct VirtualTime {
    whole: u64,
    remainder: u64,
    denominator: u64,
}

impl VirtualTime {
    pub(crate) fn new(whole: u64, remainder: u64, denominator: u64) -> Option<Self> {
        (remainder < denominator).then_some(Self {
            whole,
            remainder,
            denominator,
        })
    }

    fn compare(self, other: Self) -> Ordering {
        self.whole.cmp(&other.whole).then_with(|| {
            (u128::from(self.remainder) * u128::from(other.denominator))
                .cmp(&(u128::from(other.remainder) * u128::from(self.denominator)))
        })
    }

    /// Task timestamps use their scheduling weight as denominator.
    fn after_service(self, ticks: u64) -> Option<Self> {
        let ticks = u128::from(self.remainder) + u128::from(ticks);
        let weight = u128::from(self.denominator);
        Some(Self {
            whole: self
                .whole
                .checked_add(u64::try_from(ticks / weight).ok()?)?,
            remainder: (ticks % weight) as u64,
            denominator: self.denominator,
        })
    }
}

/// A live request at a fixed weight. The deadline stays anchored to the
/// request's original start across partial service.
#[derive(Clone, Copy)]
pub(crate) struct FairRequest {
    runtime: VirtualTime,
    deadline: VirtualTime,
}

impl FairRequest {
    /// The caller supplies the placed virtual start as `whole + remainder / weight`.
    /// Both `request` and `served` are physical CPU ticks. Only
    /// outstanding requests with representable timestamps enter the snapshot.
    pub(crate) fn new(
        weight: u32,
        whole: u64,
        remainder: u32,
        request: u64,
        served: u64,
    ) -> Option<Self> {
        if served >= request {
            return None;
        }
        let start = VirtualTime::new(whole, u64::from(remainder), u64::from(weight))?;
        Some(Self {
            runtime: start.after_service(served)?,
            deadline: start.after_service(request)?,
        })
    }
}

/// First queue position having minimum accumulated weighted virtual runtime.
pub(crate) fn pick_weighted(ready: &[FairRequest]) -> Option<usize> {
    ready
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.runtime.compare(b.runtime))
        .map(|(position, _)| position)
}

/// Computes V = sum(weight * runtime) / sum(weight) exactly. Include the
/// running entity if it participates in the caller's fair clock. The total
/// weight must fit u64. Under that bound the weighted sum fits u128 because
/// each timestamp is strictly less than 2^64.
pub(crate) fn weighted_reference(runnable: &[FairRequest]) -> Option<VirtualTime> {
    let mut total_weight = 0_u64;
    let mut total_service = 0_u128;
    for request in runnable {
        let time = request.runtime;
        total_weight = total_weight.checked_add(time.denominator)?;
        total_service +=
            u128::from(time.whole) * u128::from(time.denominator) + u128::from(time.remainder);
    }
    if total_weight == 0 {
        return None;
    }
    let weight = u128::from(total_weight);
    VirtualTime::new(
        (total_service / weight) as u64,
        (total_service % weight) as u64,
        total_weight,
    )
}

/// A task has nonnegative lag iff its current virtual runtime is <= V.
/// Among those eligible, dispatch the earliest fixed virtual deadline, with
/// snapshot order breaking ties. An external V may leave the whole set idle.
pub(crate) fn pick_eevdf(ready: &[FairRequest], reference: VirtualTime) -> Option<usize> {
    ready
        .iter()
        .enumerate()
        .filter(|(_, request)| request.runtime.compare(reference) != Ordering::Greater)
        .min_by(|(_, a), (_, b)| a.deadline.compare(b.deadline))
        .map(|(position, _)| position)
}

pub(crate) fn bench_checks() -> bool {
    use core::hint::black_box;

    // The first task has the earliest deadline (2), but negative lag at
    // both V = 3/7 and V = 1/2. At the latter reference the short request
    // (deadline 3) precedes the minimum-runtime task (deadline 8).
    let [ahead, long, short] = black_box([(1, 1, 0, 1, 0), (2, 0, 0, 16, 0), (4, 0, 0, 12, 2)])
        .map(|(w, v, r, q, s)| FairRequest::new(w, v, r, q, s));
    let (Some(ahead), Some(long), Some(short)) = (ahead, long, short) else {
        return false;
    };
    let ready = [ahead, long, short];
    let Some(reference) = weighted_reference(&ready) else {
        return false;
    };
    // V = 3/7 lies below the short task's 1/2. At an external V = 1/2
    // it becomes eligible exactly at zero lag.
    let Some(half) = VirtualTime::new(0, 1, 2) else {
        return false;
    };
    let Some(expected) = VirtualTime::new(0, 3, 7) else {
        return false;
    };
    if reference.compare(expected) != Ordering::Equal
        || pick_weighted(&ready) != Some(1)
        || pick_eevdf(&ready, reference) != Some(1)
        || pick_eevdf(&ready, half) != Some(2)
        || pick_eevdf(&[ahead], half).is_some()
    {
        return false;
    }

    // Partial service advances runtime but preserves the original deadline.
    // Both requests end at 2 + 1/2, and both current runtimes are 1 + 1/2.
    let [a, b] = black_box([(2, 0, 1, 4, 2), (4, 1, 2, 4, 0)])
        .map(|(w, v, r, q, s)| FairRequest::new(w, v, r, q, s));
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let pair = [a, b];
    let Some(reference) = weighted_reference(&pair) else {
        return false;
    };
    if pick_weighted(&pair) != Some(0)
        || pick_eevdf(&pair, reference) != Some(0)
        || pick_eevdf(&[b, a], reference) != Some(0)
    {
        return false;
    }

    // Both tasks start at V = 0. The second deadline (2/5) precedes the
    // first (1/2), even though both deadlines have the same integer part.
    let [a, b] = black_box([(2, 1), (5, 2)]).map(|(w, q)| FairRequest::new(w, 0, 0, q, 0));
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let pair = [a, b];
    let Some(reference) = weighted_reference(&pair) else {
        return false;
    };
    if pick_eevdf(&pair, reference) != Some(1) {
        return false;
    }

    // Give each selection one physical tick; over whole cycles the minimum
    // normalized service rule delivers the configured 1:3 shares.
    let weights = black_box([1, 3]);
    let mut served = [0, 0];
    for _ in 0..40 {
        let [Some(a), Some(b)] = [
            FairRequest::new(weights[0], 0, 0, 40, served[0]),
            FairRequest::new(weights[1], 0, 0, 40, served[1]),
        ] else {
            return false;
        };
        let Some(winner) = pick_weighted(&[a, b]) else {
            return false;
        };
        served[winner] += 1;
    }
    if served != [10, 30] {
        return false;
    }

    // Wide products, a fractional carry near the timestamp horizon and
    // a request whose virtual duration is below one tick.
    let weight = black_box(u32::MAX);
    let whole = black_box(u64::MAX - 1);
    let [later, earlier] = [
        FairRequest::new(weight, whole, weight - 1, 2, 1),
        FairRequest::new(weight, whole, weight - 1, 1, 0),
    ];
    let (Some(later), Some(earlier)) = (later, earlier) else {
        return false;
    };
    let pair = [later, earlier];
    let Some(reference) = weighted_reference(&pair) else {
        return false;
    };
    pick_weighted(&pair) == Some(1)
        && pick_eevdf(&pair, reference) == Some(1)
        && pick_eevdf(&[], reference).is_none()
        && pick_weighted(&[]).is_none()
        && weighted_reference(&[]).is_none()
        && VirtualTime::new(0, 0, 0).is_none()
        && FairRequest::new(0, 0, 0, 1, 0).is_none()
        && FairRequest::new(2, 0, 2, 1, 0).is_none()
        && FairRequest::new(1, 0, 0, 0, 0).is_none()
        && FairRequest::new(1, 0, 0, 2, 2).is_none()
        && FairRequest::new(1, u64::MAX, 0, 1, 0).is_none()
}

#[cfg(test)]
mod tests {
    #[test]
    fn weighted_service_and_eligible_deadlines() {
        assert!(super::bench_checks());
    }
}
