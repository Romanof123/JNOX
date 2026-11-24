//! Bounded nested groups on CPU0. One saved frame per task plus real idle.
use super::context::*;
use super::hierarchy_policy::Hierarchy;
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};

pub const LEAVES: usize = 3;
const IDLE: usize = LEAVES;
pub struct Task {
    pub entry: fn(),
    pub child: usize,
    pub quota: u64,
    pub ready: bool,
}
#[derive(Clone, Copy, PartialEq)]
enum Status {
    Ready,
    Blocked,
    Exited,
}
struct Runtime {
    policy: Hierarchy<2, 3, LEAVES>,
    tasks: [Status; LEAVES],
    frames: [*mut Frame; LEAVES + 1],
    owner: usize,
    due: u64,
}
static mut STACKS: [Stack; LEAVES + 1] = [const { Stack([0; STACK_BYTES]) }; LEAVES + 1];
static mut STATE: Option<Runtime> = None;
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

pub(super) extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) }
    disable_timer();
    super::uart_write(b"GROUP RUNTIME FAIL\n");
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

pub fn block() {
    unsafe { asm!("svc #2", options(preserves_flags)) }
}

pub fn wake(id: usize) -> bool {
    let result: usize;
    unsafe { asm!("svc #3", inout("x0") id => result, options(preserves_flags)) }
    result != 0
}

pub fn run(
    period: u64,
    parents: [u64; 2],
    children: [(usize, u64); 3],
    tasks: [Task; LEAVES],
) -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) }
    if unsafe { (&*(&raw const STATE)).is_some() } {
        fail()
    }
    init_timer_irq();
    let mut frames = [core::ptr::null_mut(); LEAVES + 1];
    for (id, t) in tasks.iter().enumerate() {
        frames[id] = initial_frame(
            id,
            fixed_runtime_enter as *const () as usize,
            t.entry as usize,
        );
    }
    frames[IDLE] = initial_frame(IDLE, fixed_runtime_idle as *const () as usize, 0);
    #[cfg(jnox_group_trace)]
    unsafe {
        (&raw mut group_journal)
            .cast::<u64>()
            .add(1)
            .write_volatile(period);
    }
    let now = counter();
    let policy = Hierarchy::new(
        now,
        period,
        parents,
        children,
        tasks.each_ref().map(|t| (t.child, t.quota)),
    )
    .unwrap_or_else(|| fail());
    let tasks = tasks.map(|t| {
        if t.ready {
            Status::Ready
        } else {
            Status::Blocked
        }
    });
    let state = unsafe {
        (&mut *(&raw mut STATE)).insert(Runtime {
            policy,
            tasks,
            frames,
            owner: IDLE,
            due: 0,
        })
    };
    let first = state.dispatch(now, 0, IDLE, (0, 0, [0; 3]), 0);
    unsafe { sched_start_frame(first) }
}

// Raw event rows: time, source (0 boot, 1 timer, 2 block, 3 wake, 4 exit),
// previous owner, charged ticks, wake argument, next owner, absolute CVAL,
// ready mask and restored frame. Finite linear prefix; count never wraps.
#[unsafe(no_mangle)]
#[cfg(jnox_group_trace)]
static mut group_journal: [[u64; 15]; 65] = [[0; 15]; 65];

impl Runtime {
    fn dispatch(
        &mut self,
        now: u64,
        source: u64,
        prev: usize,
        settled: (u64, u64, [u64; 3]),
        arg: u64,
    ) -> *mut Frame {
        let ready = self.tasks.map(|s| s == Status::Ready);
        let grant = self.policy.grant_at(now, u64::MAX, &ready);
        let (next, deadline) = match grant {
            Some((_, _, leaf, ticks)) => (leaf, Some(now + ticks)),
            None if ready.contains(&true) => (
                IDLE,
                Some(
                    self.policy
                        .next_actionable(now, &ready)
                        .unwrap_or_else(|| fail()),
                ),
            ),
            None => (IDLE, None),
        };
        let old_due = self.due;
        self.due = deadline.unwrap_or(0);
        self.owner = next;
        if let Some(due) = deadline {
            unsafe {
                asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) due, in(reg) 1u64, options(nostack, preserves_flags))
            }
        } else {
            disable_timer();
        }
        #[cfg(not(jnox_group_trace))]
        let _ = (source, prev, settled, arg, old_due);
        #[cfg(jnox_group_trace)]
        let mask = ready
            .iter()
            .enumerate()
            .fold(0, |m, (i, &v)| m | (u64::from(v) << i));
        #[cfg(jnox_group_trace)]
        unsafe {
            let rows = (&raw mut group_journal).cast::<[u64; 15]>();
            let count = rows.cast::<u64>().read_volatile();
            if count < 64 {
                rows.add(count as usize + 1).write_volatile([
                    now,
                    source,
                    prev as u64,
                    settled.0,
                    arg,
                    next as u64,
                    deadline.unwrap_or(0),
                    mask,
                    self.frames[next] as u64,
                    if source == 0 {
                        0
                    } else {
                        self.frames[prev] as u64
                    },
                    old_due,
                    settled.1,
                    settled.2[0],
                    settled.2[1],
                    settled.2[2],
                ]);
                rows.cast::<u64>().write_volatile(count + 1);
            }
        }
        self.frames[next]
    }
}

fn event(frame: *mut Frame, now: u64, source: u64) -> *mut Frame {
    let state = unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap_or_else(|| fail()) };
    let prev = state.owner;
    let base = stack_base(prev).addr();
    if !frame.is_aligned()
        || !(base..=base + STACK_BYTES - size_of::<Frame>()).contains(&frame.addr())
    {
        fail()
    }
    state.frames[prev] = frame;
    let settled = if prev == IDLE {
        (0, 0, [0; 3])
    } else {
        state.policy.finish_at(now).unwrap_or_else(|| fail())
    };
    let mut arg = 0;
    match source {
        1 => (),
        2 if prev < LEAVES => state.tasks[prev] = Status::Blocked,
        3 if prev < LEAVES => {
            arg = unsafe { (*frame).gpr[0] };
            let accepted = state.tasks.get(arg as usize) == Some(&Status::Blocked);
            if accepted {
                state.tasks[arg as usize] = Status::Ready;
            }
            unsafe { (*frame).gpr[0] = u64::from(accepted) };
        }
        4 if prev < LEAVES => state.tasks[prev] = Status::Exited,
        _ => fail(),
    }
    state.dispatch(now, source, prev, settled, arg)
}

extern "C" fn interrupt(frame: *mut Frame) -> *mut Frame {
    let now = counter();
    let iar = gic_read(GICC + 0x0c);
    if iar & 0x3ff >= 1020 {
        return frame;
    }
    disable_timer();
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
    gic_write(GICC + 0x10, iar);
    if iar & 0x3ff != TIMER_IRQ {
        fail()
    }
    event(frame, now, 1)
}

extern "C" fn syscall(frame: *mut Frame) -> *mut Frame {
    let now = counter();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) }
    if esr >> 26 != 0x15 {
        fail()
    }
    let source = match esr & 0xffff {
        1 => 4,
        2 => 2,
        3 => 3,
        _ => fail(),
    };
    event(frame, now, source)
}
