//! rustc --edition=2024 --test tools/coop_service_tests.rs
#[path = "../kernel/src/event_loop.rs"]
mod event_loop;
#[path = "../kernel/src/coop_service.rs"]
mod coop_service;

use coop_service::{Event, Handler, Post, Reject, Service};
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn handler_posts_to_capacity_and_budget_equality_is_accepted() {
    static CLOCK: AtomicU64 = AtomicU64::new(u64::MAX);
    fn clock() -> u64 { CLOCK.fetch_add(1, Ordering::Relaxed) }
    fn handler(values: &mut Vec<u64>, event: Event, post: &mut Post<'_>) {
        values.push(event.value);
        if event.value == 0 {
            for value in 1..=3 { post(Event { handler: 0, value }).unwrap(); }
            let extra = Event { handler: 0, value: 4 };
            assert_eq!(post(extra), Err(Reject::Full(extra)));
            let invalid = Event { handler: 1, value: 9 };
            assert_eq!(post(invalid), Err(Reject::InvalidHandler(invalid)));
        }
    }
    let mut service = Service::<_, 3, 1>::new([Handler { call: handler, budget: 1 }]);
    let mut values = Vec::new();
    service.enqueue(Event { handler: 0, value: 0 }).unwrap();
    for value in 0..4 {
        let done = service.turn(&mut values, clock).unwrap();
        assert_eq!((done.event.value, done.elapsed, done.overrun), (value, 1, false));
    }
    assert!(service.turn(&mut values, clock).is_none());
    assert_eq!(values, [0, 1, 2, 3]);
    assert_eq!(service.peak(), 3);
    let stats = service.stats()[0];
    assert_eq!((stats.calls, stats.total, stats.maximum, stats.overruns), (4, 4, 1, 0));
}

#[test]
fn zero_slots_and_zero_registrations_are_safe() {
    let event = Event { handler: 0, value: 9 };
    let mut no_slots = Service::<(), 0, 1>::new([Handler { call: |_, _, _| {}, budget: 0 }]);
    assert_eq!(no_slots.enqueue(event), Err(Reject::Full(event)));
    let mut no_handlers = Service::<(), 1, 0>::new([]);
    assert_eq!(no_handlers.enqueue(event), Err(Reject::InvalidHandler(event)));
    assert!(no_handlers.turn(&mut (), || unreachable!()).is_none());
}
