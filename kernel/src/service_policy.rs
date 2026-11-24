//! Bounded, single-CPU service accounting. The caller supplies monotonic time
//! for reservations and equal-length service quanta for stride scheduling.

/// A fixed budget for each period aligned to the initial start time.
pub(crate) struct PeriodicReservation {
    budget: u64,
    period: u64,
    period_start: u64,
    remaining: u64,
    earliest_start: u64,
}

impl PeriodicReservation {
    pub(crate) fn new(start: u64, budget: u64, period: u64) -> Option<Self> {
        if budget == 0 || budget > period {
            return None;
        }
        Some(Self {
            budget,
            period,
            period_start: start,
            remaining: budget,
            earliest_start: start,
        })
    }

    /// Commits a contiguous interval beginning at `now`. A caller must stop
    /// at its granted end and request again, even at a replenishment boundary.
    /// Zero means that no service was granted. Stale or overlapping requests
    /// return None without changing state.
    pub(crate) fn grant_at(&mut self, now: u64, requested: u64) -> Option<u64> {
        if now < self.earliest_start {
            return None;
        }
        let elapsed = now - self.period_start;
        let offset = elapsed % self.period;
        if elapsed >= self.period {
            self.period_start = now - offset;
            self.remaining = self.budget;
        }
        let granted = requested
            .min(self.remaining)
            .min(self.period - offset)
            .min(u64::MAX - now);
        self.remaining -= granted;
        self.earliest_start = now + granted;
        Some(granted)
    }
}

// LCM(1..=16) gives every supported weight an exact integer stride.
const STRIDE_SCALE: u32 = 720_720;
const MAX_WEIGHT: u8 = 16;

#[derive(Clone, Copy)]
struct Client {
    stride: u32,
    pass: u32,
    runnable: bool,
}

pub(crate) struct StrideClass<const N: usize> {
    clients: [Client; N],
}

impl<const N: usize> StrideClass<N> {
    /// All clients start runnable with zero service and fixed weights.
    pub(crate) fn new(weights: [u8; N]) -> Option<Self> {
        let mut clients = [Client {
            stride: 0,
            pass: 0,
            runnable: true,
        }; N];
        for (client, weight) in clients.iter_mut().zip(weights) {
            if !(1..=MAX_WEIGHT).contains(&weight) {
                return None;
            }
            client.stride = STRIDE_SCALE / u32::from(weight);
        }
        Some(Self { clients })
    }

    fn first_runnable(&self) -> Option<usize> {
        self.clients
            .iter()
            .enumerate()
            .filter(|(_, client)| client.runnable)
            .min_by_key(|(id, client)| (client.pass, *id))
            .map(|(id, _)| id)
    }

    /// Waking clients retain outstanding service debt. A sleeper cannot earn
    /// credit below the minimum pass of the currently runnable clients.
    pub(crate) fn set_runnable(&mut self, id: usize, runnable: bool) -> bool {
        let Some(client) = self.clients.get(id) else {
            return false;
        };
        if runnable && !client.runnable {
            let floor = self
                .first_runnable()
                .map_or(0, |first| self.clients[first].pass);
            self.clients[id].pass = client.pass.max(floor);
        }
        self.clients[id].runnable = runnable;
        true
    }

    /// Selects a runnable client and charges it one equal service quantum.
    /// Ties go to the smaller ID. Subtracting the winner's pass preserves the
    /// ready order and leaves the winner at zero before its stride is added.
    /// Thus every stored pass stays in 0..=STRIDE_SCALE indefinitely.
    pub(crate) fn dispatch(&mut self) -> Option<usize> {
        let winner = self.first_runnable()?;
        let base = self.clients[winner].pass;
        for client in &mut self.clients {
            client.pass = client.pass.saturating_sub(base);
        }
        self.clients[winner].pass = self.clients[winner].stride;
        Some(winner)
    }
}

pub(crate) fn reservation_checks() -> bool {
    use core::hint::black_box;

    let zero = black_box(0);
    if PeriodicReservation::new(0, zero, 0).is_some()
        || PeriodicReservation::new(0, zero, 7).is_some()
        || PeriodicReservation::new(0, black_box(8), 7).is_some()
    {
        return false;
    }
    let (start, budget, period) = black_box((100, 3, 10));
    let Some(mut server) = PeriodicReservation::new(start, budget, period) else {
        return false;
    };
    // Exhaustion, duplicate timestamps, no future credit, and one fresh
    // budget after three skipped boundaries. A late grant ends at the next
    // aligned boundary even with unspent budget.
    for (now, requested, expected) in [
        (99, 1, None),
        (100, 2, Some(2)),
        (100, 2, None),
        (101, 3, None),
        (102, 8, Some(1)),
        (103, 8, Some(0)),
        (109, 2, Some(0)),
        (108, 1, None),
        (110, 1, Some(1)),
        (145, 9, Some(3)),
        (148, 1, Some(0)),
        (159, 3, Some(1)),
        (160, 4, Some(3)),
    ] {
        if server.grant_at(now, requested) != expected {
            return false;
        }
    }
    let Some(mut last) = PeriodicReservation::new(black_box(u64::MAX - 8), 4, 6) else {
        return false;
    };
    last.grant_at(u64::MAX - 2, u64::MAX) == Some(2)
        && last.grant_at(u64::MAX, 1) == Some(0)
        && last.grant_at(u64::MAX - 1, 1).is_none()
}

pub(crate) fn stride_checks() -> bool {
    use core::hint::black_box;

    let Some(mut empty) = StrideClass::new([]) else {
        return false;
    };
    if empty.dispatch().is_some()
        || empty.set_runnable(0, true)
        || StrideClass::new(black_box([0])).is_some()
        || StrideClass::new(black_box([17])).is_some()
    {
        return false;
    }
    let Some(mut fair) = StrideClass::new(black_box([5, 7, 16])) else {
        return false;
    };
    let mut counts = [0; 3];
    for _ in 0..280 {
        let Some(id) = fair.dispatch() else {
            return false;
        };
        counts[id] += 1;
    }
    if counts != [50, 70, 160] {
        return false;
    }

    let Some(mut pair) = StrideClass::new(black_box([1, 2])) else {
        return false;
    };
    if pair.dispatch() != Some(0) || !pair.set_runnable(0, false) {
        return false;
    }
    // An immediate sleep/wake must preserve the first client's service debt.
    if !pair.set_runnable(0, true) || pair.dispatch() != Some(1) || !pair.set_runnable(0, false) {
        return false;
    }
    for _ in 0..4 {
        if pair.dispatch() != Some(1) {
            return false;
        }
    }
    if !pair.set_runnable(0, true) {
        return false;
    }
    // Sleeping through multiple turns cannot buy catch-up quanta on wake.
    for id in [0, 1, 1, 0] {
        if pair.dispatch() != Some(id) {
            return false;
        }
    }
    pair.set_runnable(0, false)
        && pair.set_runnable(1, false)
        && pair.dispatch().is_none()
        && pair.set_runnable(1, true)
        && pair.dispatch() == Some(1)
}

pub(crate) fn bench_checks() -> bool {
    reservation_checks() && stride_checks()
}

#[cfg(test)]
mod tests {
    #[test]
    fn aligned_budget_throttles_and_replenishes() {
        assert!(super::reservation_checks());
    }

    #[test]
    fn stride_shares_service_and_accounts_for_wakeups() {
        assert!(super::stride_checks());
    }
}
