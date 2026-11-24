use super::group_runtime::{self as service, Task};
#[cfg(not(jnox_groups_recurring))]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(jnox_groups_recurring))]
static PEER_RAN: AtomicBool = AtomicBool::new(false);

#[cfg(jnox_groups_recurring)]
mod recurring {
    use core::sync::atomic::{AtomicU64, Ordering};

    static REQUEST: AtomicU64 = AtomicU64::new(0);
    static RECEIPT: AtomicU64 = AtomicU64::new(0);
    static BATCH_SUM: AtomicU64 = AtomicU64::new(0);
    static CUMULATIVE_SUM: AtomicU64 = AtomicU64::new(0);
    const ACKS: [&[u8]; 3] = [b"GROUP ACK 1 ", b"GROUP ACK 2 ", b"GROUP ACK 3 "];

    fn write_sum(mut value: u64) {
        let mut digits = [0; 16];
        for digit in digits.iter_mut().rev() {
            *digit = b"0123456789abcdef"[(value & 15) as usize];
            value >>= 4;
        }
        super::super::uart_write(&digits);
    }

    pub(super) fn produce() {
        let mut total = 0;
        for round in 1..=3 {
            let sum = (1..=256)
                .map(|sample| core::hint::black_box(sample + round))
                .sum::<u64>();
            total += sum;
            BATCH_SUM.store(sum, Ordering::Relaxed);
            REQUEST.store(round, Ordering::Release);
            while RECEIPT.load(Ordering::Acquire) != round {
                core::hint::spin_loop();
            }
            if CUMULATIVE_SUM.load(Ordering::Relaxed) != total {
                super::service::fail()
            }
        }
        if total != 100_224 {
            super::service::fail()
        }
    }

    pub(super) fn consume() {
        let mut total = 0;
        for (index, marker) in ACKS.iter().enumerate() {
            let round = index as u64 + 1;
            while REQUEST.load(Ordering::Acquire) != round {
                core::hint::spin_loop();
            }
            let observed = BATCH_SUM.load(Ordering::Relaxed);
            if observed != 32_896 + 256 * round {
                super::service::fail()
            }
            total += observed;
            super::super::uart_write(marker);
            write_sum(observed);
            super::super::uart_write(b" ");
            write_sum(total);
            super::super::uart_write(b"\n");
            CUMULATIVE_SUM.store(total, Ordering::Relaxed);
            RECEIPT.store(round, Ordering::Release);
        }
    }
}

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
    #[cfg(jnox_groups_recurring)]
    recurring::produce();
    #[cfg(not(jnox_groups_recurring))]
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
    #[cfg(jnox_groups_recurring)]
    recurring::consume();
    super::uart_write(b"GROUP PEER\n");
    #[cfg(not(jnox_groups_recurring))]
    PEER_RAN.store(true, Ordering::Release);
}
pub fn run() -> ! {
    #[cfg(jnox_groups_recurring)]
    super::uart_write(b"GROUP RECURRING\n");
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
