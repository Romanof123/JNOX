//! Example application and external observation ABI for the linked event service.
use crate::coop_service::{Event, Handler, Post, Reject, Service, Stats};
use crate::fixed_runtime::{self as rt, Mode, Task};
use crate::uart_write;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const RELEASE: u64 = 12_500_000;
static TABLE: [Task; 3] = [
    Task {
        entry: delayed_peer,
        priority: 1,
        delay: RELEASE,
    },
    Task {
        entry: owner,
        priority: 1,
        delay: 0,
    },
    Task {
        entry: ready_peer,
        priority: 1,
        delay: 0,
    },
];

#[repr(C)]
struct Observations {
    header: [u64; 7],
    order: [u64; 4],
    reactions: u64,
    timer_disabled: u64,
    peers: u64,
    handlers: [[u64; 4]; 2],
    elapsed: [u64; 4],
}

#[unsafe(no_mangle)]
static mut coop_app_observations: Observations = Observations {
    header: [0; 7],
    order: [0; 4],
    reactions: 0,
    timer_disabled: 0,
    peers: 0,
    handlers: [[0; 4]; 2],
    elapsed: [0; 4],
};

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

struct Work {
    order: [u64; 4],
    count: usize,
}
impl Work {
    fn record(&mut self, event: Event) {
        self.order[self.count] = event.value;
        self.count += 1;
    }
}

fn follow_up(work: &mut Work, event: Event, post: &mut Post<'_>) {
    work.record(event);
    if event.value == 11 {
        post(Event {
            handler: event.handler,
            value: 33,
        })
        .unwrap();
    }
    let start = rt::counter();
    while rt::counter().wrapping_sub(start) < 64 {
        core::hint::spin_loop();
    }
}

#[cfg(jnox_app = "coop-events")]
fn consume(work: &mut Work, event: Event, _: &mut Post<'_>) {
    work.record(event);
}

fn sample<const N: usize, const H: usize>(handlers: [Handler<Work>; H], follow: usize) {
    let mut service = Service::<Work, N, H>::new(handlers);
    service
        .enqueue(Event {
            handler: follow,
            value: 11,
        })
        .unwrap();
    if N == 2 {
        service
            .enqueue(Event {
                handler: 0,
                value: 22,
            })
            .unwrap();
    }
    let excess = Event {
        handler: 0,
        value: 44,
    };
    let invalid = Event {
        handler: H,
        value: 55,
    };
    let full = service.enqueue(excess) == Err(Reject::Full(excess));
    let bad_id = service.enqueue(invalid) == Err(Reject::InvalidHandler(invalid));
    let mut work = Work {
        order: [0; 4],
        count: 0,
    };
    let mut reactions = 0;
    let mut elapsed = [0; 4];
    while let Some(done) = service.turn(&mut work, rt::counter) {
        if done.overrun {
            reactions += 1;
            uart_write(b"COOP OWNER OVERRUN\n");
        }
        if done.event.value != work.order[work.count - 1] {
            rt::fail();
        }
        elapsed[work.count - 1] = done.elapsed;
    }
    let mut rows = [[0; 4]; 2];
    for (
        row,
        &Stats {
            calls,
            total,
            maximum,
            overruns,
        },
    ) in rows.iter_mut().zip(service.stats())
    {
        *row = [calls, total, maximum, overruns];
    }
    // Sole writer publishes only after all peers have exited.
    unsafe {
        (&raw mut coop_app_observations).write_volatile(Observations {
            header: [
                1,
                N as u64,
                H as u64,
                service.peak() as u64,
                full.into(),
                bad_id.into(),
                work.count as u64,
            ],
            order: work.order,
            reactions,
            timer_disabled: 1,
            peers: PEERS.load(Ordering::Acquire),
            handlers: rows,
            elapsed,
        });
    }
    uart_write(b"COOP DONE\n");
}

fn owner() {
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
    if PEERS.load(Ordering::Acquire) != 0 {
        rt::fail();
    }
    uart_write(b"COOP OWNER YIELD\n");
    rt::yield_now();
    uart_write(b"COOP OWNER RESUME\n");
    let handler = Handler {
        call: follow_up,
        budget: 1,
    };
    #[cfg(jnox_app = "coop-events")]
    sample::<2, 2>(
        [
            Handler {
                call: consume,
                budget: u64::MAX,
            },
            handler,
        ],
        1,
    );
    #[cfg(jnox_app = "coop-single")]
    sample::<1, 1>([handler], 0);
}

pub fn run() -> ! {
    #[cfg(jnox_app = "coop-events")]
    uart_write(b"COOP EVENTS START\n");
    #[cfg(jnox_app = "coop-single")]
    uart_write(b"COOP SINGLE START\n");
    rt::run(&TABLE, Mode::Fifo)
}
