use crate::coop_bounded::{Handler, Post, Receipt, Request, Service};
use crate::coop_peers::{TABLE, peers, yield_to_peers};
use crate::fixed_runtime::{self as rt, Mode};
use crate::uart_write;

const COST: [u64; 2] = [12_500_000, 6_250_000];
const READINGS: [[u64; 4]; 3] = [[3, 1, 4, 1], [5, 9, 2, 6], [5, 3, 5, 8]];

#[derive(Default)]
#[repr(C)]
struct Journal {
    header: [u64; 9],
    decisions: [[u64; 10]; 5],
    completions: [[u64; 6]; 3],
    totals: [u64; 2],
}

#[unsafe(no_mangle)]
static mut coop_bounded_observations: Journal = Journal {
    header: [0; 9],
    decisions: [[0; 10]; 5],
    completions: [[0; 6]; 3],
    totals: [0; 2],
};

#[derive(Default)]
struct Work {
    log: Journal,
    posts: usize,
    count: usize,
}

impl Work {
    fn record(&mut self, now: u64, req: Request, r: Receipt) {
        self.log.decisions[self.posts] = [
            now,
            req.handler as u64,
            req.block,
            req.deadline,
            r.decision as u64,
            r.before.0 as u64,
            r.before.1,
            r.after.0 as u64,
            r.after.1,
            r.remaining,
        ];
        self.posts += 1;
    }

    fn submit(&mut self, post: &mut Post<'_>, handler: usize, block: u64, allowance: u64) {
        let now = rt::counter();
        let req = Request {
            handler,
            block,
            deadline: now.checked_add(allowance).unwrap(),
        };
        self.record(now, req, post(req, now));
    }
}

fn reduce(work: &mut Work, req: Request, _: &mut Post<'_>) {
    let readings = &READINGS[(req.block - 1) as usize];
    let mut sum = 0;
    let mut weighted = 0;
    for (i, value) in readings.iter().copied().enumerate() {
        sum += value;
        weighted += (i as u64 + 1) * value;
    }
    let row = &mut work.log.completions[work.count];
    row[4] = sum;
    row[5] = weighted;
    work.log.totals[0] += sum;
    work.log.totals[1] += weighted;
}

fn producer(work: &mut Work, req: Request, post: &mut Post<'_>) {
    reduce(work, req, post);
    work.submit(post, 1, 8, 2 * COST[1]);
    work.submit(post, 1, 3, 4 * COST[0]);
}

pub(crate) fn owner() {
    let before = yield_to_peers();
    let mut service = Service::<Work, 3, 2>::new([
        Handler {
            cost: COST[0],
            call: producer,
        },
        Handler {
            cost: COST[1],
            call: reduce,
        },
    ]);
    let mut work = Work::default();
    let mut post = |req, now| service.post(req, now);
    work.submit(&mut post, 0, 1, 4 * COST[0]);
    work.submit(&mut post, 1, 2, 4 * COST[0]);
    work.submit(&mut post, 1, 9, 0);
    while let Some((req, start, finish)) = service.turn(&mut work, rt::counter) {
        work.log.completions[work.count][..4].copy_from_slice(&[
            req.handler as u64,
            req.block,
            start,
            finish,
        ]);
        work.count += 1;
    }
    work.log.header = [
        1,
        3,
        COST[0],
        COST[1],
        before,
        peers(),
        service.peak() as u64,
        work.posts as u64,
        work.count as u64,
    ];
    unsafe {
        (&raw mut coop_bounded_observations).write_volatile(work.log);
    }
    uart_write(b"COOP DONE\n");
}

pub fn run() -> ! {
    uart_write(b"COOP BOUNDED START\n");
    rt::run(&TABLE, Mode::Fifo)
}
