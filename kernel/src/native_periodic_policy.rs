//! Anchored recurring releases for persistent native processes on one CPU.
use super::{
    dispatch_policy::{DispatchJob, DispatchPolicy},
    ready_queue::ReadyQueue,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub(crate) enum Error { Configuration = 1, Conversion, Overflow, TimeRegression, NoOwner }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action { Poll, Yield, WaitNext, Exit, Sleep(u64) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) first_release: u64,
    pub(crate) period: u64,
    pub(crate) relative_deadline: u64,
    pub(crate) generation: u64,
    pub(crate) released: u64,
    pub(crate) miss_cursor: u64,
    pub(crate) missed: u64,
    pub(crate) overlaps: u64,
    pub(crate) cancelled: u64,
    pub(crate) sleep_until: Option<u64>,
    pub(crate) exited: bool,
}

fn ticks(ns: u64, hz: u64) -> Result<u64, Error> {
    let product = u128::from(ns) * u128::from(hz);
    if product % 1_000_000_000 != 0 { return Err(Error::Conversion); }
    let value = u64::try_from(product / 1_000_000_000).map_err(|_| Error::Overflow)?;
    if value > i64::MAX as u64 { return Err(Error::Overflow); }
    Ok(value)
}

impl Slot {
    const EMPTY: Self = Self {
        first_release: 0, period: 0, relative_deadline: 0, generation: 0, released: 0,
        miss_cursor: 0, missed: 0, overlaps: 0, cancelled: 0, sleep_until: None, exited: false,
    };

    pub(crate) fn release(&self, generation: u64) -> Result<u64, Error> {
        self.first_release.checked_add(generation.checked_mul(self.period).ok_or(Error::Overflow)?)
            .ok_or(Error::Overflow)
    }

    pub(crate) fn deadline(&self, generation: u64) -> Result<u64, Error> {
        self.release(generation)?.checked_add(self.relative_deadline).ok_or(Error::Overflow)
    }

    fn runnable(self) -> bool {
        !self.exited && self.sleep_until.is_none() && self.generation < self.released
    }

    // Number of releases up to and including the sample.
    fn count(&self, now: u64) -> Result<u64, Error> {
        match now.checked_sub(self.first_release) {
            None => Ok(0),
            Some(elapsed) => (elapsed / self.period).checked_add(1).ok_or(Error::Overflow),
        }
    }

    fn admit_until(&mut self, now: u64) -> Result<(), Error> {
        let count = self.count(now)?;
        let arrivals = count - self.released;
        // The first arrival fills an empty slot. Every other new release overlaps.
        let empty = u64::from(self.generation == self.released && arrivals != 0);
        self.overlaps = self.overlaps.checked_add(arrivals - empty).ok_or(Error::Overflow)?;
        self.released = count;
        Ok(())
    }

    fn advance(&mut self, now: u64, action: Action) -> Result<(), Error> {
        if self.exited { return Ok(()); }
        // All events strictly before the call precede its commit. A timestamp
        // equal to the deadline remains on time, including after a duplicate IRQ.
        if let Some(before) = now.checked_sub(1) {
            // A repeated sample may already have admitted releases at now.
            if self.count(before)? > self.released { self.admit_until(before)?; }
            if let Some(limit) = before.checked_sub(self.deadline(0)?) {
                let due = (limit / self.period).checked_add(1).ok_or(Error::Overflow)?;
                let misses = due.saturating_sub(self.miss_cursor);
                self.missed = self.missed.checked_add(misses).ok_or(Error::Overflow)?;
                self.miss_cursor = self.miss_cursor.max(due);
            }
        }
        match action {
            Action::WaitNext | Action::Exit => {
                // An earlier sample at this counter may have admitted the next
                // release while this older generation was still outstanding.
                // Reopen just that admission so the completion precedes it.
                // If the caller is completing the new generation itself, its
                // release necessarily precedes this call and stays committed.
                if let Some(latest) = self.released.checked_sub(1) {
                    if self.generation < latest && self.release(latest)? == now {
                        self.released = latest;
                        self.overlaps = self.overlaps.checked_sub(1).ok_or(Error::Overflow)?;
                    }
                }
                self.generation = self.generation.checked_add(1).ok_or(Error::Overflow)?;
                self.miss_cursor = self.miss_cursor.max(self.generation);
                if action == Action::Exit {
                    self.cancelled = self.released - self.generation;
                    self.exited = true;
                    return Ok(());
                }
            }
            Action::Sleep(until) if until > now => self.sleep_until = Some(until),
            _ => {}
        }
        self.admit_until(now)?;
        if self.sleep_until.is_some_and(|until| until <= now) { self.sleep_until = None; }
        Ok(())
    }

    fn next(&self, now: u64) -> Result<Option<u64>, Error> {
        if self.exited { return Ok(None); }
        let mut next = self.release(self.released)?;
        // Reject an unrepresentable deadline before the corresponding generation runs.
        self.deadline(self.released)?;
        if self.miss_cursor < self.released {
            let deadline = self.deadline(self.miss_cursor)?;
            let observation = if deadline == now {
                deadline.checked_add(1).ok_or(Error::Overflow)?
            } else { deadline };
            next = next.min(observation);
        }
        if let Some(until) = self.sleep_until { next = next.min(until); }
        Ok(Some(next))
    }
}

pub(crate) struct Calendar<const N: usize> {
    policy: DispatchPolicy,
    slots: [Slot; N],
    ready: ReadyQueue<N>,
    owner: Option<usize>,
    last: u64,
    compare: Option<u64>,
    // Slots enqueued exactly at the last sample by polls that retained the
    // incumbent. Their slot-order tie with a same-counter commit is still open.
    polled_arrivals: [bool; N],
}

impl<const N: usize> Calendar<N> {
    pub(crate) fn new(policy: DispatchPolicy, tuples_ns: [[u64; 3]; N], hz: u64, epoch: u64) -> Result<Self, Error> {
        if !(1..=3).contains(&N) || hz == 0 ||
            !matches!(policy, DispatchPolicy::RateMonotonic | DispatchPolicy::DeadlineMonotonic) {
            return Err(Error::Configuration);
        }
        let mut slots = [Slot::EMPTY; N];
        for (slot, [phase, period, relative]) in slots.iter_mut().zip(tuples_ns) {
            if relative == 0 || relative > period { return Err(Error::Configuration); }
            slot.first_release = epoch.checked_add(ticks(phase, hz)?).ok_or(Error::Overflow)?;
            slot.period = ticks(period, hz)?;
            slot.relative_deadline = ticks(relative, hz)?;
            slot.deadline(0)?;
        }
        let compare = slots.iter().map(|slot| slot.first_release).min();
        Ok(Self { policy, slots, ready: ReadyQueue::new(), owner: None, last: 0, compare, polled_arrivals: [false; N] })
    }

    pub(crate) fn owner(&self) -> Option<usize> { self.owner }
    pub(crate) fn next_compare(&self) -> Option<u64> { self.compare }
    pub(crate) fn slot(&self, id: usize) -> Option<Slot> { self.slots.get(id).copied() }
    #[cfg(jnox_native_el0)]
    pub(crate) fn ready(&self) -> &[usize] { self.ready.entries() }

    pub(crate) fn step(&mut self, now: u64, action: Action) -> Result<Option<usize>, Error> {
        if now < self.last { return Err(Error::TimeRegression); }
        if self.owner.is_none() && action != Action::Poll { return Err(Error::NoOwner); }
        if let Action::Sleep(until) = action {
            if until.saturating_sub(now) > i64::MAX as u64 { return Err(Error::Overflow); }
        }
        let mut slots = self.slots;
        let mut arrivals = [(u64::MAX, usize::MAX); N];
        let mut compare: Option<u64> = None;
        for (id, slot) in slots.iter_mut().enumerate() {
            let before = *slot;
            let own = self.owner == Some(id);
            slot.advance(now, if own { action } else { Action::Poll })?;
            if let Some(next) = slot.next(now)? {
                compare = Some(compare.map_or(next, |old| old.min(next)));
            }
            if !before.runnable() && slot.runnable() {
                let arrived = if before.generation == before.released {
                    slot.release(before.released)?
                } else { 0 };
                arrivals[id] = (arrived.max(before.sleep_until.unwrap_or(0)), id);
            } else if own && slot.runnable() && matches!(action, Action::WaitNext | Action::Yield) {
                arrivals[id] = (now, id);
            }
        }
        // Commit only after every timestamp and counter for all slots is representable.
        let incumbent = self.owner;
        let mut provisional = if now == self.last { self.polled_arrivals } else { [false; N] };
        if matches!(action, Action::WaitNext | Action::Exit) {
            // Merge previously observed same-instant arrivals with the
            // submitting owner's next generation in the original slot order.
            while let Some(id) = self.ready.pop_ranked(
                |id| provisional[id].then_some(id), |a, b| a < b,
            ) {
                arrivals[id] = (now, id);
            }
        }
        if action == Action::Poll {
            for &(time, id) in &arrivals {
                if id < N && time == now { provisional[id] = true; }
            }
        }
        arrivals.sort_unstable();
        if let Some(id) = self.owner {
            if slots[id].runnable() && !matches!(action, Action::WaitNext | Action::Yield) {
                assert!(self.ready.push_front(id));
            }
        }
        for (_, id) in arrivals {
            if id != usize::MAX { assert!(self.ready.push_back(id)); }
        }
        self.owner = self.policy.pop_ready(&mut self.ready, now, |id| {
            let s = slots[id];
            Some(DispatchJob {
                release: s.first_release, service: 1, remaining: 1, priority: 0,
                period: s.period, relative_deadline: s.relative_deadline,
            })
        });
        self.polled_arrivals = if action == Action::Poll && self.owner == incumbent {
            provisional
        } else {
            [false; N]
        };
        self.slots = slots;
        self.last = now;
        self.compare = compare;
        Ok(self.owner)
    }
}
