//! Live cyclic partitions, timed against a single physical-counter epoch.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::context::*;
use super::temporal_policy::CyclicExecutive;
use super::{park, uart_write};

#[cfg(feature = "partition-legacy-diag")]
#[path = "partition_legacy_diag.rs"]
mod diag;

// The default expansion uses the original predicate only. Diagnostic operands
// are evaluated once, including counter and volatile witness samples.
macro_rules! guard {
    ($code:expr, $lhs:expr, $op:tt, $rhs:expr) => {{
        #[cfg(not(feature="partition-legacy-diag"))]
        require($lhs $op $rhs);
        #[cfg(feature="partition-legacy-diag")]
        {
            let (value,expected)=($lhs,$rhs);
            if !(value $op expected) {
                diag::stop($code,value as u64,expected as u64,u64::MAX)
            }
        }
    }}
}
macro_rules! in_range {
    ($code:expr,$range:expr,$sample:expr) => {{
        let range = $range;
        let value = $sample;
        #[cfg(not(feature = "partition-legacy-diag"))]
        require(range.contains(&value));
        #[cfg(feature = "partition-legacy-diag")]
        if !range.contains(&value) {
            diag::stop($code, value as u64, range.start as u64, range.end as u64)
        }
    }};
}
macro_rules! reject {
    ($code:expr,$a:expr,$b:expr,$c:expr) => {{
        #[cfg(feature = "partition-legacy-diag")]
        {
            diag::stop($code, $a as u64, $b as u64, $c as u64)
        }
        #[cfg(not(feature = "partition-legacy-diag"))]
        {
            fail()
        }
    }};
}

const IDLE: usize = 2;
const IRQ_TICKS: [u64; 6] = [2, 3, 4, 6, 7, 8];
const WITNESSES: [u64; 3] = [0xa19, 0xb19, 0xc19];

unsafe extern "C" {
    fn part_a();
    fn part_b();
    fn part_idle();
    static part_a_begin: u8;
    static part_a_end: u8;
    static part_b_begin: u8;
    static part_b_end: u8;
    static part_idle_begin: u8;
    static part_idle_end: u8;
}

global_asm!(
    include_str!("context.S"),
    include_str!("partition.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    diagnostic=const cfg!(feature="partition-legacy-diag") as usize,
    marker = sym task_marker,
    irq = sym timer_interrupt,
    svc = sym unexpected_svc,
    fail = sym fail,
);

static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut BENCH: Option<Bench> = None;

struct Bench {
    policy: CyclicExecutive<3>,
    end_tick: u64,
    irqs: usize,
    ready_b: bool,
    epoch: u64,
    quantum: u64,
    current: usize,
    frames: [*mut Frame; 3],
    interrupted_pc: [u64; 3],
    progress: [u64; 3],
    markers: usize,
}

#[cfg(not(feature = "partition-legacy-diag"))]
fn require(ok: bool) {
    if !ok {
        fail()
    }
}

#[cfg(not(feature = "partition-legacy-diag"))]
use self::fail as unexpected_svc;
#[cfg(feature = "partition-legacy-diag")]
extern "C" fn unexpected_svc() -> ! {
    diag::stop(91, 0, 0, 0)
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    #[cfg(feature = "partition-legacy-diag")]
    diag::stop(90, 0, 0, 0);
    #[cfg(not(feature = "partition-legacy-diag"))]
    {
        uart_write(b"PART FAIL\n");
        park()
    }
}

fn counter() -> u64 {
    let ticks: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) ticks, options(nostack, preserves_flags)) };
    ticks
}

fn stack_top(id: usize) -> *mut u8 {
    guard!(1,id,<=,IDLE);
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

// The sole CPU calls this only at boot, on exception entry or from a task
// marker with IRQs masked. No reference crosses an exception return.
fn with_bench<R>(f: impl FnOnce(&mut Bench) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    guard!(2,daif & 0x80,!=,0);
    unsafe {
        f((*(&raw mut BENCH))
            .as_mut()
            .unwrap_or_else(|| reject!(3, 0, 1, 0)))
    }
}

impl Bench {
    fn boundary(&self, index: u64) -> u64 {
        self.quantum
            .checked_mul(index)
            .and_then(|delta| self.epoch.checked_add(delta))
            .unwrap_or_else(|| reject!(4, index, self.epoch, self.quantum))
    }

    fn dispatch(&mut self, tick: u64, outgoing: Option<*mut Frame>) -> *mut Frame {
        #[cfg(feature = "partition-legacy-diag")]
        diag::boundary_attempt(self.current, tick);
        let decision = self
            .policy
            .decision(tick, |id| id == 0 || (id == 1 && self.ready_b))
            .unwrap_or_else(|| reject!(5, tick, self.irqs, self.ready_b));
        let id = decision.runnable.unwrap_or(IDLE);
        let (owner, expected) = match tick {
            0 | 4 => (Some(0), 0),
            2 => (Some(1), IDLE),
            6 => (Some(1), 1),
            3 | 7 => (None, IDLE),
            _ => reject!(6, tick, self.irqs, 0),
        };
        guard!(7,decision.owner.unwrap_or(usize::MAX),==,owner.unwrap_or(usize::MAX));
        guard!(8,id,==,expected);
        if let Some(frame) = outgoing {
            self.record_interrupt(frame, id != self.current);
        }
        self.end_tick = decision.end_tick;
        guard!(9,IRQ_TICKS.get(self.irqs).copied().unwrap_or(u64::MAX),==,self.end_tick);
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        guard!(10,frame.addr(),!=,0);
        if self.interrupted_pc[id] != 0 {
            guard!(11,unsafe { (*frame).elr },==,self.interrupted_pc[id]);
        }
        self.current = id;
        let deadline = self.boundary(self.end_tick);
        #[cfg(feature = "partition-legacy-diag")]
        diag::window(id, decision.owner, tick, self.end_tick);
        guard!(12,counter(),<,deadline);
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) deadline, in(reg) 1u64, options(nostack, preserves_flags))
        };
        #[cfg(feature = "partition-legacy-diag")]
        diag::armed(deadline);
        frame
    }

    fn record_interrupt(&mut self, frame: *mut Frame, switching: bool) {
        let id = self.current;
        guard!(13,self.frames[id].addr(),==,0);
        guard!(14,frame.addr(),==,stack_top(id).addr()-16-size_of::<Frame>());
        let saved = unsafe { &*frame };
        let (begin, end) = match id {
            0 => (&raw const part_a_begin, &raw const part_a_end),
            1 => (&raw const part_b_begin, &raw const part_b_end),
            _ => (&raw const part_idle_begin, &raw const part_idle_end),
        };
        in_range!(15, begin.addr()..end.addr(), saved.elr as usize);
        let witness = WITNESSES[id];
        let count = saved.gpr[20];
        guard!(16,saved.spsr & 0x8f,==,5);
        guard!(17,saved.gpr[19],==,witness);
        guard!(18,saved.tpidr_el1,==,witness);
        guard!(19,count>>32,==,witness);
        guard!(20,count as u32,>,0);
        guard!(21,count,>,self.progress[id]);
        // x20 and these two words belong to the outgoing context. Publish
        // its exact progress for the assembly restore witness on redispatch.
        unsafe {
            let words = stack_top(id).sub(16).cast::<u64>();
            guard!(22,ptr::read_volatile(words),==,witness);
            if switching {
                ptr::write_volatile(words.add(1), count);
            }
        }
        self.frames[id] = frame;
        self.interrupted_pc[id] = saved.elr;
        self.progress[id] = count;
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let policy = CyclicExecutive::new(2, 4, [(Some(0), 2), (Some(1), 1), (None, 1)])
        .unwrap_or_else(|| reject!(23, 2, 4, 3));
    let entries: [unsafe extern "C" fn(); 3] = [part_a, part_b, part_idle];
    let frames = core::array::from_fn(|id| unsafe {
        let frame = stack_top(id).sub(size_of::<Frame>()).cast::<Frame>();
        (*frame).elr = entries[id] as usize as u64;
        (*frame).spsr = 0x3c5; // EL1h, masked until task witnesses are live
        frame
    });
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let quantum = frequency / 20;
    guard!(24,quantum,>,0);
    unsafe {
        BENCH = Some(Bench {
            policy,
            end_tick: 0,
            irqs: 0,
            ready_b: false,
            frames,
            epoch: counter(),
            quantum,
            current: IDLE,
            interrupted_pc: [0; 3],
            progress: [0; 3],
            markers: 0,
        });
        sched_start_frame(with_bench(|bench| bench.dispatch(0, None)))
    }
}

extern "C" fn task_marker(id: usize, resumed: usize, sp: usize) {
    with_bench(|bench| {
        guard!(25,bench.current,==,id);
        guard!(26,sp,==,stack_top(id).addr()-16*resumed);
        let line: &[u8] = match (bench.irqs, bench.markers, id, resumed) {
            (0, 0, 0, 0) => b"PART A FIRST\n",
            (1, 1, IDLE, 0) => b"PART IDLE FIRST\n",
            (3, 2, 0, 1) => b"PART A RESUMED\n",
            (4, 3, 1, 0) => b"PART B FIRST\n",
            (5, 4, IDLE, 1) => b"PART IDLE RESUMED\n",
            _ => reject!(27, bench.irqs, bench.markers, (id << 1) | resumed),
        };
        #[cfg(feature = "partition-legacy-diag-negative")]
        if bench.markers == 0 {
            guard!(99,bench.markers,==,1)
        }
        bench.markers += 1;
        uart_write(line);
    });
}

extern "C" fn timer_interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    guard!(28,iar & 0x3ff,==,TIMER_IRQ);
    guard!(29,control & 7,==,5);
    // Mask and disable the level source before writing the full EOI token.
    unsafe {
        asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy", in(reg) 2u64, options(nostack, preserves_flags))
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|bench| {
        let tick = bench.end_tick;
        guard!(30,IRQ_TICKS.get(bench.irqs).copied().unwrap_or(u64::MAX),==,tick);
        in_range!(
            31,
            bench.boundary(tick)..bench.boundary(tick + 1),
            counter()
        );
        bench.irqs += 1;
        if tick == 8 {
            bench.record_interrupt(frame, false);
            guard!(32,bench.irqs,==,6);
            guard!(33,bench.markers,==,5);
            guard!(34,bench.frames.iter().all(|frame| !frame.is_null()),==,true);
            guard!(35,bench.progress.iter().all(|&count| count != 0),==,true);
            uart_write(b"PART STOP\nPART PASS\n");
            park()
        }
        if tick == 3 {
            guard!(36,bench.current,==,IDLE);
            guard!(37,bench.markers,==,2);
            guard!(38,bench.ready_b,==,false);
            bench.ready_b = true;
            uart_write(b"PART B READY\n");
        }
        bench.dispatch(tick, Some(frame))
    })
}
