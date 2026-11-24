//! Fixed initial ready set. All times and demands are architectural counter ticks.
use super::mlq_bound::{admits, bound};
use super::multilevel_service::{self as svc, Mode, Task};
use core::sync::atomic::{AtomicU64, Ordering::SeqCst};

const TARGET: usize = 3;
const QUANTUM: u64 = 4_000_000;
const ALLOWANCE: u64 = 4_000_000;
const QUEUE: [(u8, Option<u64>); 4] = [
    (2, Some(8_000_000)),
    (1, Some(8_000_000)),
    (0, Some(8_000_000)),
    (0, None),
];
const PRODUCTS: [u64; 4] = [34816, 382976, 2048, 16];
const DEADLINE: u64 = if cfg!(jnox_app = "mlq-refuse") {
    23_999_999
} else {
    24_000_000
};

// Layout is frozen in docs/mlq-admission-design.md. Atomics allow preemption
// while worker code publishes its first observation or reduction.
#[unsafe(no_mangle)]
static mlq_admission_witness: [AtomicU64; 27] = [const { AtomicU64::new(0) }; 27];

fn put(i: usize, v: u64) {
    mlq_admission_witness[i].store(v, SeqCst);
}
fn get(i: usize) -> u64 {
    mlq_admission_witness[i].load(SeqCst)
}

fn violation() -> ! {
    put(0, 5);
    super::uart_write(b"MLQ VIOLATION\n");
    svc::fail()
}

pub fn epoch(now: u64) {
    put(6, now);
}

pub fn dispatch(id: usize, start: u64, service: [u64; 4], outgoing: Option<usize>, kind: u64) {
    if get(0) == 3 && id == 2 {
        mlq_admission_witness[26].fetch_add(1, SeqCst);
    }
    if get(0) != 1 {
        return;
    }
    for i in 0..TARGET {
        if service[i] > QUEUE[i].1.unwrap_or_else(|| violation()) {
            violation();
        }
    }
    if outgoing == Some(2) && kind == 4 {
        mlq_admission_witness[25].fetch_add(1, SeqCst);
    }
    if id == TARGET {
        if outgoing != Some(2) || kind != 4 || get(25) != 1 || get(20) != 0 || get(22) != 7 {
            violation();
        }
        if mlq_admission_witness[23].fetch_add(1, SeqCst) != 0 {
            violation();
        }
        put(7, start);
        for (id, value) in service.into_iter().enumerate() {
            put(9 + id, value);
        }
        if service[TARGET] != 0 {
            violation();
        }
    }
}

fn observe(first: u64) {
    put(8, first);
    let delta = |a: u64, b: u64| a.checked_sub(b).unwrap_or_else(|| violation());
    let sum = |a: u64, b: u64| a.checked_add(b).unwrap_or_else(|| violation());
    let charged = sum(sum(get(9), get(10)), get(11));
    let gaps = delta(delta(get(7), get(6)), charged);
    let excess = get(11).saturating_sub(QUANTUM);
    let entry = delta(first, get(7));
    let used = sum(sum(gaps, excess), entry);
    let elapsed = delta(first, get(6));
    put(13, gaps);
    put(14, excess);
    put(15, entry);
    put(16, used);
    put(17, elapsed);
    if used > ALLOWANCE || elapsed > DEADLINE || get(23) != 1 {
        violation();
    }
    put(0, 3);
}

const fn samples() -> [u8; 4096] {
    let mut values = [0; 4096];
    let mut i = 0;
    while i < values.len() {
        values[i] = (1 + i % 16) as u8;
        i += 1;
    }
    values
}
static SAMPLES: [u8; 4096] = samples();

fn worker<const ID: usize>() {
    if ID == TARGET {
        observe(svc::counter());
    }
    mlq_admission_witness[22].fetch_or(1 << ID, SeqCst);
    if ID == 2 {
        let until = svc::counter()
            .checked_add(QUANTUM + 1)
            .unwrap_or_else(|| violation());
        let mut total = 0;
        for blocks in 1..=2_000_000 {
            for &sample in core::hint::black_box(&SAMPLES) {
                total += u64::from(sample & 1);
            }
            if svc::counter() >= until {
                put(24, blocks);
                put(20, total);
                return;
            }
        }
        violation();
    }
    let mut result = 0u64;
    for sample in core::hint::black_box(&SAMPLES) {
        let x = u64::from(*sample);
        result = match ID {
            0 => result + x,
            1 => result + x * x,
            2 => result + (x & 1),
            _ => result.max(x),
        };
    }
    put(18 + ID, result);
}

static TASKS: [Task; 4] = [
    Task {
        entry: worker::<0>,
        class: QUEUE[0].0,
    },
    Task {
        entry: worker::<1>,
        class: QUEUE[1].0,
    },
    Task {
        entry: worker::<2>,
        class: QUEUE[2].0,
    },
    Task {
        entry: worker::<3>,
        class: QUEUE[3].0,
    },
];

pub fn terminal(service: [u64; 4]) {
    for i in 0..TARGET {
        if service[i] > QUEUE[i].1.unwrap_or_else(|| violation()) {
            violation();
        }
    }
    if !(1..=2_000_000).contains(&get(24)) || get(26) == 0 || get(20) != PRODUCTS[2] * get(24) {
        violation();
    }
    if get(0) != 3
        || get(22) != 15
        || PRODUCTS
            .iter()
            .enumerate()
            .any(|(i, &v)| i != 2 && get(18 + i) != v)
    {
        violation();
    }
    put(0, 4);
    super::uart_write(b"MLQ VERIFIED\n");
}

pub fn run() -> ! {
    let requirement = bound(&QUEUE, TARGET, QUANTUM, Some(ALLOWANCE));
    put(1, svc::frequency());
    put(2, QUANTUM);
    put(3, ALLOWANCE);
    put(4, requirement.unwrap_or(u64::MAX));
    put(5, DEADLINE);
    if get(1) == 0 || !admits(requirement, DEADLINE) {
        put(0, 2);
        super::uart_write(b"MLQ REFUSE\n");
        super::park()
    }
    put(0, 1);
    super::uart_write(b"MLQ ADMIT\n");
    svc::run(&TASKS, Mode::Strict(QUANTUM))
}
