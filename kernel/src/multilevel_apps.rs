#[cfg(jnox_mlq_admission)]
pub use super::mlq_admission::run;
#[cfg(jnox_mlfq_wake)]
pub use super::mlfq_wake::run;
use super::multilevel_service::{self as svc, Mode, Task};

fn unit() -> u64 {
    svc::frequency() / 100
}
fn work(ticks: u64) {
    let start = svc::counter();
    while svc::counter().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

#[cfg(jnox_app = "mlq")]
pub fn run() -> ! {
    fn low() {
        svc::yield_now();
    }
    fn busy() {
        work(unit() * 12);
    }
    static TASKS: [Task; 3] = [
        Task {
            entry: low,
            class: 0,
        },
        Task {
            entry: busy,
            class: 2,
        },
        Task {
            entry: busy,
            class: 2,
        },
    ];
    super::uart_write(b"MLQ START\n");
    svc::run(&TASKS, Mode::Strict(unit()))
}

#[cfg(jnox_app = "mlfq")]
pub fn run() -> ! {
    fn client() {
        work(unit());
        svc::yield_now();
        work(unit());
        svc::block();
        work(unit());
        svc::yield_now();
        work(unit() * 4);
    }
    fn cpu() {
        work(unit() * 55);
    }
    fn peer() {
        // Run enough interactive turns to remain a competitor across boosts.
        for _ in 0..22 {
            work(unit());
            svc::wake(0);
        }
    }
    static TASKS: [Task; 3] = [
        Task {
            entry: client,
            class: 0,
        },
        Task {
            entry: cpu,
            class: 0,
        },
        Task {
            entry: peer,
            class: 0,
        },
    ];
    let u = unit();
    super::uart_write(b"MLFQ START\n");
    svc::run(
        &TASKS,
        Mode::Feedback {
            allotments: [4 * u, 6 * u, 8 * u],
            period: 16 * u,
        },
    )
}

#[cfg(jnox_multilevel_limits)]
fn units(n: u64) -> u64 {
    unit().checked_mul(n).filter(|&v| v > 0).unwrap_or_else(|| svc::fail())
}

#[cfg(any(jnox_app = "mlq-blocked", jnox_app = "mlq-exited"))]
pub fn run() -> ! {
    fn terminal() {
        #[cfg(jnox_app = "mlq-blocked")]
        svc::block();
    }
    static TASKS: [Task; 2] = [
        Task { entry: terminal, class: 1 },
        Task { entry: terminal, class: 1 },
    ];
    #[cfg(jnox_app = "mlq-blocked")]
    super::uart_write(b"MLQ-BLOCKED START\n");
    #[cfg(jnox_app = "mlq-exited")]
    super::uart_write(b"MLQ-EXITED START\n");
    svc::run(&TASKS, Mode::Strict(units(1)))
}

#[cfg(any(jnox_app = "mlq-saturated", jnox_app = "mlfq-delayed"))]
pub(super) static WORK_END: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

#[cfg(any(jnox_app = "mlq-saturated", jnox_app = "mlfq-delayed"))]
fn busy_until_end() {
    let end = WORK_END.load(core::sync::atomic::Ordering::Relaxed);
    while svc::counter() < end {
        core::hint::spin_loop();
    }
}

#[cfg(jnox_app = "mlq-saturated")]
pub fn run() -> ! {
    fn low() { work(units(1)); }
    static TASKS: [Task; 3] = [
        Task { entry: low, class: 0 },
        Task { entry: busy_until_end, class: 2 },
        Task { entry: busy_until_end, class: 2 },
    ];
    super::uart_write(b"MLQ-SATURATED START\n");
    svc::run(&TASKS, Mode::Strict(units(1)))
}

#[cfg(jnox_app = "mlfq-delayed")]
pub fn run() -> ! {
    static TASKS: [Task; 3] = [const { Task { entry: busy_until_end, class: 0 } }; 3];
    super::uart_write(b"MLFQ-DELAYED START\n");
    svc::run(&TASKS, Mode::Feedback { allotments: [units(4), units(6), units(8)], period: units(16) })
}
