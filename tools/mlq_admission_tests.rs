#[path = "../kernel/src/mlq_bound.rs"]
mod mlq_bound;
use mlq_bound::{admits, bound};

#[test]
fn ready_order_and_raw_count_equality() {
    let queue = [(0, Some(12)), (0, None), (0, None), (2, Some(7))];
    let required = bound(&queue, 1, 5, Some(2));
    assert_eq!(required, Some(14));
    assert!(admits(required, 14));
    assert!(!admits(required, 13));
    assert_eq!(bound(&queue, 2, 5, Some(2)), None);
    assert_eq!(bound(&[(0, Some(3)), (0, None)], 1, 5, Some(1)), Some(4));
    assert_eq!(bound(&[(0, None), (1, None)], 1, 5, Some(0)), Some(0));
}

#[test]
fn closed_set_invalid_inputs_and_overflow() {
    let one = [(0, None)];
    assert_eq!(bound(&[], 0, 1, Some(0)), None);
    assert_eq!(bound(&one, 1, 1, Some(0)), None);
    assert_eq!(bound(&one, 0, 0, Some(0)), None);
    assert_eq!(bound(&one, 0, 1, None), None);
    assert_eq!(bound(&[(0, None); 5], 0, 1, Some(0)), None);
    assert_eq!(bound(&[(2, None), (0, None)], 1, 1, Some(0)), None);
    assert_eq!(
        bound(&[(2, Some(u64::MAX)), (0, None)], 1, 1, Some(1)),
        None
    );
    assert!(!admits(None, u64::MAX));
}
