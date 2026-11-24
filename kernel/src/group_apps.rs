use super::group_runtime::{self as service, Task};
use core::sync::atomic::{AtomicBool, Ordering};

static PEER_RAN: AtomicBool = AtomicBool::new(false);

fn first() {
    super::uart_write(b"GROUP BLOCK\n");
    service::block();
    #[cfg(jnox_groups_late)]
    {
        // Trusted workload fault injection, never a scheduler switch.
        let span = service::frequency() * 3 / 10;
        unsafe { core::arch::asm!("msr daifset, #2", "isb", options(nostack)) }
        let start = service::counter();
        while service::counter() - start < span {
            core::hint::spin_loop();
        }
        unsafe { core::arch::asm!("msr daifclr, #2", "isb", options(nostack)) }
    }
    while !PEER_RAN.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }
    super::uart_write(b"GROUP RESUME\n");
}
fn sibling() {
    super::uart_write(b"GROUP WAKE\n");
    if service::wake(0) {
        super::uart_write(b"GROUP RETURN\n");
    }
}
fn peer() {
    super::uart_write(b"GROUP PEER\n");
    PEER_RAN.store(true, Ordering::Release);
}
pub fn run() -> ! {
    #[cfg(jnox_groups_late)]
    super::uart_write(b"GROUP LATE\n");
    let period = service::frequency() / 4;
    service::run(
        period,
        [period / 4, period / 2],
        [(0, period), (0, period), (1, period)],
        [
            Task {
                entry: first,
                child: 0,
                quota: period,
                ready: true,
            },
            Task {
                entry: sibling,
                child: 1,
                quota: period,
                ready: true,
            },
            Task {
                entry: peer,
                child: 2,
                quota: period,
                ready: true,
            },
        ],
    )
}
