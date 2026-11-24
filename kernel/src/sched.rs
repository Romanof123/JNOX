//! One-core EL1h scheduler bench for QEMU virt, GICv2, secure=off.

use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{park, uart_write};

const LOW: usize = 0;
const HIGH: usize = 1;
const STACK_BYTES: usize = 16 * 1024;
const TIMER_IRQ: u32 = 30; // Nonsecure EL1 physical timer PPI
const GICD: usize = 0x0800_0000;
const GICC: usize = 0x0801_0000;

#[repr(C, align(16))]
struct Stack([u8; STACK_BYTES]);

// Each task owns one stack. Its saved SP points to a complete Frame beneath
// its interrupted SP. The CPU-wide translation and vector configuration is
// shared. The benchmark never uses exclusive monitors as task-local state.
#[repr(C, align(16))]
struct Frame {
    gpr: [u64; 31],
    elr: u64,
    spsr: u64,
    fpcr: u64,
    fpsr: u64,
    tpidr_el1: u64,
    sp_el0: u64,
    tpidr_el0: u64,
    tpidrro_el0: u64,
    reserved: u64,
    simd: [[u64; 2]; 32],
}

static mut LOW_STACK: Stack = Stack([0; STACK_BYTES]);
static mut HIGH_STACK: Stack = Stack([0; STACK_BYTES]);
// The SVC handler publishes completion to the assembly loop's aligned LDAR.
static HIGH_DONE: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    static sched_vectors: u8;
    static sched_work_begin: u8;
    static sched_work_end: u8;
    static sched_high_return: u8;
    fn sched_low_entry();
    fn sched_high_entry();
    fn sched_start_frame(frame: *mut Frame) -> !;
}

global_asm!(
    include_str!("sched.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    done = sym HIGH_DONE,
    low_start = sym low_start,
    high_start = sym high_start,
    irq = sym timer_interrupt,
    svc = sym finish_high,
    pass = sym low_resumed,
    fail = sym fail,
);

#[derive(PartialEq, Eq)]
enum State {
    Blocked,
    Ready,
    Running,
    Finished,
}

struct Task {
    priority: u8,
    state: State,
    frame: *mut Frame,
}

impl Task {
    const fn new(priority: u8, state: State) -> Self {
        Self {
            priority,
            state,
            frame: ptr::null_mut(),
        }
    }
}

struct Scheduler {
    tasks: [Task; 2],
    current: Option<usize>,
}

impl Scheduler {
    fn dispatch(&mut self) -> *mut Frame {
        let Some((id, selected)) = self
            .tasks
            .iter_mut()
            .enumerate()
            .filter(|(_, task)| task.state == State::Ready)
            .max_by_key(|(_, task)| task.priority)
        else {
            fail()
        };
        require(!selected.frame.is_null());
        selected.state = State::Running;
        self.current = Some(id);
        core::mem::replace(&mut selected.frame, ptr::null_mut())
    }
}

static mut SCHEDULER: Scheduler = Scheduler {
    tasks: [Task::new(1, State::Ready), Task::new(2, State::Blocked)],
    current: None,
};

fn with_scheduler<R>(operation: impl FnOnce(&mut Scheduler) -> R) -> R {
    let daif: u64;
    // SAFETY: This EL1-only bench has one CPU. Boot and exception entry mask
    // IRQs. Task marker callbacks mask them before borrowing the scheduler.
    // No FIQ/SError handler borrows it. Callbacks never recurse or switch.
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & (1 << 7) != 0);
    unsafe { operation(&mut *(&raw mut SCHEDULER)) }
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

extern "C" fn fail() -> ! {
    // SAFETY: Terminal diagnostic on the single CPU. No handler can interleave
    // further UART writes after DAIF is masked.
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"SCHED FAIL\n");
    park()
}

fn stack_top(id: usize) -> *mut u8 {
    let base = if id == LOW {
        &raw mut LOW_STACK
    } else {
        &raw mut HIGH_STACK
    };
    base.cast::<u8>().wrapping_add(STACK_BYTES)
}

fn initial_frame(id: usize, entry: unsafe extern "C" fn(), pstate: u64) -> *mut Frame {
    // SAFETY: Called once per aligned, zero-initialized stack before dispatch.
    // The zeroed backing bytes supply every initial GPR, SIMD and TLS value.
    unsafe {
        let frame = stack_top(id).sub(size_of::<Frame>()).cast::<Frame>();
        (*frame).elr = entry as usize as u64;
        (*frame).spsr = pstate;
        frame
    }
}

fn gic_read(address: usize) -> u32 {
    // SAFETY: Callers use aligned readable GICv2 MMIO registers with MMU off.
    unsafe { ptr::read_volatile(address as *const u32) }
}

fn gic_write(address: usize, value: u32) {
    // SAFETY: Callers use aligned writable GICv2 MMIO registers, single CPU.
    unsafe { ptr::write_volatile(address as *mut u32, value) }
}

pub(super) fn run() -> ! {
    // SAFETY: Boot masked exceptions. VBAR is 2-KiB aligned. Keep CNTP
    // disabled until low's private work and register witnesses are ready.
    unsafe {
        asm!("msr vbar_el1, {}", "msr cntp_ctl_el0, xzr", "isb",
            in(reg) &raw const sched_vectors, options(nostack, preserves_flags));
    }
    // With secure=off the GIC exposes both groups. Use Group 0 as IRQ (FIQEn
    // clear), with priority 0x80 allowed by PMR. PPIs are banked per CPU.
    gic_write(GICD, 0);
    gic_write(GICC, 0);
    gic_write(GICD + 0x80, gic_read(GICD + 0x80) & !(1 << TIMER_IRQ));
    unsafe { ptr::write_volatile((GICD + 0x400 + TIMER_IRQ as usize) as *mut u8, 0x80) };
    gic_write(GICD + 0x280, 1 << TIMER_IRQ); // ICPENDR0
    gic_write(GICD + 0x100, 1 << TIMER_IRQ); // ISENABLER0
    gic_write(GICC + 4, 0xff); // PMR
    gic_write(GICD, 1);
    gic_write(GICC, 1);
    unsafe { asm!("dsb sy", "isb", options(nostack, preserves_flags)) };

    let frame = with_scheduler(|sched| {
        sched.tasks[LOW].frame = initial_frame(LOW, sched_low_entry, 0x3c5); // EL1h, DAIF masked
        sched.tasks[HIGH].frame = initial_frame(HIGH, sched_high_entry, 0x345); // EL1h, IRQ enabled
        let frame = sched.dispatch();
        require(sched.current == Some(LOW) && sched.tasks[HIGH].state == State::Blocked);
        frame
    });
    // SAFETY: All scheduler borrows have ended. Restore the selected stack
    // and enter its task at the ELR and PSTATE from the initial frame.
    unsafe { sched_start_frame(frame) }
}

extern "C" fn low_start(sp: usize) {
    require(sp == stack_top(LOW).addr());
    with_scheduler(|sched| require(sched.current == Some(LOW)));
    uart_write(b"SCHED LOW RUN\n");
}

extern "C" fn timer_interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    // SAFETY: IRQ entry saved all task registers. CNTP is owned by this bench.
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5); // asserted, enabled, unmasked
    require(frame.addr() == stack_top(LOW).addr() - 16 - size_of::<Frame>());
    let begin = (&raw const sched_work_begin).addr();
    let end = (&raw const sched_work_end).addr();
    // SAFETY: The vector supplied the fixed-size frame on the verified stack.
    let saved = unsafe { &*frame };
    require((begin..end).contains(&(saved.elr as usize)));
    require(saved.spsr & 0x8f == 5 && saved.gpr[9] != 0);
    require(HIGH_DONE.load(Ordering::Acquire) == 0);

    // Deassert the level source before EOI. Write back the complete IAR token.
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
    let next = with_scheduler(|sched| {
        require(sched.current == Some(LOW) && sched.tasks[LOW].state == State::Running);
        require(sched.tasks[HIGH].state == State::Blocked);
        sched.tasks[LOW].frame = frame;
        sched.tasks[LOW].state = State::Ready;
        sched.tasks[HIGH].state = State::Ready;
        let next = sched.dispatch();
        require(sched.current == Some(HIGH) && sched.tasks[LOW].state == State::Ready);
        next
    });
    uart_write(b"SCHED TIMER IRQ\n");
    next
}

extern "C" fn high_start(sp: usize) {
    require(sp == stack_top(HIGH).addr());
    // SAFETY: The high task starts with IRQ enabled. Serialize its marker
    // and scheduler access, then return it to that same IRQ state.
    unsafe { asm!("msr daifset, #2", options(nostack, preserves_flags)) };
    with_scheduler(|sched| {
        require(sched.current == Some(HIGH) && sched.tasks[LOW].state == State::Ready)
    });
    uart_write(b"SCHED HIGH RUN\n");
    unsafe { asm!("msr daifclr, #2", options(nostack, preserves_flags)) };
}

extern "C" fn finish_high(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    // SAFETY: Only the current-EL/SPx synchronous vector calls this function.
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15 && esr & 0x01ff_ffff == 1); // AArch64 SVC #1
    require(frame.addr() == stack_top(HIGH).addr() - 16 - size_of::<Frame>());
    // SAFETY: The vector built this frame on the verified high stack.
    require(unsafe { (*frame).elr as usize } == (&raw const sched_high_return).addr());
    with_scheduler(|sched| {
        require(sched.current == Some(HIGH) && sched.tasks[HIGH].state == State::Running);
        sched.tasks[HIGH].state = State::Finished;
        let next = sched.dispatch();
        require(sched.current == Some(LOW));
        HIGH_DONE.store(1, Ordering::Release);
        next
    })
}

extern "C" fn low_resumed(sp: usize) -> ! {
    // SAFETY: Low has finished its assembly witnesses. The benchmark terminates
    // with interrupts masked on its original stack.
    unsafe { asm!("msr daifset, #2", options(nostack, preserves_flags)) };
    require(sp == stack_top(LOW).addr() - 16 && HIGH_DONE.load(Ordering::Acquire) == 1);
    with_scheduler(|sched| {
        require(sched.current == Some(LOW) && sched.tasks[HIGH].state == State::Finished)
    });
    uart_write(b"SCHED LOW RESUMED\nSCHED PASS\n");
    park()
}
