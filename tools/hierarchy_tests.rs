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
