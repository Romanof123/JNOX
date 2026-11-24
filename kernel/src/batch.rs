//! Three batch scheduling workloads on one QEMU virt CPU. Only the arrival
//! interrupt is timed by CNTP. Service estimates, arrivals and completions use
//! workload-defined logical ticks, independent of host speed or measured WCET.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::batch_policy::{BatchPolicy, BatchScheduler};
use super::context::*;
use super::{park, uart_write};
use BatchPolicy::{Hrrn, Sjf, Srtf};

// Descending arrival IDs make incidental table-index ordering visible.
const A: usize = 2;
const B: usize = 1;
const C: usize = 0;
const TASKS: usize = 3;
const EPOCHS: usize = 3;

static mut STACKS: [[Stack; TASKS]; EPOCHS] =
    [const { [const { Stack([0; STACK_BYTES]) }; TASKS] }; EPOCHS];
static RELEASED: AtomicU64 = AtomicU64::new(0);
static mut EPOCH: Option<Epoch> = None;

unsafe extern "C" {
    fn batch_a_entry();
    fn batch_b_entry();
    fn batch_c_entry();
    static batch_a_work_begin: u8;
    static batch_a_work_end: u8;
    static batch_a_after_exit: u8;
    static batch_b_after_exit: u8;
    static batch_c_after_exit: u8;
}

global_asm!(
    include_str!("context.S"),
    include_str!("batch.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    a = const A,
    b = const B,
    c = const C,
    released = sym RELEASED,
    marker = sym task_marker,
    irq = sym release_interrupt,
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
    uart_write(b"BATCH FAIL\n");
    park()
}

struct Epoch {
    policy: BatchPolicy,
    scheduler: BatchScheduler<TASKS>,
    frames: [*mut Frame; TASKS],
    current: Option<usize>,
    interrupted_pc: u64,
    completed: usize,
    entered: [bool; TASKS],
    a_returned: bool,
}

fn with_epoch<R>(operation: impl FnOnce(&mut Epoch) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    // SAFETY: Boot/exception entry or a task marker masked IRQs on the single
    // CPU. No borrow survives exception return or IRQ re-enable.
    unsafe { operation((*(&raw mut EPOCH)).as_mut().unwrap_or_else(|| fail())) }
}

impl Epoch {
    fn new(policy: BatchPolicy) -> Self {
        Self {
            policy,
            scheduler: BatchScheduler::new(policy, 0),
            frames: [ptr::null_mut(); TASKS],
            current: None,
            interrupted_pc: 0,
            completed: 0,
            entered: [false; TASKS],
            a_returned: false,
        }
    }

    fn stack_slot(&self, id: usize) -> usize {
        require(id < TASKS);
        let epoch = match self.policy {
            Sjf => 0,
            Srtf => 1,
            Hrrn => 2,
        };
        epoch * TASKS + id
    }

    fn stack_top(&self, id: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.stack_slot(id) + 1) * STACK_BYTES)
    }

    fn witness(&self, id: usize) -> u64 {
        0xb190 + self.stack_slot(id) as u64
    }

    fn release(&mut self, now: u64, id: usize, estimate: u64) {
        require(self.frames[id].is_null() && !self.entered[id]);
        require(self.scheduler.release(now, id, estimate).is_ok());
        let entry: unsafe extern "C" fn() = match id {
            A => batch_a_entry,
            B => batch_b_entry,
            C => batch_c_entry,
            _ => fail(),
        };
        // SAFETY: This stack belongs exclusively to a newly released job in
        // this epoch. Its backing BSS is zero and has never been used.
        unsafe {
            let frame = self.stack_top(id).sub(size_of::<Frame>()).cast::<Frame>();
            (*frame).elr = entry as usize as u64;
            (*frame).spsr = if id == A { 0x3c5 } else { 0x345 }; // EL1h, A unmasks during work.
            (*frame).gpr[19] = self.witness(id);
            self.frames[id] = frame;
        }
    }

    fn select(&mut self, now: u64) -> Option<*mut Frame> {
        self.current = self.scheduler.decide(now).unwrap_or_else(|_| fail());
        let id = self.current?;
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        if id == A && self.interrupted_pc != 0 {
            require(frame.addr() == self.stack_top(A).addr() - 16 - size_of::<Frame>());
            require(unsafe { (*frame).elr } == self.interrupted_pc);
        }
        Some(frame)
    }

    fn start(&mut self) -> *mut Frame {
        // The old one-shot is disabled and acknowledged before this epoch
        // starts. The new A enters with IRQ masked and arms its own deadline.
        let ctl: u64;
        unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) };
        require(ctl & 1 == 0);
        RELEASED.store(0, Ordering::Release);
        if self.policy == Hrrn {
            self.release(0, A, 2);
            self.release(0, B, 5);
        } else {
            self.release(0, A, 5);
        }
        let frame = self.select(0).unwrap_or_else(|| fail());
        require(self.current == Some(A));
        frame
    }

    fn check_exception(&self, id: usize, frame: *mut Frame) {
        require(self.current == Some(id) && self.entered[id] && self.frames[id].is_null());
        require(frame.addr() == self.stack_top(id).addr() - 16 - size_of::<Frame>());
        let witness = self.witness(id);
        // SAFETY: The address above identifies the complete vector frame and
        // the two task-owned words immediately above it.
        let saved = unsafe { &*frame };
        let word = unsafe { ptr::read_volatile(self.stack_top(id).sub(16).cast::<u64>()) };
        require(saved.spsr & 0x8f == 5 && saved.gpr[19] == witness && word == witness);
    }

    fn completion_times(&self) -> &'static [(usize, u64)] {
        match self.policy {
            Sjf => &[(A, 5), (B, 7)],
            Srtf => &[(B, 3), (A, 7)],
            Hrrn => &[(A, 10), (B, 15), (C, 16)],
        }
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    unsafe {
        EPOCH = Some(Epoch::new(Sjf));
        sched_start_frame(with_epoch(Epoch::start))
    }
}

extern "C" fn task_marker(id: usize, after_irq: usize, sp: usize) {
    let daif: u64;
    unsafe {
        asm!("mrs {}, daif", "msr daifset, #2", out(reg) daif, options(nostack, preserves_flags))
    };
    with_epoch(|epoch| {
        require(epoch.current == Some(id) && sp == epoch.stack_top(id).addr() - 16);
        if after_irq == 0 {
            require(!epoch.entered[id]);
            epoch.entered[id] = true;
        } else {
            require(id == A && after_irq == 1 && epoch.entered[A] && !epoch.a_returned);
            require(
                epoch.interrupted_pc != 0 && epoch.completed == usize::from(epoch.policy == Srtf),
            );
            epoch.a_returned = true;
        }
        let line: &[u8] = match (epoch.policy, id, after_irq) {
            (Sjf, A, 0) => b"BATCH SJF A FIRST\n",
            (Sjf, A, 1) => b"BATCH SJF A RETAINED\n",
            (Sjf, B, 0) => b"BATCH SJF B FIRST\n",
            (Srtf, A, 0) => b"BATCH SRTF A FIRST\n",
            (Srtf, B, 0) => b"BATCH SRTF B FIRST\n",
            (Srtf, A, 1) => b"BATCH SRTF A RESUMED\n",
            (Hrrn, A, 0) => b"BATCH HRRN A FIRST\n",
            (Hrrn, A, 1) => b"BATCH HRRN A RETAINED\n",
            (Hrrn, B, 0) => b"BATCH HRRN B FIRST\n",
            (Hrrn, C, 0) => b"BATCH HRRN C FIRST\n",
            _ => fail(),
        };
        uart_write(line);
    });
    unsafe { asm!("msr daif, {}", in(reg) daif, options(nostack, preserves_flags)) };
}

extern "C" fn release_interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    require(RELEASED.load(Ordering::Acquire) == 0);
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

    with_epoch(|epoch| {
        epoch.check_exception(A, frame);
        require(epoch.completed == 0 && epoch.interrupted_pc == 0);
        let saved = unsafe { &*frame };
        let work = (&raw const batch_a_work_begin).addr()..(&raw const batch_a_work_end).addr();
        require(work.contains(&(saved.elr as usize)) && saved.gpr[14] == 1 && saved.gpr[20] != 0);
        let witness = epoch.witness(A);
        require(saved.simd[31] == [witness; 2] && saved.tpidr_el1 == witness);
        unsafe { ptr::write_volatile(epoch.stack_top(A).sub(8).cast::<u64>(), saved.gpr[20]) };
        epoch.interrupted_pc = saved.elr;
        epoch.frames[A] = frame;

        let (tick, arriving, estimate, expected_remaining, line): (_, _, _, _, &[u8]) =
            match epoch.policy {
                Sjf => (1, B, 2, 4, b"BATCH SJF IRQ B READY\n"),
                Srtf => (1, B, 2, 4, b"BATCH SRTF IRQ B READY\n"),
                Hrrn => (10, C, 1, 1, b"BATCH HRRN IRQ C READY\n"),
            };
        epoch.release(tick, arriving, estimate);
        require(epoch.scheduler.remaining(A) == Some(expected_remaining));
        let next = epoch.select(tick).unwrap_or_else(|| fail());
        match epoch.policy {
            Srtf => require(epoch.current == Some(B) && epoch.frames[A] == frame && next != frame),
            Sjf | Hrrn => require(epoch.current == Some(A) && next == frame),
        }
        uart_write(line);
        RELEASED.store(1, Ordering::Release);
        next
    })
}

extern "C" fn complete_task(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15 && esr & 0x01ff_ffff == 1);
    require(RELEASED.load(Ordering::Acquire) == 1);
    with_epoch(|epoch| {
        let id = epoch.current.unwrap_or_else(|| fail());
        epoch.check_exception(id, frame);
        let site = match id {
            A => &raw const batch_a_after_exit,
            B => &raw const batch_b_after_exit,
            C => &raw const batch_c_after_exit,
            _ => fail(),
        };
        require(unsafe { (*frame).elr as usize } == site.addr());
        require(id != A || epoch.a_returned);
        let timeline = epoch.completion_times();
        let &(expected, tick) = timeline.get(epoch.completed).unwrap_or_else(|| fail());
        require(id == expected && epoch.scheduler.complete(tick, id).is_ok());
        epoch.completed += 1;
        let next = epoch.select(tick);
        require(epoch.current == timeline.get(epoch.completed).map(|&(id, _)| id));
        if let Some(frame) = next {
            return frame;
        }
        require(epoch.a_returned && epoch.completed == timeline.len());
        // Every epoch owns independent stacks. Build the next policy and its
        // initial contexts with the final SVC still masked on the old stack.
        let next_policy = match epoch.policy {
            Sjf => Srtf,
            Srtf => Hrrn,
            Hrrn => {
                uart_write(b"BATCH PASS\n");
                park();
            }
        };
        *epoch = Epoch::new(next_policy);
        epoch.start()
    })
}
