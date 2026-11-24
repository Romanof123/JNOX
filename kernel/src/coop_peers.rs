use crate::fixed_runtime::{self as rt, Task};
use crate::uart_write;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const RELEASE: u64 = 12_500_000;
pub static TABLE: [Task; 3] = [
    Task {
        entry: delayed_peer,
        priority: 1,
        delay: RELEASE,
    },
    Task {
        entry: crate::fixed_apps::owner,
        priority: 1,
        delay: 0,
    },
    Task {
        entry: ready_peer,
        priority: 1,
        delay: 0,
    },
];

static PEERS: AtomicU64 = AtomicU64::new(0);

fn ready_peer() {
    PEERS.store(2, Ordering::Release);
    uart_write(b"COOP PEER 2\n");
}

fn delayed_peer() {
    PEERS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| Some(10 * v))
        .unwrap();
    uart_write(b"COOP PEER 0\n");
}

fn timer_enabled() -> bool {
    let ctl: u64;
    unsafe {
        asm!("isb", "mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags));
    }
    ctl & 1 != 0
}

pub fn peers() -> u64 {
    PEERS.load(Ordering::Acquire)
}

pub fn yield_to_peers() -> u64 {
    uart_write(b"COOP OWNER START\n");
    let start = rt::counter();
    if !timer_enabled() {
        rt::fail();
    }
    while timer_enabled() {
        if rt::counter().wrapping_sub(start) > RELEASE * 10 {
            rt::fail();
        }
        core::hint::spin_loop();
    }
    let before = peers();
    if before != 0 {
        rt::fail();
    }
    uart_write(b"COOP OWNER YIELD\n");
    rt::yield_now();
    uart_write(b"COOP OWNER RESUME\n");
    before
}
