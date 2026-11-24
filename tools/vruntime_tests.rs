use super::fair_policy::VirtualTime;
use super::vruntime_policy::VruntimeScheduler;

fn twice(time: VirtualTime) -> u64 {
    let (whole, remainder, weight) = time.parts();
    assert_eq!(2 * remainder % weight, 0);
    2 * whole + 2 * remainder / weight
}

fn serve<const N: usize>(sched: &mut VruntimeScheduler<N>, id: usize, ticks: u64) {
    let lease = sched.dispatch().unwrap();
    assert_eq!(lease.task, id);
    assert!(sched.charge(lease, ticks));
}

#[test]
fn compare_each_dispatch_charge_floor_and_wake_against_two_weight_integer_oracle() {
    let quantum = 100;
    let mut sched = VruntimeScheduler::new([1, 2]).unwrap();
    let mut service = [0u64; 2];
    let mut v = [0u64; 2]; // twice each task's exact virtual runtime
    for t in 0..21 {
        if t == 12 {
            assert!((2 * service[0]).abs_diff(service[1]) <= 4 * quantum);
            assert!(sched.sleep(1));
            assert_eq!(twice(sched.floor()), v[0]);
        }
        if t == 15 {
            assert!(v[1] < v[0]);
            assert_eq!(twice(sched.runtime(1).unwrap()), v[1]);
            v[1] = v[0];
            assert!(sched.wake(1));
            assert_eq!(twice(sched.runtime(1).unwrap()), v[1]);
            assert_eq!(twice(sched.floor()), v[0]);
        }
        let both = !(12..15).contains(&t);
        let id = usize::from(both && v[1] < v[0]);
        let elapsed = quantum + (t % 5) * 11;
        serve(&mut sched, id, elapsed);
        v[id] += if id == 0 { 2 * elapsed } else { elapsed };
        service[id] += elapsed;
        assert_eq!(twice(sched.runtime(0).unwrap()), v[0]);
        assert_eq!(twice(sched.runtime(1).unwrap()), v[1]);
        assert_eq!(sched.service(0), Some(service[0]));
        assert_eq!(sched.service(1), Some(service[1]));
        assert_eq!(
            twice(sched.floor()),
            if both { v[0].min(v[1]) } else { v[0] }
        );
    }
}

#[test]
fn equal_timestamps_choose_lowest_id_and_each_lease_can_charge_only_once() {
    let mut sched = VruntimeScheduler::new([1, 2]).unwrap();
    let first = sched.dispatch().unwrap();
    assert_eq!(first.task, 0);
    assert!(sched.dispatch().is_none());
    assert!(!sched.sleep(0));
    assert!(!sched.sleep(1));
    assert!(!sched.charge(first, 0));
    assert_eq!(sched.service(0), Some(0));
    assert!(sched.charge(first, 1));
    serve(&mut sched, 1, 1);
    serve(&mut sched, 1, 1);
    let next = sched.dispatch().unwrap();
    assert_eq!(next.task, first.task);
    assert_ne!(next, first);
    assert!(!sched.charge(first, 1));
    assert_eq!(sched.service(0), Some(1));
    assert!(sched.charge(next, 1));
    assert!(!sched.charge(next, 1));
}

#[test]
fn wake_rounds_to_own_grid_and_preserves_ahead_service_when_the_floor_is_unchanged() {
    let mut sched = VruntimeScheduler::new([3, 2]).unwrap();
    assert!(sched.sleep(1));
    serve(&mut sched, 0, 1);
    assert_eq!(sched.floor().parts(), (0, 1, 3));
    assert!(sched.wake(1));
    assert_eq!(sched.runtime(1).unwrap().parts(), (0, 1, 2));
    assert!(sched.sleep(1));
    assert!(sched.wake(1));
    assert_eq!(sched.runtime(1).unwrap().parts(), (0, 1, 2));

    assert!(sched.sleep(1));
    serve(&mut sched, 0, 1);
    assert_eq!(sched.floor().parts(), (0, 2, 3));
    assert!(sched.wake(1));
    assert_eq!(sched.runtime(1).unwrap().parts(), (1, 0, 2));
    assert_eq!(sched.service(1), Some(0)); // Wake placement consumes no CPU ticks.
    assert!(sched.sleep(0));
    assert_eq!(sched.floor().parts(), (1, 0, 2));
    assert!(sched.sleep(1));
    assert!(sched.dispatch().is_none());
    assert_eq!(sched.floor().parts(), (1, 0, 2));
    assert!(sched.wake(0));
    assert_eq!(sched.runtime(0).unwrap().parts(), (1, 0, 3));
}

#[test]
fn physical_service_overflow_preserves_fraction_and_unretired_owner() {
    let weight = u32::MAX;
    let mut sched = VruntimeScheduler::new([weight]).unwrap();
    serve(&mut sched, 0, u64::MAX - 1);
    let before = sched.runtime(0).unwrap().parts();
    let pending = sched.dispatch().unwrap();
    assert!(!sched.charge(pending, 2));
    assert_eq!(sched.service(0), Some(u64::MAX - 1));
    assert_eq!(sched.runtime(0).unwrap().parts(), before);
    assert_eq!(sched.floor().parts(), before);
    assert!(sched.charge(pending, 1));
    assert_eq!(
        sched.runtime(0).unwrap().parts(),
        (u64::MAX / u64::from(weight), 0, u64::from(weight))
    );
}

#[test]
fn overflowing_wake_and_virtual_charge_leave_states_available_for_a_valid_retry() {
    let mut sched = VruntimeScheduler::new([1, 2, 1]).unwrap();
    assert!(sched.sleep(1));
    assert!(sched.sleep(2));
    serve(&mut sched, 0, u64::MAX);
    assert!(sched.wake(1));
    assert!(sched.sleep(0));
    let pending = sched.dispatch().unwrap();
    assert_eq!(pending.task, 1);
    assert!(!sched.charge(pending, 2)); // MAX + 2/2 cannot fit.
    assert_eq!(sched.runtime(1).unwrap().parts(), (u64::MAX, 0, 2));
    assert_eq!(sched.service(1), Some(0));
    assert!(sched.charge(pending, 1));
    assert_eq!(sched.floor().parts(), (u64::MAX, 1, 2));
    assert!(!sched.wake(2)); // ceil(MAX + 1/2) on a unit-weight grid.
    assert_eq!(sched.runtime(2).unwrap().parts(), (0, 0, 1));
    assert!(!sched.sleep(2));
    assert_eq!(sched.dispatch().unwrap().task, 1);
}

#[test]
fn validates_task_count_weights_and_readiness_transitions() {
    assert!(VruntimeScheduler::new([]).is_none());
    assert!(VruntimeScheduler::new([1, 0]).is_none());
    assert!(VruntimeScheduler::new([1; 33]).is_none());
    let mut sched = VruntimeScheduler::new([1; 32]).unwrap();
    assert!(sched.runtime(32).is_none());
    assert!(sched.service(32).is_none());
    assert!(!sched.wake(32));
    assert!(!sched.sleep(32));
    assert!(!sched.wake(31));
    assert!(sched.sleep(31));
    assert!(!sched.sleep(31));
    let lease = sched.dispatch().unwrap();
    assert!(!sched.wake(31));
    assert!(sched.charge(lease, 1));
    assert!(sched.wake(31));
}
