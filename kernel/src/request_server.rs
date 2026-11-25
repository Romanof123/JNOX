//! Bounded FIFO for aperiodic requests. A successful grant charges a server
//! tick and issues a receipt to the selected request. Completion retires that
//! unit from the FIFO. The caller serializes submissions, grants and completions.

use super::aperiodic_policy::{AperiodicServer, Replenishment, ServerError, ServerKind};

/// Identifies one reserved [tick, tick+1) interval for a particular server
/// instance and request. Copies acknowledge the same reservation exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Receipt {
    instance: u64,
    tick: u64,
    id: u32,
}

impl Receipt {
    pub(crate) fn tick(self) -> u64 {
        self.tick
    }

    pub(crate) fn id(self) -> u32 {
        self.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestError {
    EmptyRequest,
    DuplicateId,
    QueueFull,
    WorkOverflow,
    TimeOverflow,
    CompletionPending,
    NoPendingCompletion,
    WrongReceipt,
    TimeRegression,
}

impl From<ServerError> for RequestError {
    fn from(value: ServerError) -> Self {
        match value {
            ServerError::EmptyRelease => Self::EmptyRequest,
            ServerError::BacklogOverflow => Self::WorkOverflow,
            ServerError::TimeOverflow => Self::TimeOverflow,
            ServerError::TimeRegression => Self::TimeRegression,
        }
    }
}

/// State after the last operation. The next grant processes replenishments
/// due at `now` before it dispatches. `queued_work` includes any unit awaiting
/// completion. `charged` and `completed` count service units since creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ServerSnapshot<'a> {
    pub(crate) now: u64,
    pub(crate) remaining_quota: u32,
    pub(crate) queued_work: u32,
    pub(crate) charged: u64,
    pub(crate) completed: u64,
    pub(crate) in_flight: Option<Receipt>,
    pub(crate) replenishments: &'a [Replenishment],
}

#[derive(Clone, Copy)]
struct Request {
    id: u32,
    remaining: u32,
}

/// Q bounds distinct resident requests, including the in-flight head request.
/// R bounds the underlying sporadic server's replenishment queue.
pub(crate) struct RequestServer<const Q: usize, const R: usize> {
    instance: u64,
    server: AperiodicServer<R>,
    requests: [Request; Q],
    head: usize,
    len: usize,
    in_flight: Option<Receipt>,
    charged: u64,
}

impl<const Q: usize, const R: usize> RequestServer<Q, R> {
    /// Assign a distinct instance key to each server or restart whose receipts
    /// may coexist. Zero is valid. Absolute tick and ID distinguish successive
    /// grants within one instance, including grants after a request ID is reused.
    pub(crate) fn new(
        kind: ServerKind,
        start: u64,
        quota: u32,
        period: u32,
        instance: u64,
    ) -> Option<Self> {
        if Q == 0 {
            return None;
        }
        Some(Self {
            instance,
            server: AperiodicServer::new(kind, start, quota, period)?,
            requests: [Request {
                id: 0,
                remaining: 0,
            }; Q],
            head: 0,
            len: 0,
            in_flight: None,
            charged: 0,
        })
    }

    fn queued_work(&self) -> u32 {
        self.server.unreserved_work() + u32::from(self.in_flight.is_some())
    }

    /// The FIFO retains a request until every granted unit has been completed.
    pub(crate) fn remaining_work(&self, id: u32) -> Option<u32> {
        self.requests
            .iter()
            .find(|request| request.remaining > 0 && request.id == id)
            .map(|request| request.remaining)
    }

    pub(crate) fn snapshot(&self) -> ServerSnapshot<'_> {
        ServerSnapshot {
            now: self.server.now(),
            remaining_quota: self.server.remaining_quota(),
            queued_work: self.queued_work(),
            charged: self.charged,
            completed: self.charged - u64::from(self.in_flight.is_some()),
            in_flight: self.in_flight,
            replenishments: self.server.pending_replenishments(),
        }
    }

    /// Enqueue at the current boundary. A pending receipt still occupies its
    /// request slot and counts toward the u32 aggregate work limit.
    pub(crate) fn submit(&mut self, id: u32, work: u32) -> Result<(), RequestError> {
        if work == 0 {
            return Err(RequestError::EmptyRequest);
        }
        if self.remaining_work(id).is_some() {
            return Err(RequestError::DuplicateId);
        }
        if self.len == Q {
            return Err(RequestError::QueueFull);
        }
        self.queued_work()
            .checked_add(work)
            .ok_or(RequestError::WorkOverflow)?;
        self.server.release(work)?;
        let tail = (self.head + self.len) % Q;
        self.requests[tail] = Request {
            id,
            remaining: work,
        };
        self.len += 1;
        Ok(())
    }

    /// Apply application admission bounds before mutating the shared ledger.
    #[cfg(jnox_ap)]
    pub(crate) fn submit_bounded(&mut self, id: u32, work: u32) -> Result<(), RequestError> {
        if work > 4
            || self
                .queued_work()
                .checked_add(work)
                .filter(|n| *n <= 6)
                .is_none()
        {
            return Err(RequestError::WorkOverflow);
        }
        self.submit(id, work)
    }

    /// Reserve at most one service unit and advance one clock tick. Outstanding
    /// completion and clock errors leave the clock, budget and FIFO intact.
    pub(crate) fn grant(&mut self, cpu_available: bool) -> Result<Option<Receipt>, RequestError> {
        if self.in_flight.is_some() {
            return Err(RequestError::CompletionPending);
        }
        let tick = self.server.now();
        if !self.server.tick(cpu_available)? {
            return Ok(None);
        }
        let receipt = Receipt {
            instance: self.instance,
            tick,
            id: self.requests[self.head].id,
        };
        // Each charge required one successful advance of the u64 clock.
        self.charged += 1;
        self.in_flight = Some(receipt);
        Ok(Some(receipt))
    }

    /// Acknowledge the issued receipt after its request performed one unit.
    /// A request that still has work keeps its place at the head of the FIFO.
    pub(crate) fn complete(&mut self, receipt: Receipt) -> Result<(), RequestError> {
        let pending = self.in_flight.ok_or(RequestError::NoPendingCompletion)?;
        if receipt != pending {
            return Err(RequestError::WrongReceipt);
        }
        let request = &mut self.requests[self.head];
        request.remaining -= 1;
        if request.remaining == 0 {
            self.head = (self.head + 1) % Q;
            self.len -= 1;
        }
        self.in_flight = None;
        Ok(())
    }
}
