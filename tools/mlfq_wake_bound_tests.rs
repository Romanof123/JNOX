#[path = "../kernel/src/wake_bound.rs"]
mod wake_bound;
use wake_bound::{admits, bound};

#[test]
fn finite_inputs_and_limits() {
    for u in [1, 625_000, u64::MAX / 23] {
        let b = 23*u;
        assert_eq!(bound(u, 4), Some(b));
        assert!(admits(u, 4, b));
        assert!(!admits(u, 4, b-1));
    }
    for (u, n) in [(0,4), (1,0), (1,3), (1,5), (u64::MAX/23+1,4), (u64::MAX,4)] {
        assert_eq!(bound(u,n), None);
        assert!(!admits(u,n,u64::MAX));
    }
}
