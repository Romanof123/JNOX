//! Two finite ticket lotteries and a measured-service stride run on one CPU.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::proportional_policy::Proportional;
use super::{park, uart_write};

const WEIGHTS: [u8; 3] = [1, 2, 4];
const SEEDS: [u64; 2] = [1, 2];
const WINDOW: usize = 32;
const STRIDE_SLOTS: usize = 16;
const DELAY_EPOCH: usize = 3;
const ROWS: usize = 2 * WINDOW + STRIDE_SLOTS + 1;
const NONE: u8 = 255;
const SCALE: u64 = 720_720;

unsafe extern "C" {
    fn prop_cpu_a();
    fn prop_cpu_b();
    fn prop_cpu_c();
    fn prop_yielder();
    fn prop_delayed();
    static prop_cpu_a_work: u8;
    static prop_cpu_a_end: u8;
    static prop_cpu_b_work: u8;
    static prop_cpu_b_end: u8;
    static prop_cpu_c_work: u8;
    static prop_cpu_c_end: u8;
    static prop_yield_return: u8;
    static prop_delayed_work: u8;
    static prop_delayed_end: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("proportional.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym yield_call, fail = sym vector_fail, counter_target = sym COUNTER_TARGET,
);

#[repr(C)]
#[derive(Clone, Copy)]
struct Row {
    // The interrupted task's actual return PC, stack frame, counter interval
    // and live progress. Timer deadlines are absolute CNTP_CVAL timestamps.
    pc: u64,
    frame: u64,
    start: u64,
    stop: u64,
    deadline: u64,
    progress: u64,
    normalization: u64,
    pass_start: [u64; 3],
    pass_stop: [u64; 3],
    elapsed: u64,
    epoch: u8,
    slot: u8,
    task: u8,
    ready: u8,
    ppi: u8,
    draw: u8,
    total: u8,
    low: u8,
    high: u8,
    applied: u8, // policy accepted the full elapsed count
    reserved: [u8; 6],
}

#[repr(C)]
struct Witness {
    count: u64,
    ticks: [u64; 2],
    seeds: [u64; 2],
    join_before: [u32; 3],
    join_after: [u32; 3],
    rows: [Row; ROWS],
}

#[unsafe(no_mangle)]
static mut proportional_witness: Witness = Witness {
    count: 0,
    ticks: [0; 2],
    seeds: SEEDS,
    join_before: [0; 3],
    join_after: [0; 3],
    rows: [Row {
        pc: 0,
        frame: 0,
        start: 0,
        stop: 0,
        deadline: 0,
        progress: 0,
        normalization: 0,
        pass_start: [0; 3],
        pass_stop: [0; 3],
        epoch: 0,
        slot: 0,
        task: 0,
        ready: 0,
        elapsed: 0,
        ppi: 0,
        draw: 0,
        total: 0,
        low: 0,
        high: 0,
        applied: 0,
        reserved: [0; 6],
    }; ROWS],
};
const _: () = assert!(size_of::<Row>() == 128);
static COUNTER_TARGET: AtomicU64 = AtomicU64::new(0);
static mut STACKS: [Stack; 12] = [const { Stack([0; STACK_BYTES]) }; 12];
static mut ACTIVE: Option<Bench> = None;

// Little-endian words: source line (commit marker), epoch, slot, actual, expected.
#[cfg(feature = "proportional-diagnostic")]
#[unsafe(no_mangle)]
static mut proportional_failure: [u64; 5] = [0, u64::MAX, u64::MAX, 0, 0];

#[inline]
fn mark_slot(_epoch: usize, _slot: usize) {
    #[cfg(feature = "proportional-diagnostic")]
    unsafe {
        ptr::write_volatile(&raw mut proportional_failure[1], _epoch as u64);
        ptr::write_volatile(&raw mut proportional_failure[2], _slot as u64);
    }
}

#[cold]
#[track_caller]
fn mismatch(_actual: u64, _expected: u64) -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    #[cfg(feature = "proportional-diagnostic")]
    unsafe {
        if ptr::read_volatile(&raw const proportional_failure[0]) == 0 {
            ptr::write_volatile(&raw mut proportional_failure[3], _actual);
            ptr::write_volatile(&raw mut proportional_failure[4], _expected);
            ptr::write_volatile(&raw mut proportional_failure[0],
                core::panic::Location::caller().line() as u64);
        }
    }
    uart_write(b"PROPORTIONAL FAIL\n");
    park()
}

#[track_caller]
fn fail() -> ! {
    mismatch(0, 1)
}

extern "C" fn vector_fail() -> ! {
    fail()
}

#[track_caller]
fn require(ok: bool) {
    if !ok {
        fail()
    }
}

macro_rules! check {
    ($actual:expr, $op:tt, $expected:expr) => {{
        let actual = $actual;
        let expected = $expected;
        if !(actual $op expected) {
            mismatch(actual as u64, expected as u64);
        }
    }};
}

#[track_caller]
fn check_counts(actual: [u64; 3], expected: [u64; 3]) {
    for (observed, target) in actual.into_iter().zip(expected) {
        if observed != target {
            mismatch(observed, target);
        }
    }
}

fn counter() -> u64 {
    let n;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) n, options(nostack, preserves_flags)) };
    n
}

fn disable_timer() {
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    };
}

struct Bench {
    policy: Proportional<3>,
    epoch: usize,
    slot: usize,
    current: Option<usize>,
    frames: [*mut Frame; 3],
    progress: [u64; 3],
    served: [u64; 3],
    selections: [u32; 3],
    ticks: [u64; 2],
    start: u64,
    deadline: u64,
    pass_start: [u64; 3],
}

fn with_bench<R>(operation: impl FnOnce(&mut Bench) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    check!(daif & 0x80, !=, 0);
    // Only the masked CPU in boot or in one of the exception handlers borrows ACTIVE.
    unsafe { operation((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn new(epoch: usize, ticks: [u64; 2]) -> Self {
        let policy = if let Some(&seed) = SEEDS.get(epoch) {
            Proportional::lottery(WEIGHTS, seed)
        } else {
            Proportional::stride(WEIGHTS)
        }
        .unwrap_or_else(|| fail());
        Self {
            policy,
            epoch,
            slot: 0,
            current: None,
            frames: [ptr::null_mut(); 3],
            progress: [0; 3],
            served: [0; 3],
            selections: [0; 3],
            ticks,
            start: 0,
            deadline: 0,
            pass_start: [0; 3],
        }
    }

    fn stride(&self) -> bool {
        self.epoch >= SEEDS.len()
    }

    fn top(&self, id: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.epoch * 3 + id + 1) * STACK_BYTES)
    }

    fn cookie(&self, id: usize) -> u64 {
        0x5100 + (self.epoch * 3 + id) as u64
    }

    fn begin(&mut self) -> *mut Frame {
        mark_slot(self.epoch, 0);
        let first: unsafe extern "C" fn() = if self.epoch == DELAY_EPOCH {
            prop_delayed
        } else if self.stride() {
            prop_yielder
        } else {
            prop_cpu_a
        };
        let entries: [unsafe extern "C" fn(); 3] = [first, prop_cpu_b, prop_cpu_c];
        for (id, entry) in entries.into_iter().enumerate() {
            // Each epoch starts on separate, unused zero-initialized stacks.
            unsafe {
                let f = self.top(id).sub(size_of::<Frame>()).cast::<Frame>();
                (*f).elr = entry as usize as u64;
                (*f).spsr = 0x3c5;
                (*f).gpr[19] = self.cookie(id);
                (*f).gpr[23] = id as u64;
                self.frames[id] = f;
            }
        }
        let ready = match self.epoch {
            0 | 1 => 7,
            2 => 3,
            DELAY_EPOCH => 1,
            _ => fail(),
        };
        require(self.policy.set_ready_mask(ready));
        if !self.stride() {
            // Checking the inclusive first and last ticket of every interval
            // must leave the generator untouched for the published seed.
            for (id, (lo, hi)) in [(0, 1), (1, 3), (3, 7)].into_iter().enumerate() {
                check!(self.policy.winner_for_draw(lo).unwrap_or(usize::MAX), ==, id);
                check!(self.policy.winner_for_draw(hi - 1).unwrap_or(usize::MAX), ==, id);
            }
            check!(self.policy.winner_for_draw(7).unwrap_or(usize::MAX), ==, usize::MAX);
        }
        self.dispatch()
    }

    fn passes(&self) -> [u64; 3] {
        if self.stride() {
            core::array::from_fn(|id| self.policy.pass(id).unwrap_or_else(|| fail()))
        } else {
            [0; 3]
        }
    }

    fn tick(&self) -> u64 {
        self.ticks[usize::from(self.stride())]
    }

    fn dispatch(&mut self) -> *mut Frame {
        mark_slot(self.epoch, self.slot);
        check!(self.current.unwrap_or(usize::MAX), ==, usize::MAX);
        let id = self.policy.dispatch().unwrap_or_else(|| fail());
        check!(self.policy.ready_mask() & (1 << id), !=, 0);
        self.pass_start = self.passes();
        self.current = Some(id);
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        check!(frame.addr(), !=, 0);
        let tick = self.tick();
        let grant = if self.stride() { 2 } else { 1 };
        #[cfg(feature = "proportional-fixture-miss")]
        let grant = if self.epoch == 0 && self.slot == 0 { 2 } else { grant };
        self.start = counter();
        self.deadline = self.start + grant * tick;
        let target_ticks = if self.epoch == DELAY_EPOCH { 3 } else { 1 };
        COUNTER_TARGET.store(self.start + target_ticks * tick, Ordering::Release);
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) self.deadline, in(reg) 1u64, options(nostack, preserves_flags));
        }
        frame
    }

    fn end_slot(&mut self, frame: *mut Frame, stopped: u64, ppi: u8) -> *mut Frame {
        let id = self.current.take().unwrap_or_else(|| fail());
        check!(self.frames[id].addr(), ==, 0);
        check!(frame.addr(), ==, self.top(id).addr() - 16 - size_of::<Frame>());
        let f = unsafe { &*frame };
        let cookie = self.cookie(id);
        check!(f.spsr & 0x8f, ==, 5);
        check!(f.gpr[23], ==, id as u64);
        check!(f.gpr[19], ==, cookie);
        check!(f.tpidr_el1, ==, cookie);
        check!(unsafe { ptr::read_volatile(self.top(id).sub(16).cast::<u64>()) }, ==, cookie);
        let pc = f.elr;
        let progress = f.gpr[20];
        self.frames[id] = frame;
        check!(stopped, >=, self.start);
        let elapsed = (stopped - self.start) / self.tick();
        let ready = self.policy.ready_mask() as u8;
        let (draw, total, low, high, normalization) = if self.stride() {
            let offset = self.policy.last_normalization().unwrap_or_else(|| fail());
            (NONE, 0, 0, 0, offset)
        } else {
            let (draw, total) = self.policy.last_draw().unwrap_or_else(|| fail());
            let low: u32 = WEIGHTS[..id].iter().copied().map(u32::from).sum();
            let high = low + u32::from(WEIGHTS[id]);
            check!(total, ==, 7);
            check!(draw, >=, low);
            check!(draw, <, high);
            (draw as u8, total as u8, low as u8, high as u8, 0)
        };
        // Charge physical residency, including any interrupt-delivery delay.
        // The journal retains full-width timing even if the policy cannot
        // represent the interval, or a normally punctual fixture slot overruns.
        let applied = u32::try_from(elapsed).is_ok_and(|charge| self.policy.stop(id, charge));
        let pass_stop = self.passes();
        let row = Row {
            pc,
            frame: frame.addr() as u64,
            start: self.start,
            stop: stopped,
            deadline: self.deadline,
            progress,
            normalization,
            pass_start: self.pass_start,
            pass_stop,
            epoch: self.epoch as u8,
            slot: self.slot as u8,
            task: id as u8,
            ready,
            elapsed,
            ppi,
            draw,
            total,
            low,
            high,
            applied: u8::from(applied),
            reserved: [0; 6],
        };
        unsafe {
            let index = ptr::read_volatile(&raw const proportional_witness.count) as usize;
            check!(index, <, ROWS);
            ptr::write_volatile(&raw mut proportional_witness.rows[index], row);
            ptr::write_volatile(&raw mut proportional_witness.count, index as u64 + 1);
        }
        require(applied);
        check!(progress, >, self.progress[id]);
        self.progress[id] = progress;
        self.check_interval(&row);
        self.served[id] += elapsed;
        self.selections[id] += 1;
        self.slot += 1;
        if self.epoch == DELAY_EPOCH {
            let target = self.start + 3 * self.tick();
            check!(f.gpr[10], ==, target);
            check!(f.gpr[11], >=, target);
            check!(f.gpr[11], <=, stopped);
            check!(row.ready, ==, 1);
            check!(self.slot, ==, 1);
            check_counts(row.pass_start, [0; 3]);
            check_counts(row.pass_stop, [SCALE * elapsed, 0, 0]);
            uart_write(b"STRIDE DELAY CHARGED\nPROPORTIONAL PASS\n");
            park()
        } else if self.stride() {
            self.check_stride(row);
            if self.slot == STRIDE_SLOTS {
                check_counts(self.served, [2, 4, 8]);
                uart_write(b"STRIDE SERVICE JOIN\n");
                *self = Self::new(DELAY_EPOCH, self.ticks);
                return self.begin();
            }
        } else if self.slot == WINDOW {
            // Compare exact integer fractions. The two independent 32-quantum
            // windows each allow an absolute count deviation of eight quanta.
            check_counts(self.served, self.selections.map(u64::from));
            for (observed, weight) in self.selections.into_iter().zip(WEIGHTS) {
                check!(observed, >, 0);
                check!((observed * 7).abs_diff(WINDOW as u32 * u32::from(weight)), <=, 8 * 7);
            }
            uart_write(if self.epoch == 0 {
                b"LOTTERY SEED A\n"
            } else {
                b"LOTTERY SEED B\n"
            });
            *self = Self::new(self.epoch + 1, self.ticks);
            return self.begin();
        }
        self.dispatch()
    }

    fn check_interval(&self, row: &Row) {
        if row.ppi == NONE {
            check!(self.epoch, ==, 2);
            check!(row.task, ==, 0);
            check!(row.pc, ==, (&raw const prop_yield_return).addr() as u64);
            check!(row.stop, <, row.deadline);
            check!(row.elapsed, ==, 1);
            return;
        }
        check!(u32::from(row.ppi), ==, TIMER_IRQ);
        check!(row.stop, >=, row.deadline);
        let (lo, hi) = match (self.epoch, row.task) {
            (DELAY_EPOCH, 0) => (&raw const prop_delayed_work, &raw const prop_delayed_end),
            (0 | 1, 0) => (&raw const prop_cpu_a_work, &raw const prop_cpu_a_end),
            (_, 1) => (&raw const prop_cpu_b_work, &raw const prop_cpu_b_end),
            (_, 2) => (&raw const prop_cpu_c_work, &raw const prop_cpu_c_end),
            _ => fail(),
        };
        check!(row.pc as usize, >=, lo.addr());
        check!(row.pc as usize, <, hi.addr());
        match self.epoch {
            0 | 1 => check!(row.elapsed, ==, 1),
            2 => check!(row.elapsed, ==, 2),
            DELAY_EPOCH => check!(row.elapsed, >=, 3),
            _ => fail(),
        }
    }

    fn check_stride(&mut self, row: Row) {
        // Verify the policy's choice only after this task actually executed.
        let slot = usize::from(row.slot);
        let expected = if slot < 8 {
            [0, 1][slot % 2]
        } else {
            [0, 1, 2, 2][slot % 4]
        };
        check!(row.task, ==, expected);
        check!(row.ready, ==, if slot < 8 { 3 } else { 7 });
        let mut charged = row.pass_start;
        charged[usize::from(expected)] +=
            row.elapsed * (SCALE / u64::from(WEIGHTS[usize::from(expected)]));
        check_counts(row.pass_stop, charged);
        if slot == 8 {
            check!(row.normalization, ==, SCALE);
            check_counts(row.pass_start, [0; 3]);
        }
        if self.slot == 8 {
            // Before normalization at the next dispatch, the new entrant must
            // join the current floor. A stale zero would put C ahead of A/B.
            check_counts(self.served, [4, 8, 0]);
            check_counts(row.pass_stop, [SCALE, SCALE, 0]);
            require(self.policy.set_ready_mask(7));
            let joined = self.passes();
            check_counts(joined, [SCALE; 3]);
            unsafe {
                ptr::write_volatile(
                    &raw mut proportional_witness.join_before,
                    row.pass_stop
                        .map(|p| u32::try_from(p).unwrap_or_else(|_| fail())),
                );
                ptr::write_volatile(
                    &raw mut proportional_witness.join_after,
                    joined.map(|p| u32::try_from(p).unwrap_or_else(|_| fail())),
                );
            }
            self.served = [0; 3];
        }
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) freq, options(nostack, preserves_flags)) };
    let ticks = [freq / 100, freq / 25];
    check!(ticks[0], !=, 0);
    check!(ticks[1], !=, 0);
    unsafe {
        ptr::write_volatile(&raw mut proportional_witness.ticks, ticks);
        ACTIVE = Some(Bench::new(0, ticks));
        sched_start_frame(with_bench(Bench::begin))
    }
}

extern "C" fn yield_call(frame: *mut Frame) -> *mut Frame {
    let stopped = counter();
    disable_timer();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    check!(esr >> 26, ==, 0x15);
    check!(esr & 0xffff, ==, 1);
    with_bench(|b| b.end_slot(frame, stopped, NONE))
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let stopped = counter();
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    check!(iar & 0x3ff, ==, TIMER_IRQ);
    check!(control & 7, ==, 5);
    disable_timer();
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|b| b.end_slot(frame, stopped, (iar & 0x3ff) as u8))
}
