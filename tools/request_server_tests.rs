//! Per-request service and accounting, compared with a calendar of individual CPU credits.
use super::aperiodic_policy::ServerKind::{self, Deferrable, Polling, Sporadic};
use super::request_server::{RequestError, RequestServer};
use std::collections::{BTreeMap, VecDeque};

const KINDS: [ServerKind; 3] = [Polling, Deferrable, Sporadic];

#[derive(Debug, PartialEq, Eq)]
struct State {
    now: u64,
    quota: u32,
    work: u32,
    flight: Option<(u64, u32)>,
    charged: u64,
    completed: u64,
    credits: Vec<(u64, u32)>,
}

fn state<const Q: usize, const R: usize>(server: &RequestServer<Q, R>) -> State {
    let s = server.snapshot();
    State {
        now: s.now,
        quota: s.remaining_quota,
        work: s.queued_work,
        flight: s.in_flight.map(|r| (r.tick(), r.id())),
        charged: s.charged.into(),
        completed: s.completed.into(),
        credits: s.replenishments.iter().map(|r| (r.due, r.amount)).collect(),
    }
}

// A cell per original budget unit. A dated cell is unavailable until that
// date's interval starts. Group the dated cells only when observing the server.
fn histogram(tokens: &[Option<u64>]) -> Vec<(u64, u32)> {
    let mut counts = BTreeMap::new();
    for &due in tokens.iter().flatten() {
        *counts.entry(due).or_default() += 1;
    }
    counts.into_iter().collect()
}

fn calendar<const R: usize>(
    kind: ServerKind,
    start: u64,
    arrivals: &[Option<(u32, u32)>],
    cpu: &[bool],
) -> Vec<Option<u32>> {
    assert_eq!(arrivals.len(), cpu.len());
    let mut server = RequestServer::<4, R>::new(kind, start, 2, 4, 1).unwrap();
    let mut tokens = [None; 2];
    let mut fifo: VecDeque<(u32, u32)> = VecDeque::new();
    let mut completed = 0;
    let mut trace = Vec::new();
    for (offset, (&arrival, &available)) in arrivals.iter().zip(cpu).enumerate() {
        let tick = start + offset as u64;
        if let Some((id, size)) = arrival {
            fifo.push_back((id, size));
            server.submit(id, size).unwrap();
        }
        for token in &mut tokens {
            if token.is_some_and(|due| due <= tick) {
                *token = None;
            }
        }
        let next_poll = start + (offset as u64 / 4 + 1) * 4;
        let mut expected = None;
        if let (Some(slot), Some(&(id, _)), true) = (
            tokens.iter().position(Option::is_none),
            fifo.front(),
            available,
        ) {
            expected = Some(id);
            if kind == Sporadic {
                let dates = histogram(&tokens);
                if dates.len() == R {
                    let latest = dates.last().unwrap().0;
                    for token in &mut tokens {
                        if *token == Some(latest) {
                            *token = Some(tick + 4);
                        }
                    }
                }
                tokens[slot] = Some(tick + 4);
            } else {
                tokens[slot] = Some(next_poll);
            }
        }
        let queued = fifo.iter().map(|&(_, size)| size).sum::<u32>();
        if kind == Polling && queued == u32::from(expected.is_some()) {
            tokens.fill(Some(next_poll));
        }
        let receipt = server.grant(available).unwrap();
        let flight = receipt.map(|r| (r.tick(), r.id()));
        assert_eq!(
            flight,
            expected.map(|id| (tick, id)),
            "slot {tick}, arrivals {arrivals:?}, CPU {cpu:?}"
        );
        let mut observed = State {
            now: tick + 1,
            quota: tokens.iter().filter(|t| t.is_none()).count() as u32,
            work: queued,
            flight,
            charged: completed + u64::from(expected.is_some()),
            completed,
            credits: if kind == Sporadic {
                histogram(&tokens)
            } else {
                Vec::new()
            },
        };
        assert_eq!(
            state(&server),
            observed,
            "grant at {tick}, arrivals {arrivals:?}, CPU {cpu:?}"
        );
        if let Some(receipt) = receipt {
            server.complete(receipt).unwrap();
            let head = fifo.front_mut().unwrap();
            head.1 -= 1;
            if head.1 == 0 {
                fifo.pop_front();
            }
            completed += 1;
            observed.completed = completed;
            observed.flight = None;
            observed.work -= 1;
            assert_eq!(state(&server), observed, "complete at {tick}");
        }
        for &(id, _) in arrivals.iter().flatten() {
            let balance = fifo.iter().find(|r| r.0 == id).map(|r| r.1);
            assert_eq!(
                server.remaining_work(id),
                balance,
                "request {id}, slot {tick}"
            );
        }
        trace.push(expected);
    }
    trace
}

#[test]
fn ten_interval_fifo_service_and_conservative_credit_merging() {
    let arrivals = [
        None,
        Some((1, 1)),
        None,
        Some((2, 3)),
        Some((3, 1)),
        None,
        None,
        Some((4, 1)),
        None,
        None,
    ];
    let cpu = [true; 10];
    // Zero denotes idle; all request IDs in this workload are positive.
    let observed = |trace: Vec<Option<u32>>| {
        trace
            .into_iter()
            .map(|id| id.unwrap_or(0))
            .collect::<Vec<_>>()
    };
    for (kind, explicit) in [
        (Polling, [0, 0, 0, 0, 1, 2, 0, 0, 2, 2]),
        (Deferrable, [0, 1, 0, 2, 2, 2, 0, 0, 3, 4]),
        (Sporadic, [0, 1, 0, 2, 0, 2, 0, 2, 0, 3]),
    ] {
        assert_eq!(observed(calendar::<2>(kind, 0, &arrivals, &cpu)), explicit);
        let merged = if kind == Sporadic {
            [0, 1, 0, 2, 0, 0, 0, 2, 2, 0]
        } else {
            explicit
        };
        assert_eq!(observed(calendar::<1>(kind, 0, &arrivals, &cpu)), merged);
    }
}

#[test]
fn small_arrival_and_cpu_availability_masks_match_individual_credits() {
    for kind in KINDS {
        for releases in 0u8..16 {
            for available in 0u8..64 {
                let mut arrivals = [None; 9];
                for (i, request) in [(9, 1), (0, 2), (6, 3), (2, 1)].into_iter().enumerate() {
                    if releases & (1 << i) != 0 {
                        arrivals[i] = Some(request);
                    }
                }
                let cpu: [bool; 9] = std::array::from_fn(|i| i >= 6 || available & (1 << i) != 0);
                calendar::<1>(kind, 7, &arrivals, &cpu);
                calendar::<2>(kind, 7, &arrivals, &cpu);
            }
        }
    }
}

#[test]
fn full_sporadic_queue_keeps_the_oldest_credit_at_its_original_due_tick() {
    let mut server = RequestServer::<1, 2>::new(Sporadic, 0, 3, 5, 9).unwrap();
    server.submit(5, 4).unwrap();
    for available in [true, false, true, true] {
        let grant = server.grant(available).unwrap();
        assert_eq!(grant.is_some(), available);
        if let Some(receipt) = grant {
            server.complete(receipt).unwrap();
        }
    }
    let before = state(&server);
    assert_eq!(before.credits, [(5, 1), (8, 2)]);
    assert_eq!(before.quota, 0);
    for expected in [0, 1, 1, 1, 3] {
        // CPU unavailable at ticks 4..=8
        assert!(server.grant(false).unwrap().is_none());
        assert_eq!(state(&server).quota, expected);
    }
    assert!(state(&server).credits.is_empty());
    assert_eq!(server.remaining_work(5), Some(1));
}

#[test]
fn submitting_during_a_grant_preserves_identity_and_service_order() {
    use RequestError::*;
    for kind in KINDS {
        let mut server = RequestServer::<2, 2>::new(kind, 3, 2, 2, 1).unwrap();
        server.submit(u32::MAX, 2).unwrap();
        let first = server.grant(true).unwrap().unwrap();
        let before = state(&server);
        assert_eq!(server.remaining_work(u32::MAX), Some(2));
        assert!(matches!(server.submit(u32::MAX, 1), Err(DuplicateId)));
        assert!(matches!(server.submit(0, 0), Err(EmptyRequest)));
        assert!(matches!(server.grant(false), Err(CompletionPending)));
        assert_eq!(state(&server), before);
        server.submit(0, 1).unwrap();
        let after_submit = state(&server);
        assert_eq!(after_submit.work, 3);
        assert_eq!(after_submit.flight, Some((3, u32::MAX)));
        assert!(matches!(server.submit(1, 1), Err(QueueFull)));
        assert_eq!(state(&server), after_submit);
        server.complete(first).unwrap();
        let between = state(&server);
        assert!(matches!(server.complete(first), Err(NoPendingCompletion)));
        assert_eq!(state(&server), between);
        let second = server.grant(true).unwrap().unwrap();
        assert_eq!(second.id(), u32::MAX);
        let before = state(&server);
        assert!(matches!(server.complete(first), Err(WrongReceipt)));
        assert_eq!(state(&server), before);
        server.complete(second).unwrap();
        assert_eq!(server.remaining_work(u32::MAX), None);
        server.submit(u32::MAX, 1).unwrap();
        for id in [0, u32::MAX] {
            let next = server.grant(true).unwrap().unwrap();
            assert_eq!(next.id(), id);
            if id == u32::MAX {
                assert!(matches!(server.complete(first), Err(WrongReceipt)));
            }
            server.complete(next).unwrap();
        }
        assert_eq!(state(&server).completed, 4);
    }
}

#[test]
fn foreign_receipts_and_receipts_from_before_restart_cannot_complete_matching_ids() {
    use RequestError::*;
    let mut a = RequestServer::<1, 0>::new(Deferrable, 9, 1, 1, 41).unwrap();
    let mut b = RequestServer::<1, 0>::new(Deferrable, 9, 1, 1, 42).unwrap();
    a.submit(7, 1).unwrap();
    b.submit(7, 1).unwrap();
    let ra = a.grant(true).unwrap().unwrap();
    let rb = b.grant(true).unwrap().unwrap();
    assert_eq!((ra.tick(), ra.id()), (rb.tick(), rb.id()));
    assert!(ra != rb);
    let before = state(&b);
    assert!(matches!(b.complete(ra), Err(WrongReceipt)));
    assert_eq!(state(&b), before);
    a.complete(ra).unwrap();
    b.complete(rb).unwrap();
    let mut restarted = RequestServer::<1, 0>::new(Deferrable, 9, 1, 1, 43).unwrap();
    restarted.submit(7, 1).unwrap();
    let fresh = restarted.grant(true).unwrap().unwrap();
    assert!(matches!(restarted.complete(ra), Err(WrongReceipt)));
    assert_eq!(restarted.remaining_work(7), Some(1));
    restarted.complete(fresh).unwrap();
}

#[test]
fn queue_and_aggregate_work_limits_are_atomic_even_with_an_outstanding_receipt() {
    use RequestError::*;
    for kind in KINDS {
        let mut server = RequestServer::<2, 1>::new(kind, 0, u32::MAX, u32::MAX, 1).unwrap();
        server.submit(0, u32::MAX).unwrap();
        let grant = server.grant(true).unwrap().unwrap();
        let before = state(&server);
        assert!(matches!(server.submit(2, 1), Err(WorkOverflow)));
        assert_eq!(state(&server), before);
        server.complete(grant).unwrap();
        server.submit(1, 1).unwrap();
        let full = state(&server);
        assert_eq!(full.work, u32::MAX);
        assert!(matches!(server.submit(2, 1), Err(QueueFull)));
        assert_eq!(state(&server), full);
        assert_eq!(server.remaining_work(0), Some(u32::MAX - 1));
        assert_eq!(server.remaining_work(1), Some(1));
    }
}

#[test]
fn capacity_validation_and_clock_overflows_leave_the_previous_state_available() {
    use RequestError::*;
    for kind in KINDS {
        assert!(RequestServer::<0, 1>::new(kind, 0, 1, 1, 1).is_none());
        assert!(RequestServer::<1, 1>::new(kind, 0, 0, 3, 1).is_none());
        assert!(RequestServer::<1, 1>::new(kind, 0, 2, 1, 1).is_none());
        assert_eq!(
            RequestServer::<1, 0>::new(kind, 0, 1, 1, 1).is_some(),
            kind != Sporadic
        );
        let mut server = RequestServer::<1, 1>::new(kind, u64::MAX, 1, 1, 1).unwrap();
        server.submit(0, 1).unwrap();
        let before = state(&server);
        for available in [false, true] {
            assert!(matches!(server.grant(available), Err(TimeOverflow)));
            assert_eq!(state(&server), before);
        }
    }
    let max = u64::MAX;
    let mut server = RequestServer::<1, 2>::new(Sporadic, max - 4, 2, 4, 1).unwrap();
    server.submit(4, 3).unwrap();
    let receipt = server.grant(true).unwrap().unwrap();
    server.complete(receipt).unwrap();
    let before = state(&server);
    assert_eq!(before.credits, [(max, 1)]);
    assert!(matches!(server.grant(true), Err(TimeOverflow))); // new repayment would wrap
    assert_eq!(state(&server), before);
    for _ in 0..3 {
        assert!(server.grant(false).unwrap().is_none());
    }
    let last = state(&server);
    assert_eq!(last.now, max);
    assert!(matches!(server.grant(true), Err(TimeOverflow)));
    assert_eq!(state(&server), last);
}
