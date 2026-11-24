//! Optional data-only scheduler interface for one reserved task and a delegated
//! group. The caller supplies trusted time, observed progress, task state and
//! remaining allowances, serializes dispatch, and enforces the returned grant.

use super::group_policy::{DelegatedTask, GateDecision, Proposal, gate_decision};

pub(crate) const MAX_TASKS: usize = 32;
pub(crate) const MAX_OPS: usize = 16;
pub(crate) const MAX_FUEL: u8 = 16;

#[derive(Clone, Copy)]
pub(crate) enum Op {
    Jump(u8),
    IfReady { task: u8, yes: u8, no: u8 },
    Propose(Proposal),
    Yield,
}

#[derive(Clone, Copy)]
pub(crate) struct Reservation {
    pub(crate) task: usize,
    pub(crate) period: u64,
    pub(crate) offset: u64,
    pub(crate) ticks: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Reason {
    Accepted = 0,
    Rejected = 1,
    Fuel = 2,
    Stalled = 3,
    InvalidProgram = 4,
    InvalidObservation = 5,
    Yielded = 6,
    Reserved = 7,
    Absent = 8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Decision {
    pub(crate) reason: Reason,
    pub(crate) steps: u8,
    pub(crate) selected: Option<Proposal>,
}

#[derive(Clone, Copy)]
pub(crate) struct Extension<const N: usize> {
    reserved: Reservation,
    origin: u64,
    last_time: u64,
    progress: u64,
    last_change: u64,
    timeout: u64,
}

impl<const N: usize> Extension<N> {
    pub(crate) fn new(
        reserved: Reservation,
        start: u64,
        progress: u64,
        timeout: u64,
    ) -> Option<Self> {
        if N > MAX_TASKS
            || reserved.task >= N
            || timeout == 0
            || reserved.ticks == 0
            || reserved.offset >= reserved.period
            || reserved.ticks > reserved.period - reserved.offset
        {
            return None;
        }
        Some(Self {
            reserved,
            origin: start,
            last_time: start,
            progress,
            last_change: start,
            timeout,
        })
    }

    /// At an unchanged progress count, the first invocation at or beyond the
    /// timeout falls back. An observed increase starts a fresh interval. CPU
    /// reservations retain precedence over a stalled or regressing count.
    pub(crate) fn decide(
        &mut self,
        now: u64,
        progress: u64,
        tasks: &[DelegatedTask; N],
        eligible_cpus: u64,
        max_grant: u64,
        program: &[Op],
        fuel: u8,
    ) -> Decision {
        if now < self.last_time {
            // A stale clock cannot establish the current reservation window.
            return Decision {
                reason: Reason::InvalidObservation,
                steps: 0,
                selected: None,
            };
        }
        self.last_time = now;
        let regressed = progress < self.progress;
        if progress > self.progress {
            self.progress = progress;
            self.last_change = now;
        }
        let reservation = self.reserved;
        let phase = (now - self.origin) % reservation.period;
        let reserved =
            (reservation.offset..reservation.offset + reservation.ticks).contains(&phase);
        let mut scope = *tasks;
        for (id, task) in scope.iter_mut().enumerate() {
            task.ready &= (id == reservation.task) == reserved;
        }
        let until_boundary = if reserved {
            reservation.offset + reservation.ticks - phase
        } else if phase < reservation.offset {
            reservation.offset - phase
        } else {
            (reservation.period - phase) + reservation.offset
        };
        let cap = max_grant.min(until_boundary).min(u64::MAX - now);
        let (mut reason, steps, proposal) = if reserved {
            (Reason::Reserved, 0, None)
        } else if regressed {
            (Reason::InvalidObservation, 0, None)
        } else if now - self.last_change >= self.timeout {
            (Reason::Stalled, 0, None)
        } else {
            interpret(program, fuel, &scope)
        };
        let selected = match gate_decision(&scope, eligible_cpus, cap, proposal) {
            Some(GateDecision::Accepted(p)) => {
                reason = Reason::Accepted;
                Some(p)
            }
            Some(GateDecision::Fallback(p)) => Some(p),
            None => None,
        };
        Decision {
            reason,
            steps,
            selected,
        }
    }
}

/// The validation pass includes unreachable operations. Each fetched opcode
/// then costs exactly one unit, including a taken jump and a terminal opcode.
/// The VM has no memory writes, indirect calls or unmetered backward edges.
fn interpret<const N: usize>(
    program: &[Op],
    fuel: u8,
    tasks: &[DelegatedTask; N],
) -> (Reason, u8, Option<Proposal>) {
    if program.is_empty() {
        return (Reason::Absent, 0, None);
    }
    let valid_target = |target: u8| usize::from(target) < program.len();
    if program.len() > MAX_OPS
        || fuel > MAX_FUEL
        || program.iter().any(|op| match *op {
            Op::Jump(target) => !valid_target(target),
            Op::IfReady { task, yes, no } => {
                usize::from(task) >= N || !valid_target(yes) || !valid_target(no)
            }
            Op::Yield | Op::Propose(_) => false,
        })
    {
        return (Reason::InvalidProgram, 0, None);
    }
    let mut pc = 0;
    for steps in 1..=fuel {
        match program[pc] {
            Op::Jump(target) => pc = usize::from(target),
            Op::IfReady { task, yes, no } => {
                pc = usize::from(if tasks[usize::from(task)].ready {
                    yes
                } else {
                    no
                });
            }
            // A proposed decision becomes Accepted only through the trusted gate.
            Op::Propose(p) => return (Reason::Rejected, steps, Some(p)),
            Op::Yield => return (Reason::Yielded, steps, None),
        }
    }
    (Reason::Fuel, fuel, None)
}
