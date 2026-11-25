//! Cooperative lifecycle for a single process with a main thread and one joinable worker.
use crate::ready_queue::ReadyQueue;

const MAIN: usize = 0;
const WORKER: usize = 1;
const ESRCH: i32 = 3;
const EAGAIN: i32 = 11;
const EDEADLK: i32 = 35;
const EOVERFLOW: i32 = 75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Vacant,
    Running,
    Ready,
    Sleeping(u64),
    Joining,
    Zombie(u64),
    Ended,
}

pub(crate) struct Policy {
    states: [State; 2],
    queue: ReadyQueue<2>,
    owner: Option<usize>,
    generation: u64,
    status: Option<i32>,
}

impl Policy {
    pub(crate) const fn new() -> Self {
        Self {
            states: [State::Running, State::Vacant],
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
}
