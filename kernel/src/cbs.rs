//! Five live, integer-clock hard CBS grants on a single EL1 CPU.
//! Absolute compare values keep exception and UART overhead inside each slot.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::context::*;
use super::temporal_policy::HardCbs;
use super::{park, uart_write};

const IDLE: usize = 2;
const EXPECTED: [usize; 5] = [0, 1, IDLE, 0, 1];
const WITNESSES: [u64; 3] = [0xa19, 0xb19, 0xc19];

unsafe extern "C" {
    fn cbs_a();
    fn cbs_b();
    fn cbs_idle();
    static cbs_a_begin: u8;
    static cbs_a_end: u8;
    static cbs_b_begin: u8;
    static cbs_b_end: u8;
    static cbs_idle_begin: u8;
    static cbs_idle_end: u8;
}

global_asm!(
    include_str!("context.S"),
    include_str!("cbs.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    marker = sym task_marker,
    irq = sym timer_interrupt,
    svc = sym unexpected_sync,
    fail = sym fail, diagnostic = const cfg!(jnox_cbs_condition) as u8,
);

static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut BENCH: Option<Bench> = None;

struct Bench {
    policy: HardCbs<2>,
    epoch: u64,
    quantum: u64,
    current: usize,
    frames: [*mut Frame; 3],
    interrupted_pc: [u64; 3],
    progress: [u64; 3],
    markers: usize,
}
#[cfg(not(jnox_cbs_condition))]
fn require(ok: bool) {
    if !ok {
        fail()
    }
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    choose!({ uart_write(b"CBS FAIL\n"); park() };
        condition::no_operands(1))
}

fn counter() -> u64 {
    let ticks: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) ticks, options(nostack, preserves_flags)) };
    ticks
}

fn stack_top(id: usize) -> *mut u8 {
    verify!(id <= IDLE; condition::at_most(3, id as u64, IDLE as u64));
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

// The sole CPU calls this only at boot, on exception entry or from a task
// marker with IRQs masked. No reference crosses an exception return.
fn with_bench<R>(f: impl FnOnce(&mut Bench) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    verify!(daif & 0x80 != 0; condition::irq_masked(daif));
    unsafe { f((*(&raw mut BENCH)).as_mut().unwrap_or_else(|| choose!(fail(); condition::no_operands(5)))) }
}

impl Bench {
    fn boundary(&self, index: u64) -> u64 {
        choose!(self.quantum
            .checked_mul(index)
            .and_then(|delta| self.epoch.checked_add(delta))
            .unwrap_or_else(|| fail()); condition::boundary(self.epoch, self.quantum, index))
    }

    fn dispatch(&mut self) -> *mut Frame {
        let slot = self.policy.now() as usize;
        let id = self
            .policy
            .tick()
            .unwrap_or_else(|_error| choose!(fail(); condition::tick_error(slot, _error)))
            .unwrap_or(IDLE);
        verify!(id == EXPECTED[slot]; condition::equal(36, id as u64, EXPECTED[slot] as u64));
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        verify!(!frame.is_null(); condition::incoming_frame(frame));
        if self.interrupted_pc[id] != 0 {
            verify!(unsafe { (*frame).elr } == self.interrupted_pc[id]; condition::equal(38, unsafe { (*frame).elr }, self.interrupted_pc[id]));
        }
        self.current = id;
        let deadline = self.boundary(self.policy.now());
        verify!(counter() < deadline; condition::before_deadline(counter(), deadline));
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) deadline, in(reg) 1u64, options(nostack, preserves_flags))
        };
        frame
    }

    fn record_interrupt(&mut self, frame: *mut Frame) {
        let id = self.current;
        verify!(self.frames[id].is_null() && self.markers as u64 == self.policy.now(); condition::outgoing_owner(self, id));
        verify!(frame.addr() == stack_top(id).addr() - 16 - size_of::<Frame>(); condition::equal(25, frame.addr() as u64, (stack_top(id).addr() - 16 - size_of::<Frame>()) as u64));
        let saved = unsafe { &*frame };
        let (begin, end) = match id {
            0 => (&raw const cbs_a_begin, &raw const cbs_a_end),
            1 => (&raw const cbs_b_begin, &raw const cbs_b_end),
            _ => (&raw const cbs_idle_begin, &raw const cbs_idle_end),
        };
        verify!((begin.addr()..end.addr()).contains(&(saved.elr as usize)); condition::range(26, begin.addr() as u64, end.addr() as u64, saved.elr));
        let witness = WITNESSES[id];
        let count = saved.gpr[20];
        verify!(saved.spsr & 0x8f == 5 && saved.gpr[19] == witness && saved.tpidr_el1 == witness; condition::registers(saved, witness));
        verify!(count >> 32 == witness && count as u32 > 0 && count > self.progress[id]; condition::progress(self, id, count, witness));
        // x20 and these two words belong to the outgoing context. Publish
        // its exact progress for the assembly restore witness on redispatch.
        unsafe {
            let words = stack_top(id).sub(16).cast::<u64>();
            verify!(ptr::read_volatile(words) == witness; condition::equal(33, ptr::read_volatile(words), witness));
            verify!(ptr::read_volatile(words.add(1)) == self.progress[id]; condition::equal(34, ptr::read_volatile(words.add(1)), self.progress[id]));
            ptr::write_volatile(words.add(1), count);
        }
        self.frames[id] = frame;
        self.interrupted_pc[id] = saved.elr;
        self.progress[id] = count;
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let mut policy = HardCbs::new(0, [(1, 3), (1, 4)]).unwrap_or_else(|| choose!(fail(); condition::no_operands(6)));
    for id in 0..2 {
        choose!(require(policy.release(id, 3).is_ok()); condition::release(&mut policy, id, 3));
    }
    let entries: [unsafe extern "C" fn(); 3] = [cbs_a, cbs_b, cbs_idle];
    let frames = core::array::from_fn(|id| unsafe {
        let frame = stack_top(id).sub(size_of::<Frame>()).cast::<Frame>();
        (*frame).elr = entries[id] as usize as u64;
        (*frame).spsr = 0x3c5; // EL1h, masked until task witnesses are live
        frame
    });
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let quantum = frequency / 20;
    verify!(quantum > 0; condition::positive_quantum(quantum));
    unsafe {
        BENCH = Some(Bench {
            policy,
            frames,
            epoch: counter(),
            quantum,
            current: IDLE,
            interrupted_pc: [0; 3],
            progress: [0; 3],
            markers: 0,
        });
        sched_start_frame(with_bench(Bench::dispatch))
    }
}

extern "C" fn task_marker(id: usize, resumed: usize, sp: usize) {
    with_bench(|bench| {
        let slot = bench.markers;
        verify!(EXPECTED.get(slot) == Some(&id) && bench.current == id; condition::marker_owner(bench, slot, id));
        verify!(resumed == usize::from(slot >= 3); condition::equal(53, resumed as u64, u64::from(slot >= 3)));
        verify!(sp == stack_top(id).addr() - 16 * resumed; condition::equal(54, sp as u64, (stack_top(id).addr() - 16 * resumed) as u64));
        let line: &[u8] = match (id, resumed) {
            (0, 0) => b"CBS A FIRST\n",
            (1, 0) => b"CBS B FIRST\n",
            (IDLE, 0) => b"CBS IDLE\n",
            (0, 1) => b"CBS A RESUMED\n",
            (1, 1) => b"CBS B RESUMED\n",
            _ => choose!(fail(); condition::pair(55, id as u64, resumed as u64)),
        };
        bench.markers += 1;
        uart_write(line);
    });
}

extern "C" fn timer_interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    verify!(iar & 0x3ff == TIMER_IRQ && control & 7 == 5; condition::timer_source(iar, control));
    // Mask and disable the level source before writing the full EOI token.
    unsafe {
        asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy", in(reg) 2u64, options(nostack, preserves_flags))
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|bench| {
        let boundary = bench.policy.now();
        verify!((bench.boundary(boundary)..bench.boundary(boundary + 1)).contains(&counter()); condition::range(22, bench.boundary(boundary), bench.boundary(boundary + 1), counter()));
        bench.record_interrupt(frame);
        if boundary == EXPECTED.len() as u64 {
            verify!(bench.frames.iter().all(|frame| !frame.is_null()); condition::terminal_frames(bench));
            verify!(bench.progress.iter().all(|&count| count != 0); condition::terminal_progress(bench));
            uart_write(b"CBS TIMER STOP\nCBS PASS\n");
            park()
        }
        bench.dispatch()
    })
}

// Select at expansion time. On the ordinary image the compiler sees the original
// predicate and its source span. In particular a counter, IAR or volatile load
// appearing in both alternatives is evaluated by exactly one alternative.
mod guard_selection {
    #[cfg(not(jnox_cbs_condition))]
    macro_rules! choose { ($plain:expr; $observed:expr) => { $plain }; }
    #[cfg(jnox_cbs_condition)]
    macro_rules! choose { ($plain:expr; $observed:expr) => { $observed }; }
    macro_rules! verify {
        ($original:expr; $observed:expr) => { choose!(require($original); $observed) };
    }
    pub(super) use {choose, verify};
}
use guard_selection::{choose, verify};

#[cfg(jnox_cbs_condition)]
#[path = "cbs_condition.rs"]
mod condition;
#[cfg(jnox_cbs_condition)]
use condition::unexpected_sync;
#[cfg(not(jnox_cbs_condition))]
use fail as unexpected_sync;
