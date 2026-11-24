//! Single-owner, finite FIFO admission on a monotonic, nonwrapping counter interval.
use crate::event_loop::EventLoop;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub handler: usize,
    pub block: u64,
    pub deadline: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Decision {
    Accepted,
    Invalid,
    Full,
    Deadline,
    ZeroCost,
    Overflow,
    InvalidInterval,
}

pub struct Receipt {
    pub decision: Decision,
    pub before: (usize, u64),
    pub after: (usize, u64),
    pub remaining: u64,
}

pub type Post<'a> = dyn FnMut(Request, u64) -> Receipt + 'a;
pub struct Handler<C> {
    pub cost: u64,
    pub call: fn(&mut C, Request, &mut Post<'_>),
}

pub struct Service<C, const N: usize, const H: usize> {
    handlers: [Handler<C>; H],
    fifo: EventLoop<Request, N>,
    queued: u64,
    peak: usize,
    watermark: Option<u64>,
    running_until: Option<u64>,
    invalid: bool,
}

impl<C, const N: usize, const H: usize> Service<C, N, H> {
    pub fn new(handlers: [Handler<C>; H]) -> Self {
        Self {
            handlers,
            fifo: EventLoop::new(),
            queued: 0,
            peak: 0,
            watermark: None,
            running_until: None,
            invalid: false,
        }
    }

    pub fn peak(&self) -> usize {
        self.peak
    }

    /// Observe an active burst without changing FIFO accounting.
    fn observe(&mut self, now: u64) -> bool {
        let previous = self.watermark.unwrap_or(now);
        self.watermark = Some(previous.max(now));
        now >= previous
    }

    fn admit(&mut self, req: Request, now: u64) -> Result<(), Decision> {
        // A refused first request cannot open a burst. Structural refusals in an
        // existing burst still participate in the single counter timeline.
        let monotonic = self.watermark.is_none() || self.observe(now);
        let Some(handler) = self.handlers.get(req.handler) else {
            return Err(Decision::Invalid);
        };
        if self.fifo.depth() == N {
            return Err(Decision::Full);
        }
        if handler.cost == 0 {
            return Err(Decision::ZeroCost);
        }
        if self.invalid
            || !monotonic
            || self.running_until.is_some_and(|until| now > until)
        {
            return Err(Decision::InvalidInterval);
        }
        let mut finish = self.running_until.unwrap_or(now);
        for entry in self.fifo.iter().chain(core::iter::once(&req)) {
            finish = finish
                .checked_add(self.handlers[entry.handler].cost)
                .ok_or(Decision::Overflow)?;
            if finish > entry.deadline {
                return Err(Decision::Deadline);
            }
        }
        let queued = self
            .queued
            .checked_add(handler.cost)
            .ok_or(Decision::Overflow)?;
        self.fifo.enqueue(req).map_err(|_| Decision::Full)?;
        self.queued = queued;
        self.peak = self.peak.max(self.fifo.depth());
        self.watermark = Some(now);
        Ok(())
    }

    pub fn post(&mut self, req: Request, now: u64) -> Receipt {
        let before = (self.fifo.depth(), self.queued);
        let remaining = self
            .running_until
            .map_or(0, |until| until.saturating_sub(now));
        let decision = self.admit(req, now).err().unwrap_or(Decision::Accepted);
        Receipt {
            decision,
            before,
            after: (self.fifo.depth(), self.queued),
            remaining,
        }
    }

    pub fn turn(&mut self, context: &mut C, clock: fn() -> u64) -> Option<(Request, u64, u64)> {
        let req = self.fifo.pop()?;
        let Handler { cost, call } = self.handlers[req.handler];
        self.queued -= cost;
        let start = clock();
        self.running_until = start.checked_add(cost);
        let monotonic = self.observe(start);
        self.invalid |= !monotonic
            || self.running_until.is_none_or(|until| until > req.deadline);
        call(context, req, &mut |next, now| self.post(next, now));
        let finish = clock();
        let monotonic = self.observe(finish);
        self.invalid |= !monotonic
            || self.running_until.is_none_or(|until| finish > until);
        self.running_until = None;
        if self.fifo.depth() == 0 {
            self.invalid = false;
            self.watermark = None;
        }
        Some((req, start, finish))
    }
}
