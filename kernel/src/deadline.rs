//! Online EDF and LLF over identical initial jobs. The hardware grid uses a
//! fixed interval of CNTFRQ / 10 counter ticks from each epoch's origin.
//! Deadlines, estimates and completion timestamps are workload inputs.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::deadline_policy::{DeadlinePolicy, DeadlineScheduler};
use super::{park, uart_write};
use DeadlinePolicy::{Edf, Llf};

const A: usize = 2;
const B: usize = 1;
const C: usize = 0;
const TASKS: usize = 3;

static mut STACKS: [[Stack; TASKS]; 2] = [const { [const { Stack([0; STACK_BYTES]) }; TASKS] }; 2];
static TICK: AtomicU64 = AtomicU64::new(0);
static mut EPOCH: Option<Epoch> = None;

// The linked symbol lets a debugger read the clock and IRQ samples from the
// completed guest. Fields are consecutive u64 words, EDF record first.
#[repr(C)]
struct ClockEvidence {
    frequency: u64,
    origin: u64,
    quantum: u64,
    observed: [u64; 7],
}

#[unsafe(export_name = "deadline_clock_evidence")]
static mut CLOCK_EVIDENCE: [ClockEvidence; 2] = [const {
    ClockEvidence {
        frequency: 0,
        origin: 0,
        quantum: 0,
        observed: [0; 7],
    }
}; 2];

unsafe extern "C" {
    fn deadline_a_entry();
    fn deadline_b_entry();
    fn deadline_c_entry();
    static deadline_edf_a_begin: u8;
    static deadline_edf_a_end: u8;
    static deadline_llf_b_hold_begin: u8;
    static deadline_llf_b_hold_end: u8;
    static deadline_llf_b_cross_begin: u8;
    static deadline_llf_b_cross_end: u8;
    static deadline_llf_b_negative_begin: u8;
    static deadline_llf_b_negative_end: u8;
    static deadline_a_after_exit: u8;
    static deadline_b_after_exit: u8;
    static deadline_c_after_exit: u8;
}

global_asm!(
    include_str!("context.S"),
    include_str!("deadline.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    a = const A,
    b = const B,
    c = const C,
    tick = sym TICK,
    marker = sym task_marker,
    irq = sym timer_interrupt,
    svc = sym complete_task,
    fail = sym fail,
);

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"DEADLINE FAIL\n");
    park()
}

fn physical_counter() -> u64 {
    let now: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) now, options(nostack, preserves_flags)) };
    now
}

#[inline(never)]
fn arm_compare(boundary: u64) {
    unsafe {
        asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
            in(reg) boundary, in(reg) 1u64, options(nostack, preserves_flags))
    };
}

struct Epoch {
    policy: DeadlinePolicy,
    scheduler: DeadlineScheduler<TASKS>,
    frames: [*mut Frame; TASKS],
    current: Option<usize>,
    interrupted_pc: u64,
    completed: usize,
    entered: [bool; TASKS],
    worker_reports: usize,
    origin: u64,
    quantum: u64,
}

fn with_epoch<R>(operation: impl FnOnce(&mut Epoch) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    // SAFETY: Boot, exception entry and task reports mask IRQs on the sole
    // CPU. Every borrow ends before IRQ re-enable or exception return.
    unsafe { operation((*(&raw mut EPOCH)).as_mut().unwrap_or_else(|| fail())) }
}

impl Epoch {
    fn new(policy: DeadlinePolicy) -> Self {
        Self {
            policy,
            scheduler: DeadlineScheduler::new(policy, 0, 1).unwrap_or_else(|| fail()),
            frames: [ptr::null_mut(); TASKS],
            current: None,
            interrupted_pc: 0,
            completed: 0,
            entered: [false; TASKS],
            worker_reports: 0,
            origin: 0,
            quantum: 0,
        }
    }

    fn worker(&self) -> usize {
        match self.policy {
            Edf => A,
            Llf => B,
        }
    }

    fn stack_slot(&self, id: usize) -> usize {
        require(id < TASKS);
        usize::from(self.policy == Llf) * TASKS + id
    }

    fn stack_top(&self, id: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.stack_slot(id) + 1) * STACK_BYTES)
    }

    fn clock_evidence(&self) -> *mut ClockEvidence {
        (&raw mut CLOCK_EVIDENCE)
            .cast::<ClockEvidence>()
            .wrapping_add(usize::from(self.policy == Llf))
    }

    fn witness(&self, id: usize) -> u64 {
        0xde00 + 16 * self.stack_slot(id) as u64
    }

    fn release(&mut self, now: u64, id: usize, estimate: u64, deadline: u64) {
        require(!self.entered[id] && self.frames[id].is_null());
        require(self.scheduler.release(now, id, estimate, deadline).is_ok());
        let entry: unsafe extern "C" fn() = match id {
            A => deadline_a_entry,
            B => deadline_b_entry,
            C => deadline_c_entry,
            _ => fail(),
        };
        let worker = id == self.worker();
        // SAFETY: Each zero-initialized stack is first touched upon release of
        // its unique epoch/job. The old epoch's final SVC uses a separate stack.
        unsafe {
            let frame = self.stack_top(id).sub(size_of::<Frame>()).cast::<Frame>();
            (*frame).elr = entry as usize as u64;
            (*frame).spsr = if worker { 0x3c5 } else { 0x345 }; // EL1h
            (*frame).gpr[19] = self.witness(id);
            (*frame).gpr[23] = u64::from(worker); // Exercise the designated task's non-yielding loop.
            self.frames[id] = frame;
        }
    }

    fn select(&mut self, now: u64) -> Option<*mut Frame> {
        self.current = self.scheduler.decide(now).unwrap_or_else(|_| fail());
        let id = self.current?;
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        if id == self.worker() && self.interrupted_pc != 0 {
            require(frame.addr() == self.stack_top(id).addr() - 16 - size_of::<Frame>());
            require(unsafe { (*frame).elr } == self.interrupted_pc);
        }
        Some(frame)
    }

    fn start(&mut self) -> *mut Frame {
        let ctl: u64;
        unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) };
        require(ctl & 1 == 0);
        TICK.store(0, Ordering::Release);
        self.release(0, A, 1, 8);
        self.release(0, B, 8, 10);
        let first = self.select(0).unwrap_or_else(|| fail());
        require(self.current == Some(self.worker()));
        let frequency: u64;
        unsafe {
            asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags))
        };
        self.quantum = frequency / 10;
        require(self.quantum != 0);
        self.origin = physical_counter();
        // SAFETY: The single CPU initializes its epoch's evidence before the first IRQ.
        unsafe {
            ptr::write_volatile(
                self.clock_evidence(),
                ClockEvidence {
                    frequency,
                    origin: self.origin,
                    quantum: self.quantum,
                    observed: [0; 7],
                },
            );
        }
        arm_compare(self.boundary(1));
        first
    }

    fn boundary(&self, tick: u64) -> u64 {
        self.quantum
            .checked_mul(tick)
            .and_then(|offset| self.origin.checked_add(offset))
            .unwrap_or_else(|| fail())
    }

    fn check_frame(&self, id: usize, frame: *mut Frame) {
        require(self.current == Some(id) && self.entered[id] && self.frames[id].is_null());
        require(frame.addr() == self.stack_top(id).addr() - 16 - size_of::<Frame>());
        let witness = self.witness(id);
        // SAFETY: We verified the fixed vector frame and the two words that
        // the running task reserved at the top of its own stack.
        let saved = unsafe { &*frame };
        let word = unsafe { ptr::read_volatile(self.stack_top(id).sub(16).cast::<u64>()) };
        require(saved.spsr & 0x8f == 5 && saved.gpr[19] == witness && word == witness);
    }

    fn save_interrupt(&mut self, tick: u64, frame: *mut Frame) {
        let id = self.worker();
        self.check_frame(id, frame);
        let (begin, end) = match (self.policy, tick) {
            (Edf, 1) => (
                &raw const deadline_edf_a_begin,
                &raw const deadline_edf_a_end,
            ),
            (Llf, 1..=5) => (
                &raw const deadline_llf_b_hold_begin,
                &raw const deadline_llf_b_hold_end,
            ),
            (Llf, 6) => (
                &raw const deadline_llf_b_cross_begin,
                &raw const deadline_llf_b_cross_end,
            ),
            (Llf, 7) => (
                &raw const deadline_llf_b_negative_begin,
                &raw const deadline_llf_b_negative_end,
            ),
            _ => fail(),
        };
        let saved = unsafe { &*frame };
        require((begin.addr()..end.addr()).contains(&(saved.elr as usize)));
        let tag = self.witness(id) + tick;
        let count = saved.gpr[20];
        require(saved.gpr[22] == tick && saved.gpr[21] == tag);
        require(count >> 32 == tag && count as u32 != 0);
        require(saved.simd[31] == [tag; 2] && saved.tpidr_el1 == tag);
        unsafe { ptr::write_volatile(self.stack_top(id).sub(8).cast::<u64>(), count) };
        self.interrupted_pc = saved.elr;
        self.frames[id] = frame;
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    unsafe {
        EPOCH = Some(Epoch::new(Edf));
        sched_start_frame(with_epoch(Epoch::start))
    }
}

extern "C" fn task_marker(id: usize, stage: usize, sp: usize) {
    let daif: u64;
    unsafe {
        asm!("mrs {}, daif", "msr daifset, #2", out(reg) daif, options(nostack, preserves_flags))
    };
    with_epoch(|e| {
        require(e.current == Some(id) && sp == e.stack_top(id).addr() - 16);
        if stage == 0 {
            require(!e.entered[id]);
            e.entered[id] = true;
        } else {
            require(id == e.worker() && e.entered[id] && stage == e.worker_reports + 1);
            let (tick, completed) = match (e.policy, stage) {
                (Edf, 1) => (1, 1),
                (Llf, 1) => (5, 0),
                (Llf, 2) => (6, 1),
                (Llf, 3) => (7, 2),
                _ => fail(),
            };
            require(TICK.load(Ordering::Acquire) == tick && e.completed == completed);
            e.worker_reports = stage;
        }
        let line: &[u8] = match (e.policy, id, stage) {
            (Edf, A, 0) => b"DEADLINE EDF A FIRST\n",
            (Edf, C, 0) => b"DEADLINE EDF C FIRST\n",
            (Edf, A, 1) => b"DEADLINE EDF A RESUMED\n",
            (Edf, B, 0) => b"DEADLINE EDF B FIRST\n",
            (Llf, B, 0) => b"DEADLINE LLF B FIRST\n",
            (Llf, B, 1) => b"DEADLINE LLF B TIE RETAINED\n",
            (Llf, A, 0) => b"DEADLINE LLF A FIRST\n",
            (Llf, B, 2) => b"DEADLINE LLF B RESUMED T6\n",
            (Llf, C, 0) => b"DEADLINE LLF C FIRST\n",
            (Llf, B, 3) => b"DEADLINE LLF B RESUMED T7\n",
            _ => fail(),
        };
        uart_write(line);
    });
    unsafe { asm!("msr daif, {}", in(reg) daif, options(nostack, preserves_flags)) };
}

extern "C" fn timer_interrupt(frame: *mut Frame) -> *mut Frame {
    let observed = physical_counter();
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    let programmed: u64;
    unsafe {
        asm!("mrs {}, cntp_cval_el0", out(reg) programmed, options(nostack, preserves_flags))
    };
    // Deassert CNTP before returning the complete GIC acknowledgement token.
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_epoch(|e| {
        let tick = TICK.load(Ordering::Acquire) + 1;
        let boundary = e.boundary(tick);
        let next_boundary = e.boundary(tick + 1);
        require(programmed == boundary && (boundary..next_boundary).contains(&observed));
        e.save_interrupt(tick, frame);
        // save_interrupt accepted only EDF tick 1 or LLF ticks 1..=7. Each
        // single-writer sample survives optimization for linked-ELF inspection.
        unsafe {
            let samples = (&raw mut (*e.clock_evidence()).observed).cast::<u64>();
            ptr::write_volatile(samples.add((tick - 1) as usize), observed);
        }
        require(e.scheduler.now() == tick - 1 && e.scheduler.remaining(C).is_none());
        match (e.policy, tick) {
            (Edf, 1) => e.release(tick, C, 3, 3),
            (Llf, 7) => e.release(tick, C, 3, 8),
            (Llf, 1..=6) => (),
            _ => fail(),
        }
        let next = e.select(tick).unwrap_or_else(|| fail());
        let expected = match (e.policy, tick) {
            (Edf, 1) => {
                uart_write(b"DEADLINE EDF IRQ C READY\n");
                C
            }
            (Llf, 1..=6) => {
                require(
                    e.scheduler.remaining(B) == Some(8 - tick) && e.scheduler.laxity(B) == Some(2),
                );
                require(e.scheduler.laxity(A) == Some(7 - i128::from(tick)));
                if tick == 6 {
                    uart_write(b"DEADLINE LLF T6 CROSSOVER\n");
                    A
                } else {
                    B
                }
            }
            (Llf, 7) => {
                require(e.scheduler.laxity(C) == Some(-2) && e.scheduler.laxity(B) == Some(2));
                uart_write(b"DEADLINE LLF IRQ C READY\n");
                C
            }
            _ => fail(),
        };
        require(e.current == Some(expected));
        if expected == e.worker() {
            require(next == frame);
        } else {
            require(next != frame && e.frames[e.worker()] == frame);
        }
        if e.policy == Llf && tick < 7 {
            require(physical_counter() < next_boundary);
            arm_compare(next_boundary);
        }
        TICK.store(tick, Ordering::Release);
        next
    })
}

extern "C" fn complete_task(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15 && esr & 0x01ff_ffff == 1);
    with_epoch(|e| {
        let id = e.current.unwrap_or_else(|| fail());
        e.check_frame(id, frame);
        let site = match id {
            A => &raw const deadline_a_after_exit,
            B => &raw const deadline_b_after_exit,
            C => &raw const deadline_c_after_exit,
            _ => fail(),
        };
        require(unsafe { (*frame).elr as usize } == site.addr());
        // A task's real SVC supplies completion. These logical times define
        // the workload, including the two quick jobs completing within a tick.
        let (expected_job, tick, expected_next) = match (e.policy, e.completed) {
            (Edf, 0) => (C, 1, Some(A)),
            (Edf, 1) => (A, 2, Some(B)),
            (Edf, 2) => (B, 3, None),
            (Llf, 0) => (A, 6, Some(B)),
            (Llf, 1) => (C, 7, Some(B)),
            (Llf, 2) => (B, 8, None),
            _ => fail(),
        };
        require(id == expected_job && e.scheduler.complete(tick, id).is_ok());
        e.completed += 1;
        let next = e.select(tick);
        require(e.current == expected_next);
        if let Some(frame) = next {
            return frame;
        }
        let (last_irq, reports) = match e.policy {
            Edf => (1, 1),
            Llf => (7, 3),
        };
        require(TICK.load(Ordering::Acquire) == last_irq && e.worker_reports == reports);
        if e.policy == Edf {
            *e = Epoch::new(Llf);
            return e.start();
        }
        uart_write(b"DEADLINE PASS\n");
        park()
    })
}
