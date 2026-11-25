//! Exclusive kernel mutexes. The caller serializes operations and removes
//! blocked tasks from dispatch until handoff or timeout.
//!
//! A waiter's effective priority propagates through each Inherit wait edge.
//! Immediate locks boost their owner on acquisition and stop donation across
//! that edge. Their waiters keep the priorities obtained from their own held
//! locks. This per-resource rule also applies when protocols are nested.
//! Admission checks the declared user mask, nesting space and wait graph.

use super::domain_policy::{effective_priorities, LockPriority};

const MAX_HELD: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Protocol {
    Inherit,
    Immediate(u8),
}

#[derive(Clone, Copy)]
pub(crate) struct LockSpec {
    pub(crate) users: u32,
    pub(crate) protocol: Protocol,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Acquire {
    Held,
    Blocked,
}

pub(crate) struct LockSet<const T: usize, const L: usize> {
    base: [u8; T],
    spec: [LockSpec; L],
    owners: [Option<usize>; L],
    // Each row is a stack. Occupied slots precede all empty slots.
    held: [[Option<usize>; MAX_HELD]; T],
    blocked_on: [Option<usize>; T],
    // A single ordered prefix holds all waiters. Removing a waiter preserves
    // the relative arrival order of every remaining task, even across locks.
    arrivals: [u8; T],
    wait_count: usize,
}

impl<const T: usize, const L: usize> LockSet<T, L> {
    pub(crate) fn new(base: [u8; T], spec: [LockSpec; L]) -> Option<Self> {
        if T == 0 || T > u32::BITS as usize || L == 0 {
            return None;
        }
        let task_mask = u32::MAX >> (u32::BITS as usize - T);
        for lock in &spec {
            if lock.users == 0 || lock.users & !task_mask != 0 {
                return None;
            }
            if let Protocol::Immediate(ceiling) = lock.protocol {
                for (task, &priority) in base.iter().enumerate() {
                    if lock.users & (1u32 << task) != 0 && priority > ceiling {
                        return None;
                    }
                }
            }
        }
        Some(Self {
            base,
            spec,
            owners: [None; L],
            held: [[None; MAX_HELD]; T],
            blocked_on: [None; T],
            arrivals: [0; T],
            wait_count: 0,
        })
    }

    pub(crate) fn owner(&self, lock: usize) -> Option<usize> {
        self.owners.get(lock).copied().flatten()
    }

    pub(crate) fn waiting(&self, task: usize) -> Option<usize> {
        self.blocked_on.get(task).copied().flatten()
    }

    #[cfg(any(jnox_mutex_chain, jnox_mutex_survivors))]
    pub(crate) fn wait_order(&self) -> &[u8] {
        &self.arrivals[..self.wait_count]
    }

    fn priorities(&self) -> Option<[u8; T]> {
        let snapshot: [LockPriority; T] = core::array::from_fn(|task| {
            let held_ceiling = self.held[task]
                .iter()
                .flatten()
                .filter_map(|&lock| match self.spec[lock].protocol {
                    Protocol::Immediate(ceiling) => Some(ceiling),
                    Protocol::Inherit => None,
                })
                .max();
            let waits_for = self.blocked_on[task].and_then(|lock| match self.spec[lock].protocol {
                Protocol::Inherit => self.owners[lock].map(|owner| owner as u8),
                Protocol::Immediate(_) => None,
            });
            LockPriority {
                base: self.base[task],
                held_ceiling,
                waits_for,
            }
        });
        effective_priorities(&snapshot)
    }

    pub(crate) fn effective(&self, task: usize) -> Option<u8> {
        self.base.get(task)?;
        Some(self.priorities()?[task])
    }

    /// Follow every wait edge, including waits on immediate-ceiling locks.
    fn would_cycle(&self, task: usize, mut owner: usize) -> bool {
        for _ in 0..T {
            if owner == task {
                return true;
            }
            let Some(next) = self.blocked_on[owner].and_then(|lock| self.owners[lock]) else {
                return false;
            };
            owner = next;
        }
        true
    }

    pub(crate) fn acquire(&mut self, task: usize, lock: usize) -> Option<Acquire> {
        let slot = self.held.get(task)?.iter().position(Option::is_none)?;
        if self.spec.get(lock)?.users & (1u32 << task) == 0 || self.blocked_on[task].is_some() {
            return None;
        }
        if let Some(owner) = self.owners[lock] {
            if self.wait_count >= T || self.would_cycle(task, owner) {
                return None;
            }
            self.arrivals[self.wait_count] = task as u8;
            self.wait_count += 1;
            self.blocked_on[task] = Some(lock);
            Some(Acquire::Blocked)
        } else {
            self.held[task][slot] = Some(lock);
            self.owners[lock] = Some(task);
            Some(Acquire::Held)
        }
    }

    /// Only a runnable owner may release, in reverse acquisition order.
    /// Direct handoff ranks waiters by their effective priority before release,
    /// then by their original arrival. A waiting task reserved a nesting slot
    /// when it blocked and cannot acquire another lock until it resumes.
    pub(crate) fn release(&mut self, task: usize, lock: usize) -> Option<Option<usize>> {
        if self.owner(lock) != Some(task) || self.waiting(task).is_some() {
            return None;
        }
        let top = self.held[task].iter().rposition(Option::is_some)?;
        if self.held[task][top] != Some(lock) {
            return None;
        }
        let priorities = self.priorities()?;
        let mut first: Option<(usize, usize)> = None;
        for (arrival, &id) in self.arrivals[..self.wait_count].iter().enumerate() {
            let waiter = usize::from(id);
            if self.blocked_on[waiter] == Some(lock)
                && first.is_none_or(|(_, best)| priorities[waiter] > priorities[best])
            {
                first = Some((arrival, waiter));
            }
        }
        let successor = if let Some((arrival, waiter)) = first {
            let slot = self.held[waiter].iter().position(Option::is_none)?;
            self.held[waiter][slot] = Some(lock);
            self.remove_wait(arrival);
            Some(waiter)
        } else {
            None
        };
        self.held[task][top] = None;
        self.owners[lock] = successor;
        Some(successor)
    }

    fn remove_wait(&mut self, arrival: usize) {
        self.blocked_on[usize::from(self.arrivals[arrival])] = None;
        self.wait_count -= 1;
        for index in arrival..self.wait_count {
            self.arrivals[index] = self.arrivals[index + 1];
        }
    }

    pub(crate) fn timeout(&mut self, task: usize) -> bool {
        let Some(arrival) = self.arrivals[..self.wait_count]
            .iter()
            .position(|&id| usize::from(id) == task)
        else {
            return false;
        };
        self.remove_wait(arrival);
        true
    }
}
