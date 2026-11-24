//! Single-core, trusted EL1 fixed-priority task runtime.
#[cfg(jnox_fixed_trace)]
#[path = "fixed_runtime_trace.rs"]
mod trace;

use super::context::*;
pub use super::fixed_policy::Mode;
use super::fixed_policy::{Action, Config, Fixed, MAX_TASKS};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

pub struct Task {
    pub entry: fn(),
    pub priority: u8,
    pub delay: u64,
}

const IDLE: usize = MAX_TASKS;
static mut STACKS: [Stack; MAX_TASKS + 1] = [const { Stack([0; STACK_BYTES]) }; MAX_TASKS + 1];
static mut STATE: Option<Runtime> = None;

struct Runtime {
    policy: Fixed,
    frames: [*mut Frame; MAX_TASKS + 1],
}

global_asm!(
    include_str!("context.S"),
    include_str!("fixed_runtime.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    irq = sym interrupt, svc = sym syscall, fail = sym fail,
);

pub fn yield_now() {
    unsafe { asm!("svc #0", options(preserves_flags)) }
}

pub fn exit() -> ! {
    unsafe { asm!("svc #1", "brk #0", options(noreturn)) }
}

pub fn sleep_ticks(ticks: u64) -> bool {
    let accepted: u64;
    unsafe { asm!("svc #2", inout("x0") ticks => accepted, options(preserves_flags)) }
    accepted != 0
}

unsafe extern "C" {
    fn fixed_runtime_enter();
    fn fixed_runtime_idle();
}

pub fn frequency() -> u64 {
    let hz;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) hz, options(nostack, preserves_flags)) }
    hz
}

pub fn counter() -> u64 {
    let now;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) now, options(nostack, preserves_flags)) }
    now
}

fn disable_timer() {
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            options(nostack, preserves_flags)
        )
    }
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) }
    disable_timer();
    super::uart_write(b"FIXED RUNTIME FAIL\n");
    super::park()
}

fn stack_base(slot: usize) -> *mut u8 {
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add(slot * STACK_BYTES)
}

fn initial_frame(slot: usize, pc: usize, argument: usize) -> *mut Frame {
    let fp = stack_base(slot)
        .wrapping_add(STACK_BYTES - size_of::<Frame>())
        .cast::<Frame>();
    // Boot owns these distinct BSS-zeroed stacks until the first ERET.
    unsafe {
        (*fp).elr = pc as u64;
        (*fp).spsr = 0x345; // EL1h, IRQ enabled, D/A/F masked.
        (*fp).gpr[0] = argument as u64;
    }
    fp
}

pub fn run(tasks: &'static [Task], mode: Mode) -> ! {
    // Only CPU0 enters this profile. Reject a second call before touching a
    // stack that could already contain the caller or another live task.
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) }
    if tasks.len() > MAX_TASKS || unsafe { (&*(&raw const STATE)).is_some() } {
        fail()
    }
    init_timer_irq();
    if frequency() == 0 || gic_read(GICD + 0x100) & (1 << TIMER_IRQ) == 0 {
        fail()
    }
    let mut configs = [const {
        Config {
            priority: 0,
            delay: 0,
        }
    }; MAX_TASKS];
    let mut frames = [ptr::null_mut(); MAX_TASKS + 1];
    for (slot, task) in tasks.iter().enumerate() {
        configs[slot] = Config {
            priority: task.priority,
            delay: task.delay,
        };
        frames[slot] = initial_frame(
            slot,
            fixed_runtime_enter as *const () as usize,
            task.entry as usize,
        );
    }
    frames[IDLE] = initial_frame(IDLE, fixed_runtime_idle as *const () as usize, 0);
    #[cfg(jnox_fixed_trace)]
    trace::init(frequency());
    let now = counter();
    let policy = Fixed::new(&configs[..tasks.len()], mode, now).unwrap_or_else(|| fail());
    let state = unsafe { (&mut *(&raw mut STATE)).insert(Runtime { policy, frames }) };
    let next = state.dispatch(
        #[cfg(jnox_fixed_trace)]
        trace::Event {
            kind: 0,
            source: u64::MAX,
            now,
            owner: u64::MAX,
            incoming: ptr::null_mut(),
            argument: 0,
            result: 0,
            pre_ctl: u64::MAX,
        },
    );
    unsafe { sched_start_frame(next) }
}

impl Runtime {
    fn dispatch(&self, #[cfg(jnox_fixed_trace)] event: trace::Event) -> *mut Frame {
        let deadline = self.policy.deadline().unwrap_or_else(|_| fail());
        if let Some(due) = deadline {
            unsafe {
                asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                    in(reg) due, in(reg) 1u64, options(nostack, preserves_flags))
            }
        } else {
            disable_timer();
        }
        let next = self.frames[self.policy.current().unwrap_or(IDLE)];
        #[cfg(jnox_fixed_trace)]
        trace::publish(event, self, next, deadline);
        next
    }

    fn save(&mut self, frame: *mut Frame) {
        let slot = self.policy.current().unwrap_or(IDLE);
        let base = stack_base(slot).addr();
        if !frame.is_aligned()
            || !(base..=base + STACK_BYTES - size_of::<Frame>()).contains(&frame.addr())
        {
            fail()
        }
        self.frames[slot] = frame;
    }
}

// Entry through the shared vector table masks IRQ. No state borrow survives ERET.
unsafe fn state() -> &'static mut Runtime {
    unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap_or_else(|| fail()) }
}

extern "C" fn interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let id = iar & 0x3ff;
    if id >= 1020 {
        return frame;
    }
    #[cfg(jnox_fixed_trace)]
    let pre_ctl = trace::ctl();
    disable_timer();
    // Complete source deassertion before the combined EOI/deactivation.
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
    gic_write(GICC + 0x10, iar);
    if id != TIMER_IRQ {
        fail()
    }
    let state = unsafe { state() };
    state.save(frame);
    #[cfg(jnox_fixed_trace)]
    let owner = state.policy.current().unwrap_or(IDLE) as u64;
    let now = counter();
    state
        .policy
        .event(now, Action::Poll)
        .unwrap_or_else(|| fail());
    state.dispatch(
        #[cfg(jnox_fixed_trace)]
        trace::Event {
            kind: 1,
            source: u64::from(id),
            now,
            owner,
            incoming: frame,
            argument: 0,
            result: 0,
            pre_ctl,
        },
    )
}

extern "C" fn syscall(frame: *mut Frame) -> *mut Frame {
    let syndrome: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) syndrome, options(nostack, preserves_flags)) }
    if syndrome >> 26 != 0x15 {
        fail()
    }
    let state = unsafe { state() };
    state.save(frame);
    #[cfg(jnox_fixed_trace)]
    let owner = state.policy.current().unwrap_or(IDLE) as u64;
    let imm = syndrome & 0xffff;
    let action = match imm {
        0 => Action::Yield,
        1 => Action::Exit,
        2 => Action::Sleep(unsafe { (*frame).gpr[0] }),
        _ => fail(),
    };
    #[cfg(jnox_fixed_trace)]
    let argument = if imm == 2 {
        unsafe { (*frame).gpr[0] }
    } else {
        0
    };
    let now = counter();
    let accepted = state.policy.event(now, action).unwrap_or_else(|| fail());
    if imm == 2 {
        unsafe { (*frame).gpr[0] = u64::from(accepted) }
    }
    state.dispatch(
        #[cfg(jnox_fixed_trace)]
        trace::Event {
            kind: 2,
            source: imm,
            now,
            owner,
            incoming: frame,
            argument,
            result: u64::from(imm == 2 && accepted),
            pre_ctl: u64::MAX,
        },
    )
}
