#[path = "../kernel/src/hierarchy_policy.rs"]
mod hierarchy_policy;
use hierarchy_policy::Hierarchy;

#[test]
fn priority_is_lexicographic_and_caps_are_shared() {
    let mut h = Hierarchy::new(
        10,
        8,
        [4, 6],
        [(1, 6), (0, 2), (0, 4)],
        [(0, 6), (2, 4), (1, 1), (1, 4)],
    )
    .unwrap();
    let all = [true; 4];
    assert_eq!(h.grant_at(10, 8, &all), Some((0, 1, 2, 1)));
    assert_eq!(h.grant_at(11, 8, &all), Some((0, 1, 3, 1)));
    assert_eq!(h.remaining_at(12, 0, 1, 3), Some((2, 0, 3)));
    assert_eq!(h.grant_at(12, 8, &all), Some((0, 2, 1, 2)));
    assert_eq!(h.remaining_at(14, 0, 2, 1), Some((0, 2, 2)));
    assert_eq!(h.remaining_at(14, 1, 0, 0), Some((6, 6, 6)));
    assert_eq!(h.grant_at(14, 8, &all), Some((1, 0, 0, 4)));
    assert_eq!(h.grant_at(35, 8, &all), Some((0, 1, 2, 1)));
    assert_eq!(h.remaining_at(36, 1, 0, 0), Some((6, 6, 6)));
    assert_eq!(
        h.grant_at(41, 8, &[true, false, false, false]),
        Some((1, 0, 0, 1))
    );
    assert_eq!(h.grant_at(42, 8, &all), Some((0, 1, 2, 1)));
}

#[test]
fn denial_is_transactional_and_time_never_wraps() {
    let mut h = Hierarchy::new(10, 8, [4], [(0, 3)], [(0, 2)]).unwrap();
    assert_eq!(h.grant_at(9, 1, &[true]), None);
    assert_eq!(h.grant_at(200, 0, &[true]), None);
    assert_eq!(h.grant_at(200, 1, &[false]), None);
    assert_eq!(h.grant_at(10, 8, &[true]), Some((0, 0, 0, 2)));
    assert_eq!(h.remaining_at(10, 0, 0, 0), Some((2, 1, 0)));
    assert_eq!(h.remaining_at(9, 0, 0, 0), None);
    assert_eq!(h.grant_at(11, 1, &[true]), None);
    assert_eq!(h.grant_at(12, 1, &[true]), None);
    assert_eq!(h.remaining_at(12, 0, 0, 0), Some((2, 1, 0)));
    assert_eq!(h.remaining_at(12, 1, 0, 0), None);
    assert_eq!(h.grant_at(u64::MAX - 1, 8, &[true]), Some((0, 0, 0, 1)));
    assert_eq!(h.grant_at(u64::MAX, 8, &[true]), None);
}

#[test]
fn invalid_budgets_and_relationships_are_rejected() {
    for (period, parent, child, leaf) in [
        (0, 1, (0, 1), (0, 1)),
        (8, 0, (0, 1), (0, 1)),
        (8, 9, (0, 1), (0, 1)),
        (8, 1, (0, 0), (0, 1)),
        (8, 1, (0, 1), (0, 9)),
        (8, 1, (1, 1), (0, 1)),
        (8, 1, (0, 1), (1, 1)),
    ] {
        assert!(Hierarchy::new(0, period, [parent], [child], [leaf]).is_none());
    }
}

#[test]
fn ordinary_late_service_remains_on_outgoing_path_across_many_refills() {
    let mut h = Hierarchy::new(
        0,
        10,
        [2, 7],
        [(0, 4), (0, 9), (1, 7)],
        [(0, 5), (1, 9), (2, 7)],
    )
    .unwrap();
    let all = [true; 3];
    let n = 100_000_000_000_000;
    let now = 10 * n + 3;
    assert_eq!(h.grant_at(0, 10, &all), Some((0, 0, 0, 2)));
    assert_eq!(
        h.finish_at(now),
        Some((now, now - 2, [8 * n + 1, 6 * n - 1, 5 * n - 2]))
    );
    assert_eq!(h.finish_at(now), None);
    assert_eq!(h.grant_at(now, 10, &all), Some((1, 2, 2, 7)));
    assert_eq!(h.finish_at(now + 1), Some((1, 0, [0; 3])));
    let sibling = [false, true, false];
    let recovery = (n + 4 * n + 1) * 10;
    assert_eq!(h.next_actionable(now + 1, &sibling), Some(recovery));
    assert_eq!(h.grant_at(recovery - 1, 10, &sibling), None);
    assert_eq!(h.grant_at(recovery, 10, &sibling), Some((0, 1, 1, 1)));
    // Repeat a late entry on the sibling. Its independent child/leaf budget
    // stays positive; one new parent debt postpones the next actionable wake.
    assert_eq!(h.finish_at(recovery + 3), Some((3, 2, [2, 0, 0])));
    assert_eq!(
        h.next_actionable(recovery + 3, &sibling),
        Some(recovery + 20)
    );
}

#[test]
fn full_counter_horizon_keeps_debt_and_rejects_unrepresentable_wakeup() {
    let mut h = Hierarchy::new(0, u64::MAX, [1], [(0, 1)], [(0, 1)]).unwrap();
    assert_eq!(h.grant_at(0, 1, &[true]), Some((0, 0, 0, 1)));
    assert_eq!(
        h.finish_at(u64::MAX),
        Some((u64::MAX, u64::MAX - 1, [u64::MAX - 2; 3]))
    );
    assert_eq!(h.next_actionable(u64::MAX, &[true]), None);
}

#[test]
fn earliest_action_rejects_stale_time_and_requires_a_representable_tick() {
    let mut h = Hierarchy::new(10, 8, [4], [(0, 8)], [(0, 8)]).unwrap();
    assert_eq!(h.next_actionable(13, &[true]), Some(13));
    assert_eq!(h.next_actionable(13, &[false]), None);
    assert_eq!(h.grant_at(13, 4, &[true]), Some((0, 0, 0, 4)));
    assert_eq!(h.next_actionable(14, &[true]), None);
    // Cross into window 1, then query the earlier grid without moving any balance.
    assert_eq!(h.finish_at(19), Some((6, 2, [0; 3])));
    assert_eq!(h.next_actionable(11, &[true]), None);
    assert_eq!(h.next_actionable(19, &[true]), Some(19));
    assert_eq!(h.grant_at(19, 4, &[true]), Some((0, 0, 0, 2)));

    let start = u64::MAX - 4;
    let mut last = Hierarchy::new(start, 4, [3], [(0, 4)], [(0, 4)]).unwrap();
    assert_eq!(last.next_actionable(u64::MAX - 1, &[true]), Some(u64::MAX - 1));
    assert_eq!(last.grant_at(start, 3, &[true]), Some((0, 0, 0, 3)));
    assert_eq!(last.finish_at(start + 3), Some((3, 0, [0; 3])));
    assert_eq!(last.next_actionable(start + 3, &[true]), None);
    assert_eq!(last.next_actionable(u64::MAX, &[true]), None);
}

#[test]
fn repeated_settlement_matches_single_tick_parent_credit() {
    // The child and leaf have a full period. Only the parent limits these grants.
    let period = 12;
    let quota = 3i64;
    let mut h = Hierarchy::new(0, period, [quota as u64], [(0, period)], [(0, period)]).unwrap();
    let mut balance = quota;
    let mut now = 0;
    for duration in [6, 5, 8, 4] {
        let mut first = now;
        while balance <= 0 {
            first += 1;
            if first % period == 0 {
                balance = (balance + quota).min(quota);
            }
        }
        assert_eq!(h.next_actionable(now, &[true]), Some(first));
        let reserved = (balance as u64).min(period - first % period);
        assert_eq!(h.grant_at(first, period, &[true]), Some((0, 0, 0, reserved)));
        now = first + duration;
        for end in first + 1..=now {
            balance -= 1;
            if end % period == 0 {
                balance = (balance + quota).min(quota);
            }
        }
        assert_eq!(h.finish_at(now), Some((duration, duration.saturating_sub(reserved),
                                          [(-balance).max(0) as u64, 0, 0])));
    }
}
