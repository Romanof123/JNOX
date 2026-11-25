//! Thread lifecycle for a single process with a main thread and one joinable worker.
use crate::ready_queue::ReadyQueue;

const EPERM: i32 = 1;
const EBUSY: i32 = 16;
const MAIN: usize = 0;
const WORKER: usize = 1;
const ESRCH: i32 = 3;
const EAGAIN: i32 = 11;
const EINVAL: i32 = 22;
const EDEADLK: i32 = 35;
const EOVERFLOW: i32 = 75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Vacant,
    Running,
    Ready,
    Sleeping(u64),
    Joining,
    MutexWait(u64),
    Zombie(u64),
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ThreadId {
    pub(crate) tid: usize,
    pub(crate) generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MutexSlot {
    pub(crate) generation: u64,
    pub(crate) address: Option<u64>,
    pub(crate) owner: Option<ThreadId>,
    pub(crate) waiter: Option<ThreadId>,
}

impl MutexSlot {
    const EMPTY: Self = Self { generation: 0, address: None, owner: None, waiter: None };
}

pub(crate) struct Policy {
    states: [State; 2],
    locks: [MutexSlot; 4],
    queue: ReadyQueue<2>,
    owner: Option<usize>,
    generation: u64,
    status: Option<i32>,
}

impl Policy {
    pub(crate) const fn new() -> Self {
        Self {
            states: [State::Running, State::Vacant],
            locks: [MutexSlot::EMPTY; 4],
            queue: ReadyQueue::new(),
            owner: Some(MAIN),
            generation: 0,
            status: None,
        }
    }

    pub(crate) fn current(&self) -> Option<usize> { self.owner }
    pub(crate) fn state(&self, id: usize) -> State { self.states[id] }
    pub(crate) fn ready(&self) -> &[usize] { self.queue.entries() }
    pub(crate) fn generation(&self) -> u64 { self.generation }
    pub(crate) fn status(&self) -> Option<i32> { self.status }

    pub(crate) fn mutex_slots(&self) -> &[MutexSlot; 4] { &self.locks }

    fn thread_id(&self, tid: usize) -> ThreadId {
        ThreadId { tid, generation: if tid == MAIN { 0 } else { self.generation } }
    }

    /// The caller checks process mapping permissions before entering this transition.
    pub(crate) fn mutex(&mut self, op: u64, addr: u64, token: u64) -> Result<u64, i32> {
        if addr == 0 || addr % 8 != 0 || addr.checked_add(7).is_none() { return Err(EINVAL); }
        let caller = self.thread_id(self.owner.ok_or(ESRCH)?);
        if op == 13 {
            if token != 0 { return Err(EINVAL); }
            if self.locks.iter().any(|s| s.address == Some(addr)) { return Err(EBUSY); }
            let index = self.locks.iter().position(|s|
                s.address.is_none() && s.generation < (u64::MAX >> 2)
            ).ok_or(EAGAIN)?;
            let slot = &mut self.locks[index];
            slot.generation += 1;
            slot.address = Some(addr);
            return Ok((slot.generation << 2) | index as u64);
        }

        let slot = &mut self.locks[(token & 3) as usize];
        if slot.address != Some(addr) || slot.generation != token >> 2 { return Err(EINVAL); }
        match op {
            14 => {
                if slot.owner.is_some() || slot.waiter.is_some() { return Err(EBUSY); }
                slot.address = None;
            }
            15 | 16 => {
                if slot.owner.is_none() {
                    slot.owner = Some(caller);
                } else if op == 16 {
                    return Err(EBUSY);
                } else if slot.owner == Some(caller) {
                    return Err(EDEADLK);
                } else {
                    // With a live owner only one waiter is possible. After
                    // owner death, both surviving incarnations can eventually
                    // block forever. Preserve the first waiter and both states.
                    if slot.waiter.is_none() { slot.waiter = Some(caller); }
                    self.states[caller.tid] = State::MutexWait(token);
                    self.owner = None;
                    self.dispatch();
                }
            }
            17 => {
                if slot.owner != Some(caller) { return Err(EPERM); }
                let waiter = slot.waiter.take();
                slot.owner = waiter;
                if let Some(next) = waiter {
                    assert_eq!(self.thread_id(next.tid), next);
                    assert_eq!(self.states[next.tid], State::MutexWait(token));
                    self.enqueue(next.tid);
                }
            }
            _ => return Err(EINVAL),
        }
        Ok(0)
    }

    fn enqueue(&mut self, id: usize) {
        assert!(self.queue.push_back(id));
        self.states[id] = State::Ready;
    }

    fn dispatch(&mut self) {
        if self.owner.is_none() {
            self.owner = self.queue.pop_highest(|_| 0);
            if let Some(id) = self.owner { self.states[id] = State::Running; }
        }
    }

    pub(crate) fn create(&mut self) -> Result<u64, i32> {
        if self.owner != Some(MAIN) || self.states[WORKER] != State::Vacant
            || self.generation == u64::MAX >> 1 {
            return Err(EAGAIN);
        }
        self.generation += 1;
        self.enqueue(WORKER);
        Ok((self.generation << 1) | 1)
    }

    /// None means main's post-SVC frame is awaiting the worker's exit.
    pub(crate) fn join(&mut self, handle: u64) -> Result<Option<u64>, i32> {
        if matches!(self.states[WORKER], State::Vacant | State::Ended)
            || handle != ((self.generation << 1) | 1) {
            return Err(ESRCH);
        }
        match self.owner {
            Some(WORKER) => Err(EDEADLK),
            Some(MAIN) => {
                if let State::Zombie(value) = self.states[WORKER] {
                    self.states[WORKER] = State::Vacant;
                    Ok(Some(value))
                } else {
                    self.states[MAIN] = State::Joining;
                    self.owner = None;
                    self.dispatch();
                    Ok(None)
                }
            }
            _ => Err(ESRCH),
        }
    }

    /// Some(value) completes main's saved join, including a zero-valued result.
    pub(crate) fn thread_exit(&mut self, value: u64) -> Option<u64> {
        let id = self.owner.take()?;
        self.states[id] = State::Ended;
        if id == WORKER && self.states[MAIN] == State::Joining {
            self.states[WORKER] = State::Vacant;
            self.enqueue(MAIN);
            self.dispatch();
            return Some(value);
        }
        if id == WORKER { self.states[WORKER] = State::Zombie(value); }
        if self.states[MAIN] == State::Ended &&
            matches!(self.states[WORKER], State::Vacant | State::Zombie(_)) {
            self.process_exit(0);
        } else {
            self.dispatch();
        }
        None
    }

    pub(crate) fn process_exit(&mut self, status: i32) {
        if self.status.is_none() {
            self.status = Some(status);
            self.owner = None;
            self.queue = ReadyQueue::new();
            self.states = [State::Ended; 2];
            self.locks = [MutexSlot::EMPTY; 4];
        }
    }

    pub(crate) fn yield_now(&mut self) {
        if let Some(id) = self.owner.take() {
            self.enqueue(id);
            self.dispatch();
        }
    }

    pub(crate) fn sleep(&mut self, now: u64, until: u64) -> Result<(), i32> {
        let id = self.owner.ok_or(ESRCH)?;
        if until > now {
            if until - now > i64::MAX as u64 { return Err(EOVERFLOW); }
            self.states[id] = State::Sleeping(until);
            self.owner = None;
            self.dispatch();
        }
        Ok(())
    }

    fn first_sleeper(&self) -> Option<(u64, usize)> {
        self.states.iter().enumerate().filter_map(|(id, state)| {
            if let State::Sleeping(at) = state { Some((*at, id)) } else { None }
        }).min()
    }

    pub(crate) fn next_compare(&self) -> Option<u64> {
        self.first_sleeper().map(|(at, _)| at)
    }

    pub(crate) fn wake_due(&mut self, now: u64) {
        while let Some((at, id)) = self.first_sleeper() {
            if at > now { break; }
            self.enqueue(id);
        }
        self.dispatch();
    }
}

/// RR timing uses the lifecycle's sole FIFO. A turn can span several measured
/// EL0 intervals; kernel gaps do not add service or move its nominal expiry.
pub(crate) struct RoundRobin {
    quantum: u64,
    turn: Option<(usize, u64)>,
    started: Option<u64>,
    last_sample: u64,
    service: [u64; 2],
}

impl RoundRobin {
    pub(crate) const fn new(quantum: u64) -> Result<Self, i32> {
        if quantum == 0 || quantum > i64::MAX as u64 { return Err(EINVAL); }
        Ok(Self { quantum, turn: None, started: None, last_sample: 0, service: [0; 2] })
    }

    /// Sample near the actual return, after owner selection and frame setup.
    pub(crate) fn resume(&mut self, tid: usize, now: u64) -> Result<(), i32> {
        if tid >= self.service.len() || self.started.is_some() || now < self.last_sample {
            return Err(EINVAL);
        }
        if let Some((owner, _)) = self.turn {
            if owner != tid { return Err(EINVAL); }
        } else {
            let end = crate::fixed_policy::relative_deadline(now, self.quantum).ok_or(EOVERFLOW)?;
            self.turn = Some((tid, end));
        }
        self.started = Some(now);
        self.last_sample = now;
        Ok(())
    }

    /// Charge exactly the interrupted thread; a late interrupt is uncapped.
    pub(crate) fn pause(&mut self, tid: usize, entry: u64) -> Result<u64, i32> {
        if self.turn.map(|(id, _)| id) != Some(tid) { return Err(EINVAL); }
        let elapsed = entry.checked_sub(self.started.ok_or(EINVAL)?).ok_or(EINVAL)?;
        self.service[tid] = self.service[tid].checked_add(elapsed).ok_or(EOVERFLOW)?;
        self.started = None;
        self.last_sample = entry;
        Ok(elapsed)
    }

    pub(crate) fn end_turn(&mut self) {
        assert!(self.started.is_none());
        self.turn = None;
    }

    pub(crate) fn logical_end(&self) -> Option<u64> { self.turn.map(|(_, end)| end) }
    pub(crate) fn remaining(&self, now: u64) -> u64 {
        self.logical_end().map_or(0, |end| end.saturating_sub(now))
    }
    pub(crate) fn service(&self, tid: usize) -> u64 { self.service[tid] }

    pub(crate) fn compare(&self, policy: &Policy, now: u64) -> Option<u64> {
        let slice = self.logical_end().filter(|_| !policy.ready().is_empty());
        policy.next_compare().into_iter().chain(slice).min().map(|at| at.max(now))
    }

    /// Wake first so an equal-time arrival participates in the single decision.
    /// A late interrupt never assigns fictitious service to the waiting peer.
    pub(crate) fn timer(&mut self, policy: &mut Policy, now: u64) -> bool {
        policy.wake_due(now);
        if self.turn.is_some() &&
            crate::fixed_policy::rotate_expired(self.remaining(now), || !policy.ready().is_empty()) {
            self.end_turn();
            policy.yield_now();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_claim_and_zombie_each_deliver_one_full_width_result() {
        let mut p = Policy::new();
        let first = p.create().unwrap();
        assert_eq!(p.current(), Some(MAIN));
        assert_eq!(p.ready(), &[WORKER]);
        assert_eq!(p.join(first), Ok(None));
        assert_eq!(p.join(first), Err(EDEADLK));
        assert_eq!(p.thread_exit(u64::MAX), Some(u64::MAX));
        assert_eq!(p.current(), Some(MAIN));
        assert_eq!(p.join(first), Err(ESRCH));
        let second = p.create().unwrap();
        p.yield_now();
        assert_eq!(p.thread_exit(0), None);
        assert_eq!(p.create(), Err(EAGAIN));
        assert_eq!(p.join(first), Err(ESRCH));
        assert_eq!(p.join(second), Ok(Some(0)));
        assert_eq!(p.join(second), Err(ESRCH));
    }

    #[test]
    fn late_wakes_order_by_deadline_then_slot_without_preempting() {
        for (main_at, first) in [(12, WORKER), (10, MAIN)] {
            let mut p = Policy::new();
            p.create().unwrap();
            p.sleep(1, main_at).unwrap();
            p.sleep(2, 10).unwrap();
            assert_eq!(p.current(), None);
            assert_eq!(p.next_compare(), Some(10));
            p.wake_due(12);
            assert_eq!(p.current(), Some(first));
            assert_eq!(p.ready(), &[1 - first]);
            p.wake_due(12);
            assert_eq!(p.next_compare(), None);
            p.yield_now();
            assert_eq!(p.current(), Some(1 - first));
        }
        let mut p = Policy::new();
        p.create().unwrap();
        p.sleep(0, 5).unwrap();
        p.wake_due(5);
        assert_eq!(p.current(), Some(WORKER));
        assert_eq!(p.ready(), &[MAIN]);
    }

    #[test]
    fn thread_and_process_exit_cancel_only_their_own_work() {
        let mut p = Policy::new();
        p.create().unwrap();
        p.thread_exit(7);
        assert_eq!(p.current(), Some(WORKER));
        p.sleep(0, 5).unwrap();
        p.wake_due(5);
        p.thread_exit(8);
        assert_eq!(p.status(), Some(0));
        let mut p = Policy::new();
        let h = p.create().unwrap();
        p.join(h).unwrap();
        p.sleep(0, 5).unwrap();
        p.process_exit(-4);
        p.wake_due(10);
        assert_eq!(p.status(), Some(-4));
        assert_eq!(p.next_compare(), None);
        assert_eq!(p.current(), None);
        assert_eq!(p.state(MAIN), State::Ended);
    }

    #[test]
    fn exhaustion_and_invalid_handles_leave_the_current_owner_unchanged() {
        let mut p = Policy::new();
        p.generation = (u64::MAX >> 1) - 1;
        assert_eq!(p.create(), Ok(u64::MAX));
        assert_eq!(p.join(0), Err(ESRCH));
        assert_eq!(p.sleep(0, u64::MAX), Err(EOVERFLOW));
        assert_eq!(p.sleep(5, 4), Ok(()));
        assert_eq!(p.current(), Some(MAIN));
        p.join(u64::MAX).unwrap();
        p.thread_exit(0);
        assert_eq!(p.create(), Err(EAGAIN));
        assert_eq!(p.generation(), u64::MAX >> 1);
        assert_eq!(p.state(WORKER), State::Vacant);
    }

    #[test]
    fn solo_expiry_and_late_irq_charge_one_actual_interval() {
        let mut p = Policy::new();
        let mut rr = RoundRobin::new(10).unwrap();
        rr.resume(MAIN, 100).unwrap();
        assert_eq!(rr.compare(&p, 100), None);
        assert_eq!(rr.pause(MAIN, 125), Ok(25));
        p.create().unwrap();
        assert_eq!(rr.logical_end(), Some(110));
        assert_eq!(rr.compare(&p, 125), Some(125));
        rr.resume(MAIN, 130).unwrap();
        assert_eq!(rr.pause(MAIN, 150), Ok(20));
        assert!(rr.timer(&mut p, 150));
        assert_eq!(p.current(), Some(WORKER));
        assert_eq!((rr.service(MAIN), rr.service(WORKER)), (45, 0));
        rr.resume(WORKER, 159).unwrap();
        assert_eq!(rr.compare(&p, 159), Some(169));
    }

    #[test]
    fn sleeper_wakes_before_rotation_and_unexpired_turn_survives_irq() {
        for target in [8, 10, 12] {
            let mut p = Policy::new();
            p.create().unwrap();
            p.yield_now();
            p.sleep(0, target).unwrap();
            let mut rr = RoundRobin::new(10).unwrap();
            rr.resume(MAIN, 0).unwrap();
            assert_eq!(rr.compare(&p, 0), Some(target));
            rr.pause(MAIN, target).unwrap();
            assert_eq!(rr.timer(&mut p, target), target >= 10);
            if target < 10 {
                rr.resume(MAIN, target + 1).unwrap();
                assert_eq!(rr.logical_end(), Some(10));
                assert_eq!(rr.compare(&p, target + 1), Some(10));
            } else {
                assert_eq!(p.current(), Some(WORKER));
                assert_eq!(p.ready(), &[MAIN]);
            }
        }
    }

    #[test]
    fn fresh_turn_and_accounting_failures_are_checked_before_mutation() {
        assert!(RoundRobin::new(0).is_err());
        assert!(RoundRobin::new(u64::MAX).is_err());
        let mut rr = RoundRobin::new(10).unwrap();
        assert_eq!(rr.resume(MAIN, u64::MAX - 9), Err(EOVERFLOW));
        assert_eq!(rr.logical_end(), None);
        rr.resume(MAIN, 10).unwrap();
        assert!(rr.pause(WORKER, 11).is_err());
        assert!(rr.pause(MAIN, 9).is_err());
        assert_eq!(rr.pause(MAIN, 15), Ok(5));
        assert!(rr.pause(MAIN, 15).is_err());
        assert!(rr.resume(WORKER, 16).is_err());
        rr.end_turn();
        assert!(rr.resume(MAIN, 14).is_err());
        rr.resume(MAIN, 18).unwrap();
        assert_eq!(rr.logical_end(), Some(28));
    }


    #[test]
    fn mutex_slots_reject_stale_copies_and_skip_exhausted_generations() {
        let mut p = Policy::new();
        p.locks[0].generation = (u64::MAX >> 2) - 1;
        let final_first = p.mutex(13, 8, 0).unwrap();
        for i in 1..4 {
            assert_eq!(p.mutex(13, 8 * (i + 1), 0), Ok(4 + i));
        }
        let prior = *p.mutex_slots();
        assert_eq!(p.mutex(13, 8, 0), Err(EBUSY));
        assert_eq!(p.mutex(13, 40, 0), Err(EAGAIN));
        assert_eq!(p.mutex(15, 16, final_first), Err(EINVAL));
        assert_eq!(p.mutex(15, 9, final_first), Err(EINVAL));
        assert_eq!(*p.mutex_slots(), prior);
        p.mutex(14, 8, final_first).unwrap();
        assert_eq!(p.mutex(13, 8, 0), Err(EAGAIN));
        p.mutex(14, 16, 5).unwrap();
        assert_eq!(p.mutex(13, 16, 0), Ok(9));
        assert_eq!(p.mutex(15, 16, 5), Err(EINVAL));
    }

    #[test]
    fn mutex_owner_sleep_and_direct_transfer_preserve_the_unlockers_turn() {
        let mut p = Policy::new();
        let t = p.mutex(13, 8, 0).unwrap();
        p.mutex(15, 8, t).unwrap();
        assert_eq!(p.mutex(15, 8, t), Err(EDEADLK));
        assert_eq!(p.mutex(16, 8, t), Err(EBUSY));
        p.create().unwrap();
        p.sleep(1, 12).unwrap();
        assert_eq!(p.mutex(17, 8, t), Err(EPERM));
        p.mutex(15, 8, t).unwrap();
        assert_eq!(p.current(), None);
        assert_eq!(p.next_compare(), Some(12));
        p.wake_due(12);
        assert_eq!(p.state(WORKER), State::MutexWait(t));
        assert_eq!(p.mutex(14, 8, t), Err(EBUSY));
        p.mutex(17, 8, t).unwrap();
        assert_eq!(p.current(), Some(MAIN));
        assert_eq!(p.ready(), &[WORKER]);
        assert_eq!(p.mutex(16, 8, t), Err(EBUSY));
        p.yield_now();
        p.mutex(17, 8, t).unwrap();
        p.mutex(14, 8, t).unwrap();
    }

    #[test]
    fn reused_worker_cannot_unlock_an_abandoned_mutex() {
        let mut p = Policy::new();
        let t = p.mutex(13, 8, 0).unwrap();
        let h = p.create().unwrap();
        p.join(h).unwrap();
        p.mutex(15, 8, t).unwrap();
        p.thread_exit(0);
        p.create().unwrap();
        p.yield_now();
        assert_eq!(p.mutex(17, 8, t), Err(EPERM));
        p.mutex(15, 8, t).unwrap();
        p.mutex(15, 8, t).unwrap();
        assert_eq!(p.current(), None);
        assert_eq!(p.next_compare(), None);
        assert_eq!(p.state(MAIN), State::MutexWait(t));
        assert_eq!(p.state(WORKER), State::MutexWait(t));
        p.process_exit(-3);
        assert_eq!(p.mutex_slots(), &[MutexSlot::EMPTY; 4]);
        assert_eq!(p.status(), Some(-3));
    }

}
