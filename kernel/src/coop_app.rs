//! Example application and external observation ABI for the linked event service.
use crate::coop_service::{Event, Handler, Post, Reject, Service, Stats};
use crate::fixed_runtime::{self as rt, Mode};
use crate::uart_write;
use crate::coop_peers::{peers, TABLE};

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
            peers: peers(),
            handlers: rows,
            elapsed,
        });
    }
    uart_write(b"COOP DONE\n");
}

pub(crate) fn owner() {
    crate::coop_peers::yield_to_peers();
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
