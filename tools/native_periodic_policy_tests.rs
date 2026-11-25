use super::dispatch_policy::DispatchPolicy::{DeadlineMonotonic as Dm, RateMonotonic as Rm};
use super::native_periodic_policy::{Action::*, Calendar, Error};

const HZ: u64 = 1_000_000_000;

#[test]
fn same_tasks_have_different_initial_owners_and_preserve_their_phase() {
    let config = [[0, 10, 9], [0, 12, 4]];
    for (policy, first) in [(Rm, 0), (Dm, 1)] {
        let mut c = Calendar::new(policy, config, HZ, 100).unwrap();
        assert_eq!(c.step(99, Poll), Ok(None));
        assert_eq!(c.next_compare(), Some(100));
        assert_eq!(c.step(100, Poll), Ok(Some(first)));
        assert_eq!(c.step(101, WaitNext), Ok(Some(1 - first)));
        assert_eq!(c.step(102, WaitNext), Ok(None));
        assert_eq!(c.next_compare(), Some(110));
        assert_eq!(c.step(110, Poll), Ok(Some(0)));
        assert_eq!(c.step(112, Poll), Ok(Some(first)));
    }
}

#[test]
fn a_delayed_batch_matches_individual_events_without_losing_pending_work() {
    let config = [[0, 5, 3]];
    let mut single = Calendar::new(Rm, config, HZ, 0).unwrap();
    let mut batch = Calendar::new(Rm, config, HZ, 0).unwrap();
    for c in [&mut single, &mut batch] { c.step(0, Poll).unwrap(); }
    for t in 1..=51 { single.step(t, Poll).unwrap(); }
    batch.step(51, Poll).unwrap();
    assert_eq!(single.slot(0), batch.slot(0));
    let s = batch.slot(0).unwrap();
    assert_eq!((s.generation, s.released, s.overlaps, s.missed), (0, 11, 10, 10));
    assert_eq!(batch.step(51, WaitNext), Ok(Some(0)));
    assert_eq!(batch.step(51, Exit), Ok(None));
    let s = batch.slot(0).unwrap();
    assert_eq!((s.cancelled, s.generation, s.missed), (9, 2, 10));
    assert_eq!(batch.next_compare(), None);
}

#[test]
fn exact_deadline_passes_and_late_completion_reports_the_miss_once() {
    for (t, missed, overlaps) in [(10, 0, 0), (11, 1, 1)] {
        let mut c = Calendar::new(Rm, [[0, 10, 10]], HZ, 0).unwrap();
        c.step(0, Poll).unwrap();
        assert_eq!(c.step(t, WaitNext), Ok(Some(0)));
        assert_eq!(c.slot(0).unwrap().missed, missed);
        assert_eq!(c.slot(0).unwrap().overlaps, overlaps);
        let state = c.slot(0);
        c.step(t, Poll).unwrap();
        assert_eq!(c.slot(0), state);
    }
}

#[test]
fn equal_rank_yield_and_sleep_preserve_the_generation() {
    let mut c = Calendar::new(Rm, [[0, 10, 9]; 2], HZ, 0).unwrap();
    assert_eq!(c.step(0, Poll), Ok(Some(0)));
    assert_eq!(c.step(1, Yield), Ok(Some(1)));
    assert_eq!(c.step(2, Sleep(15)), Ok(Some(0)));
    assert_eq!(c.step(10, Poll), Ok(Some(0)));
    assert_eq!(c.step(15, Poll), Ok(Some(0)));
    assert_eq!(c.step(15, Yield), Ok(Some(1)));
    let slot = c.slot(1).unwrap();
    assert_eq!((slot.generation, slot.released, slot.missed), (0, 2, 1));
}

#[test]
fn refused_steps_preserve_owner_calendar_and_compare() {
    assert!(matches!(Calendar::new(Rm, [[0, 11, 1]], 1, 0), Err(Error::Conversion)));
    let mut c = Calendar::new(Rm, [[0, 4, 2]], HZ, u64::MAX - 5).unwrap();
    assert_eq!(c.step(0, Yield), Err(Error::NoOwner));
    let before = (c.owner(), c.slot(0), c.next_compare());
    assert_eq!(c.step(u64::MAX - 5, Poll), Err(Error::Overflow));
    assert_eq!((c.owner(), c.slot(0), c.next_compare()), before);
}

#[test]
fn same_counter_completion_of_an_older_generation_precedes_its_successor() {
    for action in [WaitNext, Exit] {
        for at in [10, 30] {
            let mut direct = Calendar::new(Rm, [[0, 10, 10]], HZ, 0).unwrap();
            let mut polled = Calendar::new(Rm, [[0, 10, 10]], HZ, 0).unwrap();
            direct.step(0, Poll).unwrap();
            polled.step(0, Poll).unwrap();
            for _ in 0..3 { polled.step(at, Poll).unwrap(); }
            assert_eq!(direct.step(at, action), polled.step(at, action));
            let slot = direct.slot(0).unwrap();
            assert_eq!(Some(slot), polled.slot(0));
            assert_eq!(direct.next_compare(), polled.next_compare());
            let prior_backlog = at / 10 - 1;
            assert_eq!(slot.missed, prior_backlog);
            assert_eq!(slot.cancelled, if action == Exit { prior_backlog } else { 0 });
            let new_overlap = action == WaitNext && prior_backlog > 0;
            assert_eq!(slot.overlaps, prior_backlog + u64::from(new_overlap));
        }
    }
}

#[test]
fn several_owners_may_commit_at_the_same_tick_without_erasing_each_others_release() {
    let mut c = Calendar::new(Rm, [[0, 10, 10]; 2], HZ, 0).unwrap();
    assert_eq!(c.step(0, Poll), Ok(Some(0)));
    assert_eq!(c.step(20, Poll), Ok(Some(0)));
    let peer = c.slot(1);
    assert_eq!(c.step(20, Exit), Ok(Some(1)));
    assert_eq!(c.slot(1), peer);
    let exited = c.slot(0).unwrap();
    assert_eq!((exited.generation, exited.released, exited.cancelled, exited.overlaps), (1, 2, 1, 1));
    assert_eq!(c.step(20, WaitNext), Ok(Some(1)));
    assert_eq!(c.slot(1).unwrap().overlaps, 2); // generation 1 still preceded release 2
    assert_eq!(c.step(20, WaitNext), Ok(Some(1)));
    assert_eq!(c.slot(1).unwrap().overlaps, 1); // generation 1 also completed at release 2
    assert_eq!(c.step(20, Exit), Ok(None));     // generation 2 actually ran
    let last = c.slot(1).unwrap();
    assert_eq!((last.generation, last.released, last.cancelled, last.missed), (3, 3, 0, 1));
    assert_eq!(c.slot(0), Some(exited));
}

#[test]
fn refused_future_arithmetic_does_not_consume_a_same_counter_correction() {
    let epoch = u64::MAX - 30;
    let mut c = Calendar::new(Rm, [[0, 10, 10]], HZ, epoch).unwrap();
    c.step(epoch, Poll).unwrap();
    c.step(epoch + 10, Poll).unwrap();
    let before = (c.slot(0), c.owner(), c.next_compare());
    // Generation 2 fits, but the subsequent deadline cannot be represented.
    assert_eq!(c.step(epoch + 20, WaitNext), Err(Error::Overflow));
    assert_eq!((c.slot(0), c.owner(), c.next_compare()), before);
    assert_eq!(c.step(epoch + 10, Exit), Ok(None));
    let s = c.slot(0).unwrap();
    assert_eq!((s.generation, s.released, s.overlaps, s.cancelled), (1, 1, 0, 0));
}

#[test]
fn poll_at_release_preserves_the_slot_order_tie_with_a_returning_owner() {
    for first in [0, 1] {
        let mut tuples = [[10, 10, 10]; 2];
        tuples[first][0] = 0;
        for repeats in 0..=2 {
            let mut c = Calendar::new(Rm, tuples, HZ, 0).unwrap();
            assert_eq!(c.step(0, Poll), Ok(Some(first)));
            for _ in 0..repeats { assert_eq!(c.step(10, Poll), Ok(Some(first))); }
            assert_eq!(c.step(10, WaitNext), Ok(Some(0)));
            assert_eq!(c.step(10, Yield), Ok(Some(1)));
        }
    }
}

#[test]
fn same_counter_commit_after_real_preemption_belongs_to_the_new_owner() {
    let mut c = Calendar::new(Rm, [[0, 10, 10], [10, 5, 5]], HZ, 0).unwrap();
    assert_eq!(c.step(0, Poll), Ok(Some(0)));
    assert_eq!(c.step(10, Poll), Ok(Some(1)));
    assert_eq!(c.step(10, Exit), Ok(Some(0)));
    let short = c.slot(1).unwrap();
    assert_eq!((short.generation, short.released, short.cancelled), (1, 1, 0));
    assert_eq!(c.slot(0).unwrap().overlaps, 1);
    assert_eq!(c.step(10, WaitNext), Ok(Some(0)));
    assert_eq!(c.slot(0).unwrap().overlaps, 0);
}

#[test]
fn already_waiting_peer_precedes_all_equal_counter_newcomers() {
    for first in 0..3 {
        let newcomer = (first + 1) % 3;
        let waiting = (first + 2) % 3;
        let mut tuples = [[0, 10, 10]; 3];
        tuples[newcomer][0] = 10;
        for polls in 0..=2 {
            let mut c = Calendar::new(Rm, tuples, HZ, 0).unwrap();
            if c.step(0, Poll).unwrap() != Some(first) {
                assert_eq!(c.step(0, Yield), Ok(Some(first)));
            }
            for _ in 0..polls { assert_eq!(c.step(10, Poll), Ok(Some(first))); }
            assert_eq!(c.step(10, WaitNext), Ok(Some(waiting)));
            assert_eq!(c.step(10, Yield), Ok(Some(first.min(newcomer))));
            assert_eq!(c.step(10, Yield), Ok(Some(first.max(newcomer))));
        }
    }
}

#[test]
fn voluntary_dispatch_at_the_same_counter_keeps_the_queue_order_it_established() {
    let mut c = Calendar::new(Rm, [[10, 10, 10], [0, 10, 10], [0, 10, 10]], HZ, 0).unwrap();
    assert_eq!(c.step(0, Poll), Ok(Some(1)));
    assert_eq!(c.step(10, Poll), Ok(Some(1)));
    assert_eq!(c.step(10, Yield), Ok(Some(2)));
    assert_eq!(c.step(10, Poll), Ok(Some(2)));
    assert_eq!(c.step(10, WaitNext), Ok(Some(0)));
    assert_eq!(c.step(10, Yield), Ok(Some(1)));
}
