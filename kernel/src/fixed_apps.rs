//! Linked example workloads. All times are physical-counter ticks. At QEMU
//! virt's 62.5 MHz, the RR quantum is 200 ms and HIGH arrives halfway through.
use crate::fixed_runtime::{self as rt, Mode, Task};
use crate::uart_write;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const QUANTUM: u64 = 12_500_000;

// Stable QMP observation layout. Slots in this object always mean A, B, HIGH
// even when their static scheduler table has a different order.
#[repr(C)]
struct Observations {
    stack_pointer: [AtomicU64; 3],
    progress: [AtomicU64; 2],
    a_at_high: AtomicU64,
    a_at_b_start: AtomicU64,
    b_at_a_done: AtomicU64,
}

#[unsafe(no_mangle)]
static fixed_app_observations: Observations = Observations {
    stack_pointer: [const { AtomicU64::new(0) }; 3],
    progress: [const { AtomicU64::new(0) }; 2],
    a_at_high: AtomicU64::new(0),
    a_at_b_start: AtomicU64::new(0),
    b_at_a_done: AtomicU64::new(0),
};

fn record_stack(role: usize) {
    let sp: u64;
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    fixed_app_observations.stack_pointer[role].store(sp, Ordering::Relaxed);
}

// Each counter has exactly one writer. The atomic publications also keep
// the busy work observable across asynchronous interrupts and in a QMP dump.
#[inline(never)]
fn work_until(worker: usize, ready: &AtomicU64) {
    let progress = &fixed_app_observations.progress[worker];
    let mut n = progress.load(Ordering::Relaxed);
    loop {
        n = n.wrapping_add(1);
        progress.store(n, Ordering::Release);
        if ready.load(Ordering::Acquire) != 0 {
            return;
        }
    }
}

#[cfg(jnox_app = "fixed-fifo")]
mod selected {
    use super::*;
    static TABLE: [Task; 3] = [
        Task { entry: producer, priority: 1, delay: 0 },
        Task { entry: sleeper, priority: 1, delay: 0 },
        Task { entry: alarm, priority: 3, delay: QUANTUM / 2 },
    ];

    pub(super) fn start() -> ! {
        uart_write(b"FIXED FIFO START\n");
        rt::run(&TABLE, Mode::Fifo)
    }

    fn producer() {
        record_stack(0);
        uart_write(b"FIXED FIFO A START\n");
        rt::yield_now();
        uart_write(b"FIXED FIFO A RESUME\n");
        work_until(0, &fixed_app_observations.a_at_high);
        uart_write(b"FIXED FIFO A DONE\n");
    }

    fn sleeper() {
        record_stack(1);
        uart_write(b"FIXED FIFO B YIELD\n");
        if !rt::sleep_ticks(0) {
            uart_write(b"FIXED FIFO B YIELD FAILED\n");
            rt::exit();
        }
        uart_write(b"FIXED FIFO B SLEEP\n");
        if rt::sleep_ticks(QUANTUM * 2) && !rt::sleep_ticks(u64::MAX) {
            uart_write(b"FIXED FIFO B DONE\n");
        } else {
            uart_write(b"FIXED FIFO B SLEEP FAILED\n");
        }
        rt::exit();
    }

    fn alarm() {
        record_stack(2);
        let work = fixed_app_observations.progress[0].load(Ordering::Acquire);
        uart_write(b"FIXED FIFO HIGH\n");
        fixed_app_observations.a_at_high.store(work, Ordering::Release);
    }
}

#[cfg(jnox_app = "fixed-rr")]
mod selected {
    use super::*;
    static TABLE: [Task; 3] = [
        Task { entry: worker_a, priority: 2, delay: 0 },
        Task { entry: alarm, priority: 5, delay: QUANTUM / 2 },
        Task { entry: worker_b, priority: 2, delay: 0 },
    ];

    pub(super) fn start() -> ! {
        uart_write(b"FIXED RR START\n");
        rt::run(&TABLE, Mode::RoundRobin(QUANTUM))
    }

    fn worker_a() {
        record_stack(0);
        uart_write(b"FIXED RR A START\n");
        work_until(0, &fixed_app_observations.progress[1]);
        let peer = fixed_app_observations.progress[1].load(Ordering::Acquire);
        uart_write(b"FIXED RR A DONE\n");
        fixed_app_observations.b_at_a_done.store(peer, Ordering::Release);
    }

    fn worker_b() {
        record_stack(1);
        let peer = fixed_app_observations.progress[0].load(Ordering::Acquire);
        fixed_app_observations.a_at_b_start.store(peer, Ordering::Release);
        uart_write(b"FIXED RR B START\n");
        work_until(1, &fixed_app_observations.b_at_a_done);
        uart_write(b"FIXED RR B DONE\n");
    }

    fn alarm() {
        record_stack(2);
        let work = fixed_app_observations.progress[0].load(Ordering::Acquire);
        fixed_app_observations.a_at_high.store(work, Ordering::Release);
        uart_write(b"FIXED RR HIGH\n");
        let start = rt::counter();
        while rt::counter().wrapping_sub(start) < QUANTUM / 4 {
            core::hint::spin_loop();
        }
    }
}

#[cfg(jnox_app = "fixed-empty")]
mod selected {
    use super::*;
    static TABLE: [Task; 0] = [];

    pub(super) fn start() -> ! {
        uart_write(b"FIXED EMPTY START\n");
        rt::run(&TABLE, Mode::Fifo)
    }
}

#[cfg(jnox_app = "fixed-delayed")]
mod selected {
    use super::*;
    static TABLE: [Task; 1] = [
        Task { entry: delayed, priority: 1, delay: QUANTUM / 2 },
    ];

    pub(super) fn start() -> ! {
        uart_write(b"FIXED DELAYED START\n");
        rt::run(&TABLE, Mode::Fifo)
    }

    fn delayed() {
        record_stack(0);
        uart_write(b"FIXED DELAYED TASK\n");
        rt::exit();
    }
}

pub fn run() -> ! {
    // Keep the named QMP observation region linked even for an empty table.
    core::hint::black_box(&fixed_app_observations);
    selected::start()
}
