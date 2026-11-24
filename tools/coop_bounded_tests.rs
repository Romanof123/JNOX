//! rustc --edition=2024 --test tools/coop_bounded_tests.rs
#[path = "../kernel/src/event_loop.rs"]
mod event_loop;
#[path = "../kernel/src/coop_bounded.rs"]
mod coop_bounded;

use coop_bounded::{Decision::*, Handler, Post, Request, Service};

fn request(handler: usize, block: u64, deadline: u64) -> Request {
    Request { handler, block, deadline }
}
fn noop(_: &mut (), _: Request, _: &mut Post<'_>) {}

#[test]
fn refuse_without_mutation_and_admit_equality() {
    let mut service = Service::<(), 2, 2>::new([
        Handler { cost: 5, call: noop }, Handler { cost: 0, call: noop },
    ]);
    for (req, now, expected) in [
        (request(2, 1, 20), 10, Invalid), (request(1, 1, 20), 10, ZeroCost),
        (request(0, 1, u64::MAX), u64::MAX, Overflow), (request(0, 1, 14), 10, Deadline),
    ] {
        let r = service.post(req, now);
        assert_eq!((r.decision, r.before, r.after, service.peak()), (expected, (0,0), (0,0), 0));
    }
    assert_eq!(service.post(request(0,1,15), 10).decision, Accepted);
    assert_eq!(service.post(request(0,2,30), 9).decision, InvalidInterval);
    assert_eq!(service.post(request(0,2,30), 11).decision, Deadline); // earlier deadline
    assert_eq!(service.post(request(0,2,20), 10).decision, InvalidInterval);
    let mut service = Service::<(), 2, 1>::new([Handler { cost: 5, call: noop }]);
    assert_eq!(service.post(request(0,1,15), 10).decision, Accepted);
    assert_eq!(service.post(request(0,2,20), 10).decision, Accepted);
    assert_eq!(service.post(request(0,3,30), 10).decision, Full);
    assert_eq!(service.peak(), 2);
}

#[test]
fn nested_remaining_and_expired_interval() {
    fn nested(_: &mut (), _: Request, post: &mut Post<'_>) {
        let short = post(request(1,8,123), 103);
        assert_eq!((short.decision, short.remaining, short.before, short.after),
                   (Deadline, 7, (1,7), (1,7)));
        let equal = post(request(1,3,124), 103);
        assert_eq!((equal.decision, equal.after), (Accepted, (2,14)));
        assert_eq!(post(request(1,9,150), 102).decision, InvalidInterval);
        assert_eq!(post(request(1,9,150), 111).decision, InvalidInterval);
    }
    let mut service = Service::<(), 3, 2>::new([
        Handler { cost: 10, call: nested }, Handler { cost: 7, call: noop },
    ]);
    assert_eq!(service.post(request(0,1,150), 99).decision, Accepted);
    assert_eq!(service.post(request(1,2,150), 99).decision, Accepted);
    use std::sync::atomic::{AtomicU64, Ordering};
    static NOW: AtomicU64 = AtomicU64::new(100);
    fn clock() -> u64 { NOW.fetch_add(12, Ordering::Relaxed) }
    assert_eq!(service.turn(&mut (), clock).unwrap().0.block, 1);
    assert_eq!(service.post(request(1,4,500), 113).decision, InvalidInterval);
    for block in [2,3] { assert_eq!(service.turn(&mut (), clock).unwrap().0.block, block); }
    assert_eq!(service.post(request(1,4,7), 0).decision, Accepted);
}

#[test]
fn refused_samples_bound_later_posts_in_a_busy_epoch() {
    let mut service = Service::<(), 3, 2>::new([
        Handler { cost: 5, call: noop }, Handler { cost: 0, call: noop },
    ]);
    assert_eq!(service.post(request(0, 1, 100), 10).decision, Accepted);
    for (now, handler, deadline, expected) in [
        (20, 0, 29, Deadline), (15, 0, 100, InvalidInterval),
        (20, 0, 29, Deadline), (22, 3, 100, Invalid),
        (21, 0, 100, InvalidInterval), (23, 1, 100, ZeroCost),
        (22, 0, 100, InvalidInterval),
    ] {
        let receipt = service.post(request(handler, 2, deadline), now);
        assert_eq!((receipt.decision, receipt.before, receipt.after, service.peak()),
                   (expected, (1, 5), (1, 5), 1));
    }
    assert_eq!(service.post(request(0, 2, 33), 23).decision, Accepted);
}

#[test]
fn callback_boundaries_and_structural_refusals_observe_busy_epoch() {
    fn nested(observed: &mut Vec<(u64, coop_bounded::Decision)>, req: Request, post: &mut Post<'_>) {
        if req.block == 1 {
            observed.push((1, post(request(0, 9, 200), 34).decision));
        }
    }
    let mut service = Service::<_, 2, 1>::new([Handler { cost: 10, call: nested }]);
    for block in [1, 2] {
        assert_eq!(service.post(request(0, block, 200), 10).decision, Accepted);
    }
    assert_eq!(service.post(request(0, 9, 200), 32).decision, Full);
    let mut observed = Vec::new();
    service.turn(&mut observed, || 31).unwrap();
    assert_eq!(observed, [(1, InvalidInterval)]);
    assert_eq!(service.post(request(0, 9, 200), 40).decision, InvalidInterval);
    service.turn(&mut observed, || 50).unwrap();
    assert!(service.turn(&mut observed, || unreachable!()).is_none());
    assert_eq!(service.post(request(0, 3, 10), 0).decision, Accepted);
}

#[test]
fn callback_return_before_refused_nested_sample_invalidates_through_drain() {
    fn nested(_: &mut (), req: Request, post: &mut Post<'_>) {
        if req.block == 1 {
            assert_eq!(post(request(0, 9, 20), 27).decision, Deadline);
            assert_eq!(post(request(0, 9, 100), 26).decision, InvalidInterval);
        }
    }
    let mut service = Service::<_, 3, 1>::new([Handler { cost: 10, call: nested }]);
    for block in [1, 2] {
        assert_eq!(service.post(request(0, block, 100), 10).decision, Accepted);
    }
    service.turn(&mut (), || 25).unwrap();
    assert_eq!(service.post(request(0, 9, 100), 28).decision, InvalidInterval);
    service.turn(&mut (), || 30).unwrap();
    assert_eq!(service.post(request(0, 3, 10), 0).decision, Accepted);
}

#[test]
fn overflow_observation_prevents_wrap_to_zero_until_drain() {
    let mut service = Service::<(), 2, 1>::new([Handler { cost: 1, call: noop }]);
    assert_eq!(service.post(request(0, 1, u64::MAX), 1).decision, Accepted);
    assert_eq!(service.post(request(0, 9, u64::MAX), u64::MAX).decision, Overflow);
    assert_eq!(service.post(request(0, 9, 10), 0).decision, InvalidInterval);
    service.turn(&mut (), || u64::MAX).unwrap();
    assert_eq!(service.post(request(0, 2, 1), 0).decision, Accepted);
}
