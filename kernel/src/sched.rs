//! One-core EL1h scheduler bench for QEMU virt, GICv2, secure=off.

use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::dispatch_policy::{DispatchJob, DispatchPolicy};
use super::{park, ready_queue::ReadyQueue, uart_write};

const LOW: usize = 0;
// The first woken peer (HIGH / FIFO A) deliberately has the larger ID in FIFO.
const HIGH: usize = if cfg!(feature = "fifo-bench") { 2 } else { 1 };
const PEER_B: usize = 1;
const TASK_COUNT: usize = HIGH + 1;
const EDF: bool = cfg!(all(feature = "edf-bench", not(feature = "fifo-bench")));
const LOW_PRIORITY: u8 = if EDF { 3 } else { 1 };
const HIGH_PRIORITY: u8 = 2;
static mut LOW_STACK: Stack = Stack([0; STACK_BYTES]);
static mut HIGH_STACK: Stack = Stack([0; STACK_BYTES]);
#[cfg(feature = "fifo-bench")]
static mut B_STACK: Stack = Stack([0; STACK_BYTES]);
// The SVC handler publishes completion to the assembly loop's aligned LDAR.
static HIGH_DONE: AtomicU64 = AtomicU64::new(0);
// Published by the IRQ handler and observed by the peers' LDAR work loops.
static RR_ROTATIONS: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    static sched_work_begin: u8;
    static sched_work_end: u8;
    static sched_high_return: u8;
    #[cfg(all(feature = "fifo-bench", not(feature = "rr-bench")))]
    static sched_high_resume: u8;
    #[cfg(all(feature = "fifo-bench", not(feature = "rr-bench")))]
    static sched_b_resume: u8;
    #[cfg(feature = "fifo-bench")]
    static sched_b_return: u8;
    #[cfg(feature = "rr-bench")]
    static sched_rr_a_begin: u8;
    #[cfg(feature = "rr-bench")]
    static sched_rr_a_end: u8;
    #[cfg(feature = "rr-bench")]
    static sched_rr_b_begin: u8;
    #[cfg(feature = "rr-bench")]
    static sched_rr_b_end: u8;
    fn sched_low_entry();
    fn sched_high_entry();
    #[cfg(feature = "fifo-bench")]
    fn sched_b_entry();
}

global_asm!(
    include_str!("context.S"),
    include_str!("sched.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    fifo = const cfg!(feature = "fifo-bench") as usize,
    rr = const cfg!(feature = "rr-bench") as usize,
    rotations = sym RR_ROTATIONS,
    high_id = const HIGH,
    b_id = const PEER_B,
    done = sym HIGH_DONE,
    low_start = sym low_start,
    high_start = sym high_start,
    irq = sym timer_interrupt,
    svc = sym service_call,
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
    #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
    release: u64,
    #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
    deadline: u64,
    state: State,
    frame: *mut Frame,
}

impl Task {
    const fn new(priority: u8, state: State) -> Self {
        Self {
            priority,
            #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
            release: 0,
            #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
            deadline: 0,
            state,
            frame: ptr::null_mut(),
        }
    }
}

struct Scheduler {
    tasks: [Task; TASK_COUNT],
    ready: ReadyQueue<TASK_COUNT>,
    current: Option<usize>,
    #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
    interrupted_low_pc: u64,
}

impl Scheduler {
    fn peers_finished(&self) -> bool {
        self.tasks[1..]
            .iter()
            .all(|task| task.state == State::Finished)
    }

    fn wake(&mut self, id: usize) {
        let task = &mut self.tasks[id];
        require(task.state == State::Blocked && !task.frame.is_null());
        #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
        {
            let frequency: u64;
            unsafe {
                asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags))
            };
            require(frequency > 0);
            task.release = counter_ticks();
            let seconds = if id == LOW { 60 } else { 1 };
            let relative = frequency.checked_mul(seconds).unwrap_or_else(|| fail());
            task.deadline = task.release.checked_add(relative).unwrap_or_else(|| fail());
        }
        require(self.ready.push_back(id));
        task.state = State::Ready;
    }

    fn dispatch(&mut self) -> *mut Frame {
        let tasks = &self.tasks;
        #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
        let selected_id = {
            let now = counter_ticks();
            DispatchPolicy::EarliestDeadlineFirst.pop_ready(&mut self.ready, now, |id| {
                let task = &tasks[id];
                require(task.state == State::Ready && task.release <= now);
                let relative = task
                    .deadline
                    .checked_sub(task.release)
                    .unwrap_or_else(|| fail());
                require(relative > 0);
                Some(DispatchJob {
                    release: task.release,
                    relative_deadline: relative,
                    ..DispatchJob::fixed_priority(task.priority)
                })
            })
        };
        #[cfg(any(not(feature = "edf-bench"), feature = "fifo-bench"))]
        let selected_id = {
            DispatchPolicy::FixedPriority.pop_ready(&mut self.ready, 0, |id| {
                Some(DispatchJob::fixed_priority(tasks[id].priority))
            })
        };
        let Some(id) = selected_id else { fail() };
        let selected = &mut self.tasks[id];
        require(selected.state == State::Ready && !selected.frame.is_null());
        selected.state = State::Running;
        self.current = Some(id);
        #[cfg(feature = "rr-bench")]
        if id != LOW && RR_ROTATIONS.load(Ordering::Acquire) < 3 {
            arm_rr_quantum();
        }
        core::mem::replace(&mut selected.frame, ptr::null_mut())
    }

    #[cfg(feature = "fifo-bench")]
    fn rotate(&mut self, id: usize, frame: *mut Frame) -> *mut Frame {
        self.tasks[id].frame = frame;
        self.tasks[id].state = State::Ready;
        require(self.ready.push_back(id));
        self.dispatch()
    }
}

static mut SCHEDULER: Scheduler = Scheduler {
    tasks: [
        Task::new(LOW_PRIORITY, State::Blocked),
        Task::new(HIGH_PRIORITY, State::Blocked),
        #[cfg(feature = "fifo-bench")]
        Task::new(HIGH_PRIORITY, State::Blocked),
    ],
    ready: ReadyQueue::new(),
    current: None,
    #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
    interrupted_low_pc: 0,
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

#[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
fn counter_ticks() -> u64 {
    let ticks: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) ticks, options(nostack, preserves_flags)) };
    ticks
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
    let base = match id {
        LOW => &raw mut LOW_STACK,
        HIGH => &raw mut HIGH_STACK,
        #[cfg(feature = "fifo-bench")]
        PEER_B => &raw mut B_STACK,
        _ => fail(),
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

pub(super) fn run() -> ! {
    init_timer_irq();
    let frame = with_scheduler(|sched| {
        sched.tasks[LOW].frame = initial_frame(LOW, sched_low_entry, 0x3c5); // EL1h, DAIF masked
        // RR peers enable IRQs at their work loop once their witnesses are live.
        let peer_pstate = if cfg!(feature = "rr-bench") {
            0x3c5
        } else {
            0x345
        };
        sched.tasks[HIGH].frame = initial_frame(HIGH, sched_high_entry, peer_pstate);
        #[cfg(feature = "fifo-bench")]
        {
            sched.tasks[PEER_B].frame = initial_frame(PEER_B, sched_b_entry, peer_pstate);
        }
        sched.wake(LOW);
        let frame = sched.dispatch();
        require(sched.current == Some(LOW));
        require(
            sched.tasks[1..]
                .iter()
                .all(|task| task.state == State::Blocked),
        );
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

#[cfg(feature = "rr-bench")]
fn arm_rr_quantum() {
    let frequency: u64;
    // Charge 1/100 s of the guest physical counter from dispatch, including
    // restore and marker time. IMASK holds delivery until the peer's work
    // window. Clearing IMASK later leaves this deadline intact.
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let ticks = frequency / 100;
    require(ticks > 0 && ticks <= i32::MAX as u64);
    unsafe {
        asm!("msr cntp_tval_el0, {ticks}", "msr cntp_ctl_el0, {masked}", "isb",
            ticks = in(reg) ticks, masked = in(reg) 3u64, options(nostack, preserves_flags))
    };
}

#[cfg(feature = "rr-bench")]
fn expire_rr_slice(sched: &mut Scheduler, id: usize, frame: *mut Frame) -> *mut Frame {
    let turns = RR_ROTATIONS.load(Ordering::Acquire);
    let (expected, peer, threshold) = match turns {
        0 => (HIGH, PEER_B, 2),
        1 => (PEER_B, HIGH, 3),
        2 => (HIGH, PEER_B, 3),
        _ => fail(),
    };
    require(id == expected && sched.tasks[peer].state == State::Ready);
    require(sched.tasks[id].priority == sched.tasks[peer].priority);
    require(sched.tasks[LOW].state == State::Ready);
    let (begin, end, witness) = if id == HIGH {
        (
            &raw const sched_rr_a_begin,
            &raw const sched_rr_a_end,
            0xa19,
        )
    } else {
        (
            &raw const sched_rr_b_begin,
            &raw const sched_rr_b_end,
            0xb19,
        )
    };
    // SAFETY: IRQ entry verified the frame address on this running peer's stack.
    let saved = unsafe { &*frame };
    require((begin.addr()..end.addr()).contains(&(saved.elr as usize)));
    require(saved.gpr[19] == witness && saved.tpidr_el1 == witness);
    require(saved.simd[31] == [witness; 2]);
    require(saved.gpr[11] == threshold && saved.gpr[14] == 1);
    let count = saved.gpr[9];
    require(count >> 32 == witness && count as u32 != 0);
    // The task reserves two stack words: its private witness and a snapshot
    // of the interrupted counter. It checks the restored counter on resumption.
    unsafe {
        let words = stack_top(id).sub(16).cast::<u64>();
        require(ptr::read_volatile(words) == witness && ptr::read_volatile(words.add(1)) == 0);
        ptr::write_volatile(words.add(1), count);
    }
    // No task runs until exception return. Dispatch arms the incoming slice
    // using the new rotation count and leaves CNTP disabled after the third.
    RR_ROTATIONS.store(turns + 1, Ordering::Release);
    let next = sched.rotate(id, frame);
    require(sched.current == Some(peer));
    next
}

extern "C" fn timer_interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    // SAFETY: IRQ entry saved all task registers. CNTP is owned by this bench.
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5); // asserted, enabled, unmasked
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
    with_scheduler(|sched| {
        let Some(id) = sched.current else { fail() };
        require(sched.tasks[id].state == State::Running);
        require(frame.addr() == stack_top(id).addr() - 16 - size_of::<Frame>());
        // SAFETY: The vector supplied the fixed-size frame on the verified stack.
        let saved = unsafe { &*frame };
        require(saved.spsr & 0x8f == 5 && saved.gpr[9] != 0);
        #[cfg(feature = "rr-bench")]
        if id != LOW {
            return expire_rr_slice(sched, id, frame);
        }
        require(id == LOW);
        let begin = (&raw const sched_work_begin).addr();
        let end = (&raw const sched_work_end).addr();
        require((begin..end).contains(&(saved.elr as usize)));
        #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
        {
            sched.interrupted_low_pc = saved.elr;
        }
        sched.tasks[LOW].frame = frame;
        sched.tasks[LOW].state = State::Ready;
        require(sched.ready.push_front(LOW)); // Interrupted LOW keeps its turn.
        sched.wake(HIGH);
        #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
        {
            let low = &sched.tasks[LOW];
            let urgent = &sched.tasks[HIGH];
            require(low.priority > urgent.priority && low.release < urgent.release);
            require(urgent.deadline < low.deadline);
        }
        #[cfg(feature = "fifo-bench")]
        sched.wake(PEER_B);
        let next = sched.dispatch();
        require(sched.current == Some(HIGH) && sched.tasks[LOW].state == State::Ready);
        uart_write(b"SCHED TIMER IRQ\n");
        next
    })
}

// Each task body reports from its own entry or resumption call site.
extern "C" fn high_start(sp: usize, id: usize, resumed: usize) {
    // SAFETY: Serialize the marker and scheduler access on this single CPU.
    unsafe { asm!("msr daifset, #2", options(nostack, preserves_flags)) };
    let line: &[u8] = match (id, resumed) {
        #[cfg(not(feature = "fifo-bench"))]
        (HIGH, 0) if EDF => b"EDF URGENT RUN\n",
        #[cfg(not(feature = "fifo-bench"))]
        (HIGH, 0) => b"SCHED HIGH RUN\n",
        #[cfg(all(feature = "fifo-bench", not(feature = "rr-bench")))]
        (HIGH, 0) => b"FIFO A FIRST\n",
        #[cfg(all(feature = "fifo-bench", not(feature = "rr-bench")))]
        (PEER_B, 0) => b"FIFO B FIRST\n",
        #[cfg(all(feature = "fifo-bench", not(feature = "rr-bench")))]
        (HIGH, 1) => b"FIFO A RESUMED\n",
        #[cfg(all(feature = "fifo-bench", not(feature = "rr-bench")))]
        (PEER_B, 1) => b"FIFO B RESUMED\n",
        #[cfg(feature = "rr-bench")]
        (HIGH, 0) => b"RR A FIRST\n",
        #[cfg(feature = "rr-bench")]
        (PEER_B, 0) => b"RR B FIRST\n",
        #[cfg(feature = "rr-bench")]
        (HIGH, 1) => b"RR A RESUMED\n",
        #[cfg(feature = "rr-bench")]
        (PEER_B, 1) => b"RR B RESUMED\n",
        _ => fail(),
    };
    require(sp == stack_top(id).addr() - resumed * 16);
    with_scheduler(|sched| {
        require(sched.current == Some(id) && sched.tasks[id].state == State::Running);
        require(sched.tasks[LOW].state == State::Ready);
    });
    #[cfg(feature = "rr-bench")]
    {
        let turns = RR_ROTATIONS.load(Ordering::Acquire);
        require(turns == (2 * resumed + usize::from(id == PEER_B)) as u64);
    }
    uart_write(line);
    #[cfg(not(feature = "rr-bench"))]
    unsafe {
        asm!("msr daifclr, #2", options(nostack, preserves_flags))
    };
}

extern "C" fn service_call(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    // SAFETY: Only the current-EL/SPx synchronous vector calls this function.
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    require(esr >> 26 == 0x15); // AArch64 SVC
    let immediate = esr & 0x01ff_ffff;
    with_scheduler(|sched| {
        let Some(id) = sched.current else { fail() };
        require(id != LOW && sched.tasks[id].state == State::Running);
        require(sched.tasks[LOW].state == State::Ready);
        require(frame.addr() == stack_top(id).addr() - 16 - size_of::<Frame>());
        // SAFETY: The vector built this frame on the verified task stack.
        let saved = unsafe { &*frame };
        require(saved.spsr & 0x8f == 5); // IRQ-enabled EL1h
        let elr = saved.elr as usize;

        #[cfg(all(feature = "fifo-bench", not(feature = "rr-bench")))]
        if immediate == 2 {
            let (peer, resume_pc) = if id == HIGH {
                (PEER_B, &raw const sched_high_resume)
            } else {
                (HIGH, &raw const sched_b_resume)
            };
            require(elr == resume_pc.addr() && sched.tasks[peer].state == State::Ready);
            let next = sched.rotate(id, frame);
            require(sched.current == Some(peer));
            return next;
        }

        #[cfg(feature = "fifo-bench")]
        let finish_pc = if id == HIGH {
            &raw const sched_high_return
        } else {
            &raw const sched_b_return
        };
        #[cfg(not(feature = "fifo-bench"))]
        let finish_pc = &raw const sched_high_return;
        require(immediate == 1 && elr == finish_pc.addr());
        #[cfg(feature = "rr-bench")]
        require(RR_ROTATIONS.load(Ordering::Acquire) == 3);
        sched.tasks[id].state = State::Finished;
        let next = sched.dispatch();
        #[cfg(feature = "fifo-bench")]
        {
            let (first, last) = if cfg!(feature = "rr-bench") {
                (PEER_B, HIGH)
            } else {
                (HIGH, PEER_B)
            };
            if id == first {
                require(sched.current == Some(last));
                return next;
            }
        }
        require(sched.current == Some(LOW) && sched.peers_finished());
        #[cfg(all(feature = "edf-bench", not(feature = "fifo-bench")))]
        {
            require(next.addr() == stack_top(LOW).addr() - 16 - size_of::<Frame>());
            // SAFETY: dispatch returned LOW's previously validated IRQ frame.
            require(unsafe { (*next).elr } == sched.interrupted_low_pc);
        }
        HIGH_DONE.store(1, Ordering::Release); // Let LOW exit only after the final peer.
        next
    })
}

extern "C" fn low_resumed(sp: usize) -> ! {
    // SAFETY: Low has finished its assembly witnesses. The benchmark terminates
    // with interrupts masked on its original stack.
    unsafe { asm!("msr daifset, #2", options(nostack, preserves_flags)) };
    require(sp == stack_top(LOW).addr() - 16 && HIGH_DONE.load(Ordering::Acquire) == 1);
    with_scheduler(|sched| require(sched.current == Some(LOW) && sched.peers_finished()));
    uart_write(b"SCHED LOW RESUMED\nSCHED PASS\n");
    park()
}
