//! Bounded scheduling and process-private lock state for four native roles.
//! The CPU0 adapter serializes entry, deadline processing, operation, dispatch and return.
use crate::domain_policy::{effective_priorities, LockPriority};
use crate::ready_queue::ReadyQueue;

const ROLES: usize = 4;
const EPERM: i32 = 1;
const ESRCH: i32 = 3;
const EAGAIN: i32 = 11;
const EBUSY: i32 = 16;
const EINVAL: i32 = 22;
const EDEADLK: i32 = 35;
const ENOTSUP: i32 = 95;
const ETIMEDOUT: i32 = 110;
const MAX_GENERATION: u64 = u64::MAX >> 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Vacant, Ready, Running, Sleeping(u64), Joining(u64),
    MutexWait(u64, Option<u64>), Zombie(u64), Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Completion { Join(u64), Mutex(i32) }

#[derive(Clone, Copy)]
pub(crate) struct MutexSlot {
    pub(crate) generation: u64,
    pub(crate) address: Option<u64>,
    pub(crate) owner: Option<u64>,
    pub(crate) waiters: [Option<u64>; ROLES],
    pub(crate) protocol: u8,
    pub(crate) users: u8,
    pub(crate) ceiling: u8,
}

impl MutexSlot {
    const EMPTY: Self = Self {
        generation: 0, address: None, owner: None, waiters: [None; ROLES],
        protocol: 0, users: 0, ceiling: 0,
    };

    fn remove_waiter(&mut self, index: usize) -> u64 {
        let handle = self.waiters[index].expect("registered waiter");
        self.waiters.copy_within(index + 1.., index);
        self.waiters[ROLES - 1] = None;
        handle
    }
}

fn slot(handle: u64) -> usize { (handle & 3) as usize }

pub(crate) struct Policy {
    base: [u8; ROLES],
    ranks: [u8; ROLES],
    states: [State; ROLES],
    generation: [u64; ROLES],
    rr: [bool; ROLES],
    quantum: Option<u64>,
    remaining: [u64; ROLES],
    queue: ReadyQueue<ROLES>,
    owner: Option<usize>,
    started: Option<u64>,
    now: u64,
    status: Option<i32>,
    locks: [MutexSlot; 4],
    // Acquisition order for each incarnation, including NONE resources.
    held: [[Option<usize>; 4]; ROLES],
    completion: [Option<Completion>; ROLES],
}

impl Policy {
    pub(crate) fn new(base: [u8; ROLES], rr_quantum: Option<u64>) -> Self {
        assert!(base.iter().all(|rank| (1..=31).contains(rank)));
        assert!(rr_quantum != Some(0));
        Self {
            base, ranks: base, states: [State::Running, State::Vacant, State::Vacant, State::Vacant],
            generation: [0; ROLES], rr: [rr_quantum.is_some(); ROLES], quantum: rr_quantum,
            remaining: [rr_quantum.unwrap_or(0); ROLES], queue: ReadyQueue::new(), owner: Some(0),
            started: None, now: 0, status: None, locks: [MutexSlot::EMPTY; 4],
            held: [[None; 4]; ROLES], completion: [None; ROLES],
        }
    }

    pub(crate) fn current(&self) -> Option<usize> { self.owner }
    pub(crate) fn state(&self, id: usize) -> State { self.states[id] }
    pub(crate) fn base(&self, id: usize) -> u8 { self.base[id] }
    pub(crate) fn effective(&self, id: usize) -> u8 { self.ranks[id] }
    pub(crate) fn is_rr(&self, id: usize) -> bool { self.rr[id] }
    pub(crate) fn residual(&self, id: usize) -> u64 { self.remaining[id] }
    pub(crate) fn ready_order(&self) -> &[usize] { self.queue.entries() }
    pub(crate) fn mutex_slots(&self) -> &[MutexSlot; 4] { &self.locks }
    pub(crate) fn status(&self) -> Option<i32> { self.status }
    pub(crate) fn handle(&self, id: usize) -> u64 { (self.generation[id] << 2) | id as u64 }
    pub(crate) fn take_completion(&mut self, id: usize) -> Option<Completion> {
        let result = self.completion[id].take();
        if result == Some(Completion::Mutex(0)) {
            core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
        }
        result
    }

    fn recompute(&mut self) {
        let snapshot = core::array::from_fn(|id| {
            let held_ceiling = self.held[id].iter().flatten().map(|&k| &self.locks[k])
                .filter(|lock| lock.protocol == 2).map(|lock| lock.ceiling).max();
            let waits_for = match self.states[id] {
                State::MutexWait(token, _) if self.locks[slot(token)].protocol == 1 =>
                    self.locks[slot(token)].owner.map(|owner| slot(owner) as u8),
                _ => None,
            };
            LockPriority { base: self.base[id], held_ceiling, waits_for }
        });
        let ranks = effective_priorities(&snapshot).expect("admitted acyclic wait graph");
        // Existing equal peers precede arrivals from another rank. Preserve
        // ready order within each partition, regardless of slot numbers.
        let old = core::mem::replace(&mut self.queue, ReadyQueue::new());
        for changed in [false, true] {
            for &id in old.entries() {
                if (self.ranks[id] != ranks[id]) == changed { assert!(self.queue.push_back(id)); }
            }
        }
        self.ranks = ranks;
    }

    fn ready(&mut self, id: usize) {
        self.states[id] = State::Ready;
        assert!(self.queue.push_back(id));
    }

    fn block(&mut self, state: State) {
        let id = self.owner.take().expect("active caller");
        self.states[id] = state;
        self.remaining[id] = self.quantum.unwrap_or(0);
    }

    pub(crate) fn init_epoch(&mut self, now: u64) { self.now = now; }
    pub(crate) fn resume(&mut self, stamp: u64) { self.started = self.owner.map(|_| stamp); }

    pub(crate) fn pause(&mut self, entry: u64) {
        if let (Some(start), Some(id)) = (self.started, self.owner) {
            if let Some(elapsed) = entry.checked_sub(start) {
                if self.rr[id] { self.remaining[id] = self.remaining[id].saturating_sub(elapsed); }
                self.started = None;
            }
        }
    }

    fn deadline(&self, id: usize) -> Option<u64> {
        match self.states[id] {
            State::Sleeping(at) | State::MutexWait(_, Some(at)) => Some(at),
            _ => None,
        }
    }

    pub(crate) fn advance(&mut self, decision: u64) {
        if decision < self.now { return; }
        self.now = decision;
        while let Some((_, id)) = (0..ROLES).filter_map(|id| Some((self.deadline(id)?, id)))
            .filter(|&(at, _)| at <= decision).min()
        {
            if let State::MutexWait(token, _) = self.states[id] {
                let handle = self.handle(id);
                let lock = &mut self.locks[slot(token)];
                assert_eq!(lock.generation, token >> 2);
                let index = lock.waiters.iter().position(|&w| w == Some(handle)).expect("timed waiter");
                lock.remove_waiter(index);
                self.completion[id] = Some(Completion::Mutex(ETIMEDOUT));
                self.states[id] = State::Ready;
                self.recompute();
            }
            self.ready(id);
        }
    }

    pub(crate) fn next_timer(&self, now: u64) -> Option<u64> {
        let deadline = (0..ROLES).filter_map(|id| self.deadline(id)).min();
        let expiry = self.owner.filter(|&id| self.rr[id] && self.queue.entries().iter()
            .any(|&peer| self.ranks[peer] == self.ranks[id]))
            .and_then(|id| self.started.unwrap_or(now).checked_add(self.remaining[id]));
        deadline.into_iter().chain(expiry).min()
    }

    pub(crate) fn dispatch(&mut self) {
        if let Some(id) = self.owner {
            let priority = self.ranks[id];
            let expired = self.rr[id] && self.remaining[id] == 0;
            let rotate = expired && self.queue.entries().iter().any(|&peer| self.ranks[peer] == priority);
            if expired { self.remaining[id] = self.quantum.unwrap(); }
            if rotate {
                self.ready(id);
            } else if self.queue.entries().iter().any(|&peer| self.ranks[peer] > priority) {
                self.states[id] = State::Ready;
                assert!(self.queue.push_front(id));
            } else {
                return;
            }
        }
        self.owner = self.queue.pop_highest(|id| self.ranks[id]);
        if let Some(id) = self.owner { self.states[id] = State::Running; }
    }

    pub(crate) fn create(&mut self, explicit: Option<(bool, u8)>) -> Result<u64, i32> {
        if self.owner.is_none() { return Err(ESRCH); }
        if explicit.is_some_and(|(rr, _)| rr && self.quantum.is_none()) { return Err(ENOTSUP); }
        let matches = |id: usize| explicit.is_none_or(|(_, priority)| self.base[id] == priority);
        if !(1..ROLES).any(matches) { return Err(EINVAL); }
        let id = (1..ROLES).find(|&id| matches(id) && self.states[id] == State::Vacant
            && self.generation[id] < MAX_GENERATION).ok_or(EAGAIN)?;
        self.generation[id] += 1;
        self.rr[id] = explicit.map_or(self.quantum.is_some(), |(rr, _)| rr);
        self.remaining[id] = self.quantum.unwrap_or(0);
        self.completion[id] = None;
        self.ready(id);
        Ok(self.handle(id))
    }

    pub(crate) fn yield_now(&mut self) {
        if let Some(id) = self.owner.take() {
            self.remaining[id] = self.quantum.unwrap_or(0);
            self.ready(id);
        }
    }

    pub(crate) fn sleep(&mut self, deadline: u64) {
        if deadline > self.now { self.block(State::Sleeping(deadline)); }
    }

    fn dependency(&self, id: usize) -> Option<u64> {
        match self.states[id] {
            State::Joining(target) => Some(target),
            State::MutexWait(token, _) => self.locks[slot(token)].owner,
            _ => None,
        }
    }

    fn cycle(&self, caller: u64, mut target: u64) -> bool {
        for _ in 0..ROLES {
            if target == caller { return true; }
            let Some(next) = self.dependency(slot(target)) else { return false; };
            target = next;
        }
        true
    }

    fn reap(&mut self, id: usize) {
        self.states[id] = if self.held[id].iter().any(Option::is_some) { State::Ended } else { State::Vacant };
    }

    pub(crate) fn join(&mut self, handle: u64) -> Result<Option<u64>, i32> {
        let id = slot(handle);
        let caller = self.owner.ok_or(ESRCH)?;
        if self.handle(id) != handle || matches!(self.states[id], State::Vacant | State::Ended) {
            return Err(ESRCH);
        }
        if self.cycle(self.handle(caller), handle) { return Err(EDEADLK); }
        if id == 0 || self.states.contains(&State::Joining(handle)) { return Err(EINVAL); }
        if let State::Zombie(value) = self.states[id] {
            self.reap(id);
            return Ok(Some(value));
        }
        self.block(State::Joining(handle));
        Ok(None)
    }

    pub(crate) fn thread_exit(&mut self, value: u64) {
        let id = self.owner.take().expect("exiting caller");
        let handle = self.handle(id);
        if id == 0 {
            self.states[id] = State::Ended;
        } else if let Some(joiner) = self.states.iter().position(|&s| s == State::Joining(handle)) {
            self.reap(id);
            self.completion[joiner] = Some(Completion::Join(value));
            self.ready(joiner);
        } else {
            self.states[id] = State::Zombie(value);
        }
        if self.states[0] == State::Ended && self.states.iter().all(|state|
            matches!(state, State::Ended | State::Vacant | State::Zombie(_))) {
            self.process_exit(0);
        }
    }

    pub(crate) fn process_exit(&mut self, status: i32) {
        if self.status.is_none() {
            self.status = Some(status);
            self.owner = None;
            self.queue = ReadyQueue::new();
            self.states = [State::Ended; ROLES];
            self.locks = [MutexSlot::EMPTY; 4];
            self.held = [[None; 4]; ROLES];
            self.completion = [None; ROLES];
        }
    }

    pub(crate) fn mutex_init(&mut self, address: u64, protocol: u8, users: u8, ceiling: u8) -> Result<u64, i32> {
        if self.owner.is_none() { return Err(ESRCH); }
        if address == 0 || address % 8 != 0 || address.checked_add(7).is_none()
            || protocol > 2 || users == 0 || users & !0x0f != 0 || ceiling > 31 {
            return Err(EINVAL);
        }
        if protocol == 2 && (ceiling == 0 || self.base.iter().enumerate()
            .any(|(id, &base)| users & (1 << id) != 0 && base > ceiling)) {
            return Err(EINVAL);
        }
        if self.locks.iter().any(|lock| lock.address == Some(address)) { return Err(EBUSY); }
        let index = self.locks.iter().position(|lock| lock.address.is_none() && lock.generation < MAX_GENERATION)
            .ok_or(EAGAIN)?;
        let generation = self.locks[index].generation + 1;
        self.locks[index] = MutexSlot {
            generation, address: Some(address), protocol, users, ceiling, ..MutexSlot::EMPTY
        };
        Ok((generation << 2) | index as u64)
    }

    fn acquire(&mut self, id: usize, token: u64, try_only: bool, deadline: Option<u64>) -> Result<(), i32> {
        let index = slot(token);
        let lock = &self.locks[index];
        if lock.users & (1 << id) == 0 { return Err(EPERM); }
        let caller = self.handle(id);
        if let Some(owner) = lock.owner {
            if try_only { return Err(EBUSY); }
            if self.cycle(caller, owner) { return Err(EDEADLK); }
            if deadline.is_some_and(|at| at <= self.now) { return Err(ETIMEDOUT); }
        }
        // All-NONE locks keep the ordinary independent-lock semantics. As
        // soon as a protocol participates, other owned locks count for depth.
        let stack = &self.held[id];
        let depth = stack.iter().flatten().count();
        let has_protocol = lock.protocol != 0 || stack.iter().flatten().any(|&k| self.locks[k].protocol != 0);
        if depth >= stack.len() || (has_protocol && depth >= 2) { return Err(EAGAIN); }
        if lock.owner.is_none() {
            self.held[id][depth] = Some(index);
            self.locks[index].owner = Some(caller);
            core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
        } else {
            let free = self.locks[index].waiters.iter_mut().find(|w| w.is_none()).ok_or(EAGAIN)?;
            *free = Some(caller);
            self.block(State::MutexWait(token, deadline));
        }
        self.recompute();
        Ok(())
    }

    fn release(&mut self, id: usize, token: u64) -> Result<(), i32> {
        let index = slot(token);
        let lock = &self.locks[index];
        if lock.owner != Some(self.handle(id)) { return Err(EPERM); }
        let owned = &self.held[id];
        let at = owned.iter().position(|&k| k == Some(index)).expect("owned lock in stack");
        if lock.protocol != 0 && owned[at + 1..].iter().any(Option::is_some) { return Err(EINVAL); }
        let mut winner = None;
        let mut best = 0;
        for (order, &waiter) in lock.waiters.iter().enumerate() {
            let Some(handle) = waiter else { break; };
            let rank = self.ranks[slot(handle)];
            if winner.is_none() || (lock.protocol != 0 && rank > best) {
                winner = Some(order);
                best = rank;
            }
        }
        core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
        self.held[id].copy_within(at + 1.., at);
        self.held[id][3] = None;
        let next = winner.map(|order| self.locks[index].remove_waiter(order));
        self.locks[index].owner = next;
        if let Some(handle) = next {
            let successor = slot(handle);
            assert_eq!(self.handle(successor), handle);
            assert!(matches!(self.states[successor], State::MutexWait(t, _) if t == token));
            let pos = self.held[successor].iter().position(Option::is_none).expect("reserved nesting slot");
            self.held[successor][pos] = Some(index);
            self.states[successor] = State::Ready;
            self.completion[successor] = Some(Completion::Mutex(0));
        }
        self.recompute();
        if let Some(handle) = next { self.ready(slot(handle)); }
        Ok(())
    }

    pub(crate) fn mutex(&mut self, op: u64, address: u64, token: u64, deadline: Option<u64>) -> Result<(), i32> {
        let id = self.owner.ok_or(ESRCH)?;
        let index = slot(token);
        let lock = &self.locks[index];
        if lock.address != Some(address) || lock.generation != token >> 2 { return Err(EINVAL); }
        match op {
            14 if deadline.is_none() => {
                if lock.owner.is_some() || lock.waiters[0].is_some() { return Err(EBUSY); }
                self.locks[index] = MutexSlot { generation: lock.generation, ..MutexSlot::EMPTY };
                Ok(())
            }
            15 => self.acquire(id, token, false, deadline),
            16 if deadline.is_none() => self.acquire(id, token, true, None),
            17 if deadline.is_none() => self.release(id, token),
            _ => Err(EINVAL),
        }
    }
}
