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
