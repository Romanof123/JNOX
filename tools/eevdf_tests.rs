//! Host acceptance inputs for the live single-CPU EEVDF profile.

use crate::eevdf_policy::Eevdf;

#[test]
fn twelve_live_grants_match_rational_eligibility_service_and_response() {
    let mut policy = Eevdf::new([1, 2, 1], [1, 8, 4]).unwrap();
    for id in 0..3 {
        assert!(policy.wake(id));
    }
    // For this finite trace fair fluid increments have denominator 20:
    // (1,2,1)/4, (2,2,0)/4 or (2,2,1)/5. Accumulate each runnable period
    // separately so the sleeping task receives no ideal service during sleep.
    let grants = [
        (0, 1), (1, 1), (2, 1), (1, 3), (0, 2), (0, 2),
        (2, 1), (1, 1), (1, 1), (0, 2), (1, 2), (2, 1),
    ];
    let mut signed_error = [0i32; 3];
    let mut peak_error = 0;
    let mut time = 0;
    let mut wake_at = None;
    let mut responses = Vec::new();
    for (i, (id, units)) in grants.into_iter().enumerate() {
        match i {
            1 => {
                // A alone has the earliest deadline 2, but A runtime 1 > V.
                // B and C both have deadline 4 and are eligible at runtime 0.
                assert_eq!(policy.reference(), Some((1, 4)));
                assert_eq!(policy.snapshot(0), Some((1, 2, 1)));
                assert_eq!(policy.snapshot(1), Some((0, 8, 2)));
                assert_eq!(policy.snapshot(2), Some((0, 4, 1)));
            }
            2 => {
                // B now lies exactly at V=1/2. Shortening C's slice to 2
                // gives eligible C the earlier deadline.
                assert_eq!(policy.reference(), Some((2, 4)));
                assert!(policy.configure(2, 1, 2));
                assert_eq!(policy.snapshot(2), Some((0, 2, 1)));
            }
            3 => {
                assert_eq!(policy.remaining(2), Some(1));
                assert!(policy.sleep(2));
                assert_eq!(policy.reference(), Some((2, 3)));
                assert!(policy.configure(0, 2, 2));
                assert_eq!(policy.snapshot(0), Some((2, 4, 2)));
                assert_eq!(policy.reference(), Some((3, 4)));
            }
            4 => {
                assert_eq!(policy.reference(), Some((6, 4)));
                assert!(policy.wake(2));
                wake_at = Some(time);
                assert_eq!(policy.snapshot(2), Some((2, 3, 1)));
                assert_eq!(policy.remaining(2), Some(1));
                assert_eq!(policy.reference(), Some((8, 5)));
            }
            7 => {
                assert_eq!(policy.snapshot(2), Some((3, 5, 1)));
                assert_eq!(policy.remaining(2), Some(2));
                assert!(policy.sleep(2));
            }
            8 => {
                assert_eq!(policy.reference(), Some((11, 4)));
                assert!(policy.wake(2));
                wake_at = Some(time);
                assert_eq!(policy.snapshot(2), Some((3, 5, 1)));
            }
            _ => {}
        }
        assert_eq!(policy.dispatch(), Some(id), "grant {i}");
        if id == 2 {
            if let Some(wake) = wake_at.take() {
                responses.push(time - wake);
            }
        }
        assert!(policy.stop(id, units));
        let shares = match i {
            0..=2 => [5, 10, 5],
            3 | 7 => [10, 10, 0],
            _ => [8, 8, 4],
        };
        for task in 0..3 {
            let actual = if id == task { 20 } else { 0 };
            signed_error[task] += (actual - shares[task]) * units as i32;
            peak_error = peak_error.max(signed_error[task].abs());
        }
        time += units;
    }
    assert_eq!(time, 18);
    assert_eq!(peak_error, 25); // 25/20 physical service units
    assert_eq!(signed_error, [-3, 2, 1]);
    assert_eq!(responses, [4, 5]); // wake to first dispatch, in service units
}

#[test]
fn invalid_transitions_preserve_request_anchor_and_charge_once() {
    assert!(Eevdf::<0>::new([], []).is_none());
    assert!(Eevdf::<33>::new([1; 33], [1; 33]).is_none());
    assert!(Eevdf::new([0, 1], [1, 1]).is_none());
    assert!(Eevdf::new([17], [1]).is_none());
    assert!(Eevdf::new([1], [0]).is_none());
    let mut policy = Eevdf::new([16, 1], [64, 1]).unwrap();
    assert_eq!(policy.dispatch(), None);
    assert!(!policy.wake(2));
    assert!(policy.wake(0));
    assert!(policy.wake(1));
    assert!(!policy.wake(1));
    assert_eq!(policy.dispatch(), Some(1));
    assert_eq!(policy.dispatch(), None);
    assert!(!policy.sleep(0));
    assert!(!policy.configure(1, 2, 1));
    assert!(!policy.stop(0, 1));
    assert!(!policy.stop(1, 2));
    assert!(policy.stop(1, 1));
    assert!(!policy.stop(1, 1));
    assert_eq!(policy.reference(), Some((1, 17)));
    // A's represented start would be 0 + (1-16)/17, below the epoch.
    assert!(!policy.configure(0, 1, 5));
    assert_eq!(policy.snapshot(0), Some((0, 64, 16)));
    assert_eq!(policy.dispatch(), Some(0));
    assert!(policy.stop(0, 1));
    assert!(policy.configure(0, 16, 64));
    assert_eq!(policy.snapshot(0), Some((1, 64, 16)));
    assert_eq!(policy.remaining(0), Some(63));
    assert!(policy.configure(0, 16, 8));
    assert_eq!(policy.snapshot(0), Some((1, 9, 16)));
}

#[test]
fn an_idle_gap_preserves_the_last_clock_and_a_partial_sleeping_request() {
    let mut policy = Eevdf::new([2, 1], [3, 2]).unwrap();
    assert!(policy.wake(0));
    assert_eq!(policy.dispatch(), Some(0));
    assert!(policy.stop(0, 1));
    assert!(policy.sleep(0));
    assert_eq!(policy.reference(), None);
    assert_eq!(policy.remaining(0), Some(2));
    assert!(policy.wake(1)); // previous V=1/2; task 1 rounds upward to 1
    assert_eq!(policy.snapshot(1), Some((1, 3, 1)));
    assert!(policy.wake(0)); // preserve its partial slice while placing at V=1
    assert_eq!(policy.snapshot(0), Some((2, 4, 2)));
    assert_eq!(policy.remaining(0), Some(2));
    assert_eq!(policy.reference(), Some((3, 3)));
    assert_eq!(policy.dispatch(), Some(0));
}
