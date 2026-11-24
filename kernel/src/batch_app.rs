//! Ordinary Rust consumer. Only this module chooses the work, estimates and releases.
use super::{
    batch_policy::BatchPolicy,
    batch_runtime::{self as batch, Error, Handle},
    uart_write,
};
use core::sync::atomic::{AtomicUsize, Ordering};

static FINISHED: AtomicUsize = AtomicUsize::new(0);

fn compute_until(deadline: u64) {
    let mut n = 0u64;
    let mut sum = 0u64;
    while batch::counter() < deadline {
        for _ in 0..64 {
            n += 1;
            sum = core::hint::black_box(sum.wrapping_add(n));
        }
    }
    // n is a multiple of 64; divide first to avoid multiplication overflow.
    assert_eq!(sum, (n / 2).wrapping_mul(n + 1));
}

fn continuation(_: usize, _: Handle) {
    uart_write(b"APP CONTINUATION\nAPP FINISHED\n");
}

fn underestimated(_: usize, _: Handle) {
    compute_until(batch::counter() + batch::frequency() / 1000);
    uart_write(b"APP OVERRUN SUBMIT\n");
    assert!(batch::submit(continuation, 0, 1, 0).is_ok());
    uart_write(b"APP OVERRUN RETAINED\n");
}

fn item(index: usize, own: Handle) {
    let label: &[u8] = match index {
        1 => b"APP OLDER ONE\n",
        2 => b"APP OLDER TWO\n",
        3 => b"APP SHORT\n",
        _ => unreachable!(),
    };
    uart_write(label);
    if FINISHED.fetch_add(1, Ordering::Relaxed) == 2 {
        // The original producer's slot is vacant by this point. Submit into
        // that slot and exercise an obsolete token from its previous occupant.
        assert!(batch::submit(underestimated, 0, 1, batch::frequency() / 20).is_ok());
        let old = unsafe { (*(&raw const PRODUCER)).unwrap() };
        assert!(old != own && batch::complete(old) == Err(Error::Stale));
        uart_write(b"APP STALE REJECTED\n");
    }
}

static mut PRODUCER: Option<Handle> = None;

fn producer(_: usize, own: Handle) {
    unsafe {
        PRODUCER = Some(own);
    }
    let hz = batch::frequency();
    assert!(matches!(batch::submit(item, 0, 0, 0), Err(Error::Estimate)));
    assert!(matches!(
        batch::submit(item, 0, 1 << 32, 0),
        Err(Error::Estimate)
    ));
    assert!(matches!(
        batch::submit(item, 0, 1, u64::MAX),
        Err(Error::Deadline)
    ));
    uart_write(b"APP ESTIMATES REJECTED\n");
    let older = batch::submit(item, 1, hz * 2, 0).unwrap();
    batch::submit(item, 2, hz * 2, 0).unwrap();
    assert_eq!(batch::complete(older), Err(Error::Stale));
    let delay = hz / 10;
    batch::submit(item, 3, hz / 2, delay).unwrap();
    assert!(matches!(batch::submit(item, 0, 1, 0), Err(Error::Capacity)));
    uart_write(b"APP FULL REJECTED\nAPP WORK BEGIN\n");
    compute_until(batch::counter() + delay + hz / 500);
    uart_write(b"APP WORK END\n");
}

pub(super) fn run() -> ! {
    #[cfg(jnox_app = "batch-sjf")]
    let policy = BatchPolicy::Sjf;
    #[cfg(jnox_app = "batch-srtf")]
    let policy = BatchPolicy::Srtf;
    #[cfg(jnox_app = "batch-hrrn")]
    let policy = BatchPolicy::Hrrn;
    batch::start(policy, producer, batch::frequency())
}
