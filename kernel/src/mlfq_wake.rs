use super::multilevel_service::{self as svc, Mode, Task};
use super::wake_bound;
use core::sync::atomic::{AtomicU64, Ordering::SeqCst};

// Plain and traced ELFs share this little-endian u64 receipt/journal.
const HEADER: usize = 40;
const WIDTH: usize = 15;
const ROWS: usize = 64;
#[unsafe(no_mangle)]
static mlfq_wake_witness: [AtomicU64; HEADER + WIDTH * ROWS] =
    [const { AtomicU64::new(0) }; HEADER + WIDTH * ROWS];

fn get(i: usize) -> u64 { mlfq_wake_witness[i].load(SeqCst) }
fn put(i: usize, value: u64) { mlfq_wake_witness[i].store(value, SeqCst); }
fn require(condition: bool, stage: u64) {
    if !condition {
        put(39, stage);
        put(0, 5);
        super::uart_write(b"MLFQ WAKE VIOLATION\n");
        svc::fail()
    }
}
fn delta(later: u64, earlier: u64) -> u64 {
    later.checked_sub(earlier).unwrap_or_else(|| svc::fail())
}
fn add(a: u64, b: u64) -> u64 {
    a.checked_add(b).unwrap_or_else(|| svc::fail())
}

// Each sample packet has 32 reproducible words; fold in order with FNV-1a.
#[inline(never)]
fn packet(seed: u64, sequence: u64) -> u64 {
    let mut hash = seed;
    for offset in 0..32 {
        let sample = core::hint::black_box(sequence.rotate_left(offset) ^ u64::from(offset));
        hash = (hash ^ sample).wrapping_mul(0x100000001b3);
    }
    hash
}

fn target() {
    let mut hash = 1;
    let mut count = 0;
    loop {
        count = add(count, 1);
        hash = packet(hash, count);
        if get(17) != 0 { break; }
    }
    put(20, count);
    put(24, hash);
    put(18, 1);
    svc::block();
    let receipt = packet(hash ^ get(19), add(count, 1));
    put(28, receipt);
    put(9, svc::counter());
}

fn stream(id: usize) {
    let mut hash = id as u64 + 1;
    let mut count = 0;
    let mut supplied = false;
    loop {
        count = add(count, 1);
        hash = packet(hash, count);
        if id == 3 && count == 1 { put(17, hash); }
        if id == 1 && !supplied && get(18) != 0 {
            put(19, hash);
            put(32, count);
            require(svc::wake(0), 1);
            supplied = true;
        }
        if get(9) != 0 {
            put(20 + id, count);
            put(24 + id, hash);
            put(28 + id, packet(hash ^ get(28), add(count, 1)));
            return;
        }
    }
}

const TASKS: [Task; 4] = [
    Task { entry: target, class: 0 },
    Task { entry: || stream(1), class: 0 },
    Task { entry: || stream(2), class: 0 },
    Task { entry: || stream(3), class: 0 },
];

pub fn run() -> ! {
    let u = svc::frequency() / 100;
    let limit = u.checked_mul(23).and_then(|v|
        v.checked_sub(u64::from(cfg!(jnox_app = "mlfq-wake-refuse"))));
    let bound = wake_bound::bound(u, TASKS.len());
    put(1, u);
    put(2, bound.unwrap_or(0));
    put(3, limit.unwrap_or(0));
    if !limit.is_some_and(|l| wake_bound::admits(u, TASKS.len(), l)) {
        put(0, 2);
        super::uart_write(b"MLFQ WAKE REFUSE\n");
        super::park()
    }
    put(0, 1);
    super::uart_write(b"MLFQ WAKE ADMIT\n");
    svc::run(&TASKS, Mode::Feedback { allotments: [2*u, 4*u, 6*u], period: 10*u })
}

#[allow(clippy::too_many_arguments)]
pub fn boundary(kind: u64, result: u64, owner: usize, start: u64, due: u64,
                stop: u64, total: u64, boost: bool, queue: u64, next: usize,
                dispatch: u64, rank: u8, outgoing_frame: usize, next_frame: usize, pre_rank: u8) {
    let row = get(4) as usize;
    require(row < ROWS, 2);
    let fields = [kind, result, owner as u64, start, due, stop, total,
                  u64::from(boost), queue, next as u64, dispatch, u64::from(rank),
                  outgoing_frame as u64, next_frame as u64, u64::from(pre_rank)];
    for (i, v) in fields.into_iter().enumerate() { put(HEADER + WIDTH * row + i, v); }
    put(4, row as u64 + 1);
    if owner == 0 && kind == 4 && rank < 2 { put(11, 1); }
    if owner == 0 && kind == 2 {
        require(get(11) == 1 && rank < 2, 3);
        put(12, outgoing_frame as u64);
    }
    let wake = kind == 3 && result == 1;
    if wake {
        require(owner == 1 && get(12) != 0 && get(5) == 0 && pre_rank < 2, 4);
        put(5, stop);
        put(13, total);
    }
    if get(5) == 0 || get(9) != 0 { return; }
    require(get(8) == 0, 5); // uninterrupted final partial owner interval
    let u = get(1);
    if !wake {
        require(stop.saturating_sub(due) <= u, 6);
        let elapsed = delta(stop, start);
        let period = 10*u;
        require(total / period - delta(total, elapsed) / period <= 1, 14);
        if get(6) != 0 {
            // Each dispatch ahead consumes one unique FIFO position, even on yield.
            require(owner < 4 && get(33) & (1 << owner) == 0 && elapsed <= 3*u, 15);
            put(33, get(33) | (1 << owner));
        }
    }
    put(10, add(get(10), delta(dispatch, stop)));
    require(get(10) <= 2*u, 7);
    if boost && get(6) == 0 {
        require(delta(total, get(13)) <= 11*u && next != 0 && rank == 2, 8);
        put(6, stop);
        put(14, total);
    }
    if next == 0 {
        require(get(6) != 0 && next_frame as u64 == get(12), 9);
        let post = delta(total, get(14));
        require(post <= 9*u, 10);
        put(7, post);
        put(8, dispatch);
    }
}

pub fn terminal() {
    let end = get(9);
    let partial = delta(end, get(8));
    let elapsed = delta(end, get(5));
    let pre = delta(get(14), get(13));
    require(get(5) != 0 && get(8) != 0 && partial <= get(1), 11);
    require(elapsed == add(add(pre, get(7)), add(get(10), partial)) && elapsed <= get(3), 12);
    require((0..4).all(|id| get(20+id) > 0) && get(32) > 0, 13);
    put(15, elapsed);
    put(16, partial);
    put(0, 4);
    super::uart_write(b"MLFQ WAKE VERIFIED\n");
}
