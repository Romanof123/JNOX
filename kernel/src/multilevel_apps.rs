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
