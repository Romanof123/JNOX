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
