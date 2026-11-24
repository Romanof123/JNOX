//! Single-owner linked Rust event service. Handlers receive only a posting capability.
use crate::event_loop::EventLoop;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub handler: usize,
    pub value: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    InvalidHandler(Event),
    Full(Event),
}

pub type Post<'a> = dyn FnMut(Event) -> Result<(), Reject> + 'a;
pub struct Handler<C> {
    pub call: fn(&mut C, Event, &mut Post<'_>),
    pub budget: u64,
}

#[derive(Clone, Copy, Default)]
pub struct Stats {
    pub calls: u64,
    pub total: u64,
    pub maximum: u64,
    pub overruns: u64,
}

pub struct Completion {
    pub event: Event,
    pub elapsed: u64,
    pub overrun: bool,
}

pub struct Service<C, const N: usize, const H: usize> {
    fifo: EventLoop<Event, N>,
    handlers: [Handler<C>; H],
    stats: [Stats; H],
    peak: usize,
}

impl<C, const N: usize, const H: usize> Service<C, N, H> {
    pub fn new(handlers: [Handler<C>; H]) -> Self {
        Self {
            fifo: EventLoop::new(),
            handlers,
            stats: [Stats::default(); H],
            peak: 0,
        }
    }

    pub fn enqueue(&mut self, event: Event) -> Result<(), Reject> {
        if event.handler >= H {
            return Err(Reject::InvalidHandler(event));
        }
        self.fifo.enqueue(event).map_err(Reject::Full)?;
        self.peak = self.peak.max(self.fifo.depth());
        Ok(())
    }

    pub fn peak(&self) -> usize {
        self.peak
    }
    pub fn stats(&self) -> &[Stats; H] {
        &self.stats
    }

    pub fn turn(&mut self, context: &mut C, clock: fn() -> u64) -> Option<Completion> {
        let mut result = None;
        let mut depth = self.fifo.depth().saturating_sub(1);
        self.fifo.dispatch_one(|event, append| {
            let handler = &self.handlers[event.handler];
            let mut post = |next: Event| {
                if next.handler >= H {
                    return Err(Reject::InvalidHandler(next));
                }
                append(next).map_err(Reject::Full)?;
                depth += 1;
                self.peak = self.peak.max(depth);
                Ok(())
            };
            let start = clock();
            (handler.call)(context, event, &mut post);
            let elapsed = clock().wrapping_sub(start);
            let overrun = elapsed > handler.budget;
            let stats = &mut self.stats[event.handler];
            stats.calls = stats.calls.saturating_add(1);
            stats.total = stats.total.saturating_add(elapsed);
            stats.maximum = stats.maximum.max(elapsed);
            stats.overruns = stats.overruns.saturating_add(u64::from(overrun));
            result = Some(Completion {
                event,
                elapsed,
                overrun,
            });
        });
        result
    }
}
