//! Fixed-epoch ordinary Rust workers on private, preemptible EL1 stacks.
use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering::SeqCst};

use super::context::*;
use super::temporal_policy::{CyclicExecutive, WindowDecision};
use super::{park, uart_write};

const Q_TABLE: bool = cfg!(feature = "ordinary-partition-q");
const IDLE: usize = 2;
const RECORDS: usize = 16;
const CYCLE: u64 = if Q_TABLE { 5 } else { 4 };
const RELEASE: u64 = if Q_TABLE { 1 } else { 3 };

global_asm!(
    ".set JNOX_ORDINARY_PARTITION, 1", include_str!("context.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym irq, svc = sym fail, fail = sym fail,
);

#[unsafe(no_mangle)]
static mut ordinary_entry_stamp: u64 = 0;
#[unsafe(no_mangle)]
static mut ordinary_return_stamp: u64 = 0;
static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut BENCH: Option<Bench> = None;
static GENERATION: AtomicUsize = AtomicUsize::new(0);
static LAST: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

static ITERATIONS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

struct Observation {
    when: AtomicU64,
    owner: AtomicUsize,
    sp: AtomicUsize,
    count: AtomicU64,
    value: AtomicU64,
}

static OBS: [Observation; RECORDS] = [const {
    Observation {
        when: AtomicU64::new(0),
        owner: AtomicUsize::new(3),
        sp: AtomicUsize::new(0),
        count: AtomicU64::new(0),
        value: AtomicU64::new(0),
    }
}; RECORDS];

fn counter() -> u64 {
    let t;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) t, options(nostack, preserves_flags)) };
    t
}

#[inline(always)]
fn worker<const ID: usize, const STEP: u64>() -> ! {
    let mut count = 0u64;
    let mut value = STEP;
    let mut seen = usize::MAX;
    loop {
        value = value.rotate_left(7).wrapping_add(STEP);
        count = count.wrapping_add(1);
        let generation = GENERATION.load(SeqCst);
        let now = counter();
        LAST[ID].store(now, SeqCst);
        ITERATIONS[ID].store(count, SeqCst);
        if seen != generation && generation == GENERATION.load(SeqCst) {
            let sp: usize;
            unsafe { asm!("mov {}, sp", out(reg) sp, options(nostack, preserves_flags)) };
            let o = &OBS[generation];
            o.owner.store(ID, SeqCst);
            o.sp.store(sp, SeqCst);
            o.count.store(count, SeqCst);
            o.value.store(value, SeqCst);
            o.when.store(now, SeqCst);
            seen = generation;
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn ordinary_a() -> ! {
    worker::<0, 17>()
}
#[unsafe(no_mangle)]
extern "C" fn ordinary_b() -> ! {
    worker::<1, 31>()
}
#[unsafe(no_mangle)]
extern "C" fn ordinary_idle() -> ! {
    worker::<2, 1>()
}

enum Table {
    P(CyclicExecutive<3>),
    Q(CyclicExecutive<4>),
}

impl Table {
    fn decide(&self, tick: u64) -> WindowDecision {
        let ready = |id| id == 0 || tick >= RELEASE;
        match self {
            Table::P(t) => t.decision(tick, ready),
            Table::Q(t) => t.decision(tick, ready),
        }
        .unwrap_or_else(|| fail())
    }
}

#[derive(Clone, Copy, Default)]
struct Edge {
    due: u64,
    entry: u64,
    live: u64,
    ret: u64,
    next: u64,
    program_before: u64,
    readback: u64,
    program_after: u64,
    owner: usize,
    chosen: usize,
    from: usize,
    out_last: u64,
    out_count: u64,
    out_pc: u64,
    out_sp: usize,
    out_hash: u64,
    in_pc: u64,
    in_sp: usize,
    in_hash: u64,
}
struct Bench {
    table: Table,
    epoch: u64,
    q: u64,
    frequency: u64,
    frames: [*mut Frame; 3],
    current: usize,
    n: usize,
    edges: [Edge; RECORDS],
}

fn stack_top(id: usize) -> usize {
    (&raw mut STACKS).addr() + (id + 1) * STACK_BYTES
}

fn frame_identity(frame: *mut Frame, id: usize) -> (u64, usize, u64) {
    let sp = frame
        .addr()
        .checked_add(size_of::<Frame>())
        .unwrap_or_else(|| fail());
    if sp > stack_top(id) || frame.addr() < stack_top(id) - STACK_BYTES || sp % 16 != 0 {
        fail()
    }
    let f = unsafe { &*frame };
    let hash = f.gpr.iter().fold(f.spsr, |h, &r| h.rotate_left(9) ^ r);
    (f.elr, sp, hash)
}

impl Bench {
    fn dispatch(&mut self, mut e: Edge) -> *mut Frame {
        if self.n >= RECORDS {
            fail()
        }
        loop {
            e.live = counter();
            let tick = (e.live - self.epoch) / self.q;
            let d = self.table.decide(tick);
            e.owner = d.owner.unwrap_or(IDLE);
            e.chosen = d.runnable.unwrap_or(IDLE);
            e.next = self
                .epoch
                .checked_add(d.end_tick.checked_mul(self.q).unwrap_or_else(|| fail()))
                .unwrap_or_else(|| fail());
            let frame = self.frames[e.chosen];
            (e.in_pc, e.in_sp, e.in_hash) = frame_identity(frame, e.chosen);
            e.program_before = counter();
            unsafe {
                asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                     in(reg) e.next, in(reg) 1u64, options(nostack, preserves_flags));
                asm!("mrs {}, cntp_cval_el0", out(reg) e.readback, options(nostack, preserves_flags));
            }
            e.program_after = counter();
            if e.readback != e.next {
                fail()
            }
            if e.program_after < e.next {
                self.current = e.chosen;
                self.edges[self.n] = e;
                GENERATION.store(self.n, SeqCst);
                self.n += 1;
                return frame;
            }
        }
    }
}

// Counter-only recovery probes run before the measured epoch. No task runs here.
fn probe_recovery(table: &Table, frequency: u64) {
    let q = frequency / 20;
    let limit = frequency / 1000;
    let close = if Q_TABLE { 4 * q } else { 2 * q };
    let nominal = table.decide(close / q);
    for late in [limit, limit + 1] {
        let live = table.decide((close + late) / q);
        if live != nominal {
            fail()
        }
    }
    let live = table.decide((close + q + q / 4) / q);
    let (chosen, end) = if Q_TABLE { (Some(1), 6) } else { (None, 4) };
    if live.runnable != chosen || live.end_tick != end {
        fail()
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack)) };
    if frequency == 0 || frequency % 1000 != 0 {
        fail()
    }
    let table = if Q_TABLE {
        Table::Q(
            CyclicExecutive::new(2, 5, [(Some(1), 1), (None, 1), (Some(0), 2), (Some(1), 1)])
                .unwrap_or_else(|| fail()),
        )
    } else {
        Table::P(
            CyclicExecutive::new(2, 4, [(Some(0), 2), (Some(1), 1), (None, 1)])
                .unwrap_or_else(|| fail()),
        )
    };
    probe_recovery(&table, frequency);
    let entries = [
        ordinary_a as extern "C" fn() -> !,
        ordinary_b,
        ordinary_idle,
    ];
    let frames = core::array::from_fn(|i| unsafe {
        let frame = (stack_top(i) - size_of::<Frame>()) as *mut Frame;
        (*frame).elr = entries[i] as usize as u64;
        (*frame).spsr = 0x345; // EL1h, IRQ enabled, D/A/F masked.
        frame
    });
    unsafe {
        BENCH = Some(Bench {
            table,
            epoch: 0,
            q: frequency / 20,
            frequency,
            frames,
            current: IDLE,
            n: 0,
            edges: [Edge::default(); RECORDS],
        });
        let b = (*(&raw mut BENCH)).as_mut().unwrap_or_else(|| fail());
        b.epoch = counter();
        sched_start_frame(b.dispatch(Edge {
            due: b.epoch,
            from: 3,
            ..Edge::default()
        }))
    }
}

extern "C" fn irq(frame: *mut Frame) -> *mut Frame {
    let entry = unsafe { ptr::read_volatile(&raw const ordinary_entry_stamp) };
    let iar = gic_read(GICC + 0x0c);
    if iar & 1023 != TIMER_IRQ {
        fail()
    }
    unsafe { asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy", in(reg) 2u64, options(nostack)) };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack)) };
    let b = unsafe { (*(&raw mut BENCH)).as_mut().unwrap_or_else(|| fail()) };
    let from = b.current;
    let (out_pc, out_sp, out_hash) = frame_identity(frame, from);
    b.frames[from] = frame;
    let prev = &mut b.edges[b.n - 1];
    prev.ret = unsafe { ptr::read_volatile(&raw const ordinary_return_stamp) };
    let e = Edge {
        due: prev.next,
        entry,
        from,
        out_last: LAST[from].load(SeqCst),
        out_pc,
        out_sp,
        out_hash,
        out_count: ITERATIONS[from].load(SeqCst),
        ..Edge::default()
    };
    if (prev.live - b.epoch) / b.q >= 3 * CYCLE {
        dump(b, e)
    }
    b.dispatch(e)
}

struct Uart;
impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        uart_write(s.as_bytes());
        Ok(())
    }
}

fn dump(b: &Bench, terminal: Edge) -> ! {
    let mut u = Uart;
    let _ = writeln!(
        u,
        "OP HEADER table={} epoch={} freq={} q={} n={}",
        if Q_TABLE { "Q" } else { "P" },
        b.epoch,
        b.frequency,
        b.q,
        b.n
    );
    for id in 0..3 {
        let _ = writeln!(
            u,
            "OP STACK id={} lo={} hi={}",
            id,
            stack_top(id) - STACK_BYTES,
            stack_top(id)
        );
    }
    for (n, e) in b.edges[..b.n].iter().enumerate() {
        if n != 0 {
            let mut tick = (e.due - b.epoch) / b.q;
            let live_tick = (e.live - b.epoch) / b.q;
            while tick <= live_tick {
                let slot = b.table.decide(tick);
                let _ = writeln!(
                    u,
                    "OP BOUND n={} tick={} end={} owner={} runnable={} skipped={}",
                    n,
                    tick,
                    slot.end_tick,
                    slot.owner.unwrap_or(IDLE),
                    slot.runnable.unwrap_or(IDLE),
                    u8::from(slot.end_tick <= live_tick)
                );
                tick = slot.end_tick;
                if tick > 4 * CYCLE {
                    break;
                } // bounded diagnostic on excessive trace loss
            }
        }
        let _ = writeln!(
            u,
            "OP TIMER n={} d={} p={} c={} after={}",
            n, e.next, e.program_before, e.readback, e.program_after
        );
        let o = &OBS[n];
        let _ = writeln!(
            u,
            "OP EDGE n={} due={} entry={} live={} ret={} next={} owner={} chosen={} from={} last={} out_count={} out_pc={} out_sp={} out_hash={} in_pc={} in_sp={} in_hash={}",
            n,
            e.due,
            e.entry,
            e.live,
            e.ret,
            e.next,
            e.owner,
            e.chosen,
            e.from,
            e.out_last,
            e.out_count,
            e.out_pc,
            e.out_sp,
            e.out_hash,
            e.in_pc,
            e.in_sp,
            e.in_hash
        );
        let _ = writeln!(
            u,
            "OP OBS n={} x={} task={} sp={} count={} value={}",
            n,
            o.when.load(SeqCst),
            o.owner.load(SeqCst),
            o.sp.load(SeqCst),
            o.count.load(SeqCst),
            o.value.load(SeqCst)
        );
    }
    let _ = writeln!(
        u,
        "OP STOP due={} entry={} from={} last={} out_count={} out_pc={} out_sp={} out_hash={}",
        terminal.due,
        terminal.entry,
        terminal.from,
        terminal.out_last,
        terminal.out_count,
        terminal.out_pc,
        terminal.out_sp,
        terminal.out_hash
    );
    park()
}

extern "C" fn fail() -> ! {
    uart_write(b"OP INVALID\n");
    park()
}
