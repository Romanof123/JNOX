//! Single-core cooperative FCFS bench on the virt EL1 physical timer.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::event_loop::EventLoop;
use super::{park, ready_queue::ReadyQueue, uart_write};

const A: usize = 1;
const B: usize = 0;
const EVENTS: usize = 2;
const WITNESSES: [u64; 2] = [0xb19, 0xa19];

static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static RELEASED: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    fn coop_a_entry();
    fn coop_b_entry();
    fn coop_event_entry();
    static coop_a_work_begin: u8;
    static coop_a_work_end: u8;
    static coop_a_after_yield: u8;
    static coop_a_after_wake: u8;
    static coop_a_after_exit: u8;
    static coop_b_after_block: u8;
    static coop_b_after_exit: u8;
}

global_asm!(
    include_str!("context.S"),
    include_str!("coop.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    a_witness = const WITNESSES[A],
    b_witness = const WITNESSES[B],
    released = sym RELEASED,
    marker = sym task_marker,
    irq = sym release_interrupt,
    svc = sym service_call,
    events = sym run_events,
    fail = sym fail,
);

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

extern "C" fn fail() -> ! {
    // Terminal diagnostic. This single CPU owns the UART.
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"COOP FAIL\n");
    park()
}

fn stack_top(id: usize) -> *mut u8 {
    require(id <= EVENTS);
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

fn initial_frame(id: usize, entry: unsafe extern "C" fn(), pstate: u64) -> *mut Frame {
    // SAFETY: Once per stack, before its first use. All backing words were zeroed at boot.
    unsafe {
        let frame = stack_top(id).sub(size_of::<Frame>()).cast::<Frame>();
        (*frame).elr = entry as usize as u64;
        (*frame).spsr = pstate;
        frame
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Blocked,
    Ready,
    Running,
    Finished,
}

struct Scheduler {
    ready: ReadyQueue<2>,
    states: [State; 2],
    frames: [*mut Frame; 2],
    current: Option<usize>,
    markers: usize,
}

static mut SCHEDULER: Scheduler = Scheduler {
    ready: ReadyQueue::new(),
    states: [State::Blocked; 2],
    frames: [ptr::null_mut(); 2],
    current: None,
    markers: 0,
};

// SAFETY: The sole CPU borrows the scheduler only while IRQs are masked.
// Each borrow ends before returning from an exception or re-enabling IRQs.
fn with_scheduler<R>(operation: impl FnOnce(&mut Scheduler) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    unsafe { operation(&mut *(&raw mut SCHEDULER)) }
}

impl Scheduler {
    fn enqueue(&mut self, id: usize) {
        require(matches!(self.states[id], State::Blocked | State::Running));
        require(!self.frames[id].is_null() && self.ready.push_back(id));
        self.states[id] = State::Ready;
    }

    fn dispatch(&mut self) -> Option<*mut Frame> {
        require(self.current.is_none());
        // FCFS gives every ready task equal rank. The stable queue removes the
        // earliest insertion, counting a yield or a wake as a new arrival.
        let id = self.ready.pop_preferred(|_, _| false)?;
        require(self.states[id] == State::Ready && !self.frames[id].is_null());
        self.current = Some(id);
        self.states[id] = State::Running;
        Some(core::mem::replace(&mut self.frames[id], ptr::null_mut()))
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let first = with_scheduler(|s| {
        s.frames[A] = initial_frame(A, coop_a_entry, 0x3c5); // A unmasks in its work loop.
        s.frames[B] = initial_frame(B, coop_b_entry, 0x345); // EL1h with IRQ enabled.
        s.enqueue(A);
        let first = s.dispatch().unwrap_or_else(|| fail());
        require(s.current == Some(A) && s.states[B] == State::Blocked);
        first
    });
    unsafe { sched_start_frame(first) }
}

extern "C" fn task_marker(site: usize, sp: usize) {
    let daif: u64;
    unsafe {
        asm!("mrs {}, daif", "msr daifset, #2", out(reg) daif, options(nostack, preserves_flags))
    };
    // The task supplies a literal at each call site and its live stack pointer.
    let (id, other_state, line): (_, _, &[u8]) = match site {
        0 => (A, State::Blocked, b"COOP A FIRST\n"),
        1 => (A, State::Ready, b"COOP A RETAINED\n"),
        2 => (B, State::Ready, b"COOP B FIRST\n"),
        3 => (A, State::Blocked, b"COOP A RESUMED\n"),
        4 => (A, State::Ready, b"COOP B WOKEN\n"),
        5 => (B, State::Finished, b"COOP B RESUMED\n"),
        _ => fail(),
    };
    with_scheduler(|s| {
        require(s.markers == site && s.current == Some(id));
        require(s.states[id] == State::Running && s.states[1 - id] == other_state);
        require(sp == stack_top(id).addr() - 16);
        s.markers += 1;
    });
    uart_write(line);
    unsafe { asm!("msr daif, {}", in(reg) daif, options(nostack, preserves_flags)) };
}

// The caller has established task identity. Verify the exact frame and stack
// before reading any saved register or task-owned stack word.
fn check_task_frame(id: usize, frame: *mut Frame) {
    require(id == A || id == B);
    require(frame.addr() == stack_top(id).addr() - 16 - size_of::<Frame>());
    let saved = unsafe { &*frame };
    require(saved.spsr & 0x8f == 5 && saved.gpr[19] == WITNESSES[id]);
    let word = unsafe { ptr::read_volatile(stack_top(id).sub(16).cast::<u64>()) };
    require(word == WITNESSES[id]);
}

extern "C" fn release_interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5);
    require(RELEASED.load(Ordering::Acquire) == 0);
    // Deassert the level-triggered one-shot before acknowledging the complete IAR token.
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

    with_scheduler(|s| {
        require(s.current == Some(A) && s.states[A] == State::Running && s.frames[A].is_null());
        require(s.markers == 1 && s.states[B] == State::Blocked);
        check_task_frame(A, frame);
        let saved = unsafe { &*frame };
        let work = (&raw const coop_a_work_begin).addr()..(&raw const coop_a_work_end).addr();
        require(work.contains(&(saved.elr as usize)) && saved.gpr[14] == 1 && saved.gpr[20] != 0);
        // A will compare the restored live count with this private stack word.
        unsafe { ptr::write_volatile(stack_top(A).sub(8).cast::<u64>(), saved.gpr[20]) };
        s.enqueue(B);
    });
    uart_write(b"COOP IRQ B READY\n");
    RELEASED.store(1, Ordering::Release);
    // A cooperative IRQ may release work. ERET consumes the original frame
    // directly, including the untouched ELR at which A was interrupted.
    frame
}

extern "C" fn service_call(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15 && RELEASED.load(Ordering::Acquire) == 1);
    let immediate = esr & 0x01ff_ffff;
    with_scheduler(|s| {
        let id = s.current.unwrap_or_else(|| fail());
        require(s.states[id] == State::Running && s.frames[id].is_null());
        check_task_frame(id, frame);
        let elr = unsafe { (*frame).elr as usize };
        let (next_id, next_pc) = match (id, immediate) {
            (A, 2) => {
                require(
                    elr == (&raw const coop_a_after_yield).addr() && s.states[B] == State::Ready,
                );
                s.frames[A] = frame;
                s.enqueue(A);
                (B, coop_b_entry as *const () as usize)
            }
            (B, 3) => {
                require(
                    elr == (&raw const coop_b_after_block).addr() && s.states[A] == State::Ready,
                );
                s.frames[B] = frame;
                s.states[B] = State::Blocked;
                (A, (&raw const coop_a_after_yield).addr())
            }
            (A, 4) => {
                require(
                    elr == (&raw const coop_a_after_wake).addr() && s.states[B] == State::Blocked,
                );
                s.enqueue(B);
                return frame;
            }
            (A, 1) => {
                require(
                    elr == (&raw const coop_a_after_exit).addr() && s.states[B] == State::Ready,
                );
                s.states[A] = State::Finished;
                (B, (&raw const coop_b_after_block).addr())
            }
            (B, 1) => {
                require(
                    elr == (&raw const coop_b_after_exit).addr() && s.states[A] == State::Finished,
                );
                require(s.markers == 6);
                s.states[B] = State::Finished;
                s.current = None;
                require(s.dispatch().is_none());
                return initial_frame(EVENTS, coop_event_entry, 0x345);
            }
            _ => fail(),
        };
        s.current = None;
        let next = s.dispatch().unwrap_or_else(|| fail());
        require(s.current == Some(next_id) && unsafe { (*next).elr as usize } == next_pc);
        next
    })
}

#[inline(never)]
fn handle_event(event: u8, enqueue: &mut dyn FnMut(u8) -> Result<(), u8>, expected: &mut u8) {
    require(event == *expected);
    match event {
        1 => {
            uart_write(b"EVENT 1\n");
            require(enqueue(3).is_ok()); // Event 2 is already waiting.
        }
        2 => uart_write(b"EVENT 2\n"),
        3 => uart_write(b"EVENT 3\n"),
        _ => fail(),
    }
    *expected += 1;
}

extern "C" fn run_events(sp: usize) -> ! {
    require(sp == stack_top(EVENTS).addr());
    let [first, second, overflow] = core::hint::black_box([1u8, 2, 4]);
    let mut events = EventLoop::<u8, 2>::new();
    require(events.enqueue(first).is_ok() && events.enqueue(second).is_ok());
    require(events.enqueue(overflow) == Err(4));
    let mut expected = 1;
    while events.dispatch_one(|event, enqueue| handle_event(event, enqueue, &mut expected)) {}
    require(expected == 4);
    uart_write(b"COOP PASS\n");
    park()
}
