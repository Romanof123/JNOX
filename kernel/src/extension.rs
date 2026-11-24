//! One-core POL-018 bench. The evaluator and each service context use private
//! stacks. Fixed absolute counter compares delimit evaluation and service.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::extension_policy::{Decision, Extension, Op, Reason, Reservation};
use super::group_policy::{DelegatedTask, Proposal};
use super::{park, uart_write};

const SLOTS: usize = 9;
const IDLE: usize = 3;
const EVALUATOR: usize = 4;
const NONE: u64 = 255;
const WATCHDOG: u64 = 2;
const FUEL: u8 = 5;
const RESERVATION: Reservation = Reservation {
    task: 0,
    period: 3,
    offset: 2,
    ticks: 1,
};
const VALID: Proposal = Proposal {
    task: 2,
    generation: 17,
    cpu: 0,
    ticks: 1,
};
const LIVE: [Op; 2] = [
    Op::IfReady {
        task: 2,
        yes: 1,
        no: 0,
    },
    Op::Propose(VALID),
];
const OUTSIDE: [Op; 1] = [Op::Propose(Proposal { cpu: 1, ..VALID })];
const STALE: [Op; 1] = [Op::Propose(Proposal {
    generation: 16,
    ..VALID
})];
const EXCESS: [Op; 1] = [Op::Jump(0)];

fn program(slot: usize) -> &'static [Op] {
    match slot {
        0 => &LIVE,
        1 => &OUTSIDE,
        3 => &STALE,
        4 => &EXCESS,
        _ => &[],
    }
}

fn tasks(slot: usize) -> [DelegatedTask; 3] {
    [7, 11, 17].map(|generation| DelegatedTask {
        generation,
        ready: slot != 7 || generation == 7,
        allowed_cpus: 1,
        grant_limit: 1,
    })
}

unsafe extern "C" {
    fn ext_reserved();
    fn ext_fallback();
    fn ext_nominee();
    fn ext_idle();
    fn ext_eval_live();
    fn ext_eval_stopped();
    static ext_reserved_work: u8;
    static ext_reserved_end: u8;
    static ext_fallback_work: u8;
    static ext_fallback_end: u8;
    static ext_nominee_work: u8;
    static ext_nominee_end: u8;
    static ext_idle_work: u8;
    static ext_idle_end: u8;
    static ext_eval_live_work: u8;
    static ext_eval_live_end: u8;
    static ext_eval_stopped_work: u8;
    static ext_eval_stopped_end: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("extension.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym fail, fail = sym fail, evaluate = sym evaluate,
    epoch = sym EPOCH, observed = sym FIRST_WORK,
);

// This ABI contains only little-endian u64 values for QMP physical-memory
// capture. A completed row is made visible by incrementing count last.
#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Sample {
    slot: u64,
    ready_mask: u64,
    proposed_id: u64,
    proposed_generation: u64,
    proposed_cpu: u64,
    reason: u64,
    steps: u64,
    selected: u64,
    grant: u64,
    progress: u64,
    eval_cval: u64,
    eval_irq: u64,
    eval_pc: u64,
    eval_frame: u64,
    eval_work: u64,
    decision_time: u64,
    dispatch_pc: u64,
    first_work: u64,
    service_cval: u64,
    service_irq: u64,
    service_pc: u64,
    service_frame: u64,
    service_work: u64,
    eval_ppi: u64,
    service_ppi: u64,
}

#[repr(C)]
struct Witness {
    count: u64,
    frequency: u64,
    origin: u64,
    slot_ticks: u64,
    eval_ticks: u64,
    fallback_ticks: u64,
    watchdog: u64,
    samples: [Sample; SLOTS],
}

#[unsafe(no_mangle)]
static mut extension_witness: Witness = unsafe { core::mem::zeroed() };
const _: () = assert!(size_of::<Witness>() == 8 * (7 + SLOTS * 25));
static EPOCH: AtomicU64 = AtomicU64::new(0);
static HEARTBEAT: AtomicU64 = AtomicU64::new(0);
static FIRST_WORK: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];
static mut STACKS: [Stack; 5] = [const { Stack([0; STACK_BYTES]) }; 5];

#[derive(Clone, Copy)]
struct Request {
    slot: usize,
    deadline: u64,
    policy: Extension<3>,
    tasks: [DelegatedTask; 3],
}
#[derive(Clone, Copy)]
struct Reply {
    policy: Extension<3>,
    decision: Decision,
    slot: usize,
}
static mut REQUEST: Option<Request> = None;
static mut REPLY: Option<Reply> = None;
static mut ACTIVE: Option<Bench> = None;

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"EXTENSION FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn counter() -> u64 {
    let now;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) now, options(nostack, preserves_flags)) };
    now
}

fn arm(compare: u64) {
    require(counter() < compare);
    unsafe {
        asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
            in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags))
    };
}

// Run the data VM on the evaluator stack with IRQs enabled. A timed-out
// calculation can only abandon a private copy of the policy. No borrow of
// trusted scheduler state crosses an interruptible instruction here.
extern "C" fn evaluate() {
    let mut request = unsafe { ptr::read_volatile(&raw const REQUEST) }.unwrap_or_else(|| fail());
    let progress = HEARTBEAT.load(Ordering::Acquire) + 1;
    let decision = request.policy.decide(
        request.slot as u64,
        progress,
        &request.tasks,
        1,
        1,
        program(request.slot),
        FUEL,
    );
    unsafe {
        asm!("msr daifset, #2", options(nostack, preserves_flags));
        if counter() >= request.deadline {
            return;
        }
        ptr::write_volatile(
            &raw mut REPLY,
            Some(Reply {
                policy: request.policy,
                decision,
                slot: request.slot,
            }),
        );
    }
    HEARTBEAT.store(progress, Ordering::Release);
}

fn top(id: usize) -> *mut u8 {
    require(id <= EVALUATOR);
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

fn cookie(id: usize) -> u64 {
    0xe100 + id as u64
}

fn fresh(id: usize, entry: unsafe extern "C" fn()) -> *mut Frame {
    // The evaluator is rebuilt only from boot or a different service stack.
    unsafe {
        let frame = top(id).sub(size_of::<Frame>()).cast::<Frame>();
        ptr::write_bytes(frame, 0, 1);
        (*frame).elr = entry as usize as u64;
        (*frame).spsr = 0x3c5;
        (*frame).gpr[19] = cookie(id);
        frame
    }
}

fn work_range(id: usize, stopped: bool) -> (usize, usize) {
    let (begin, end) = match id {
        0 => (&raw const ext_reserved_work, &raw const ext_reserved_end),
        1 => (&raw const ext_fallback_work, &raw const ext_fallback_end),
        2 => (&raw const ext_nominee_work, &raw const ext_nominee_end),
        IDLE => (&raw const ext_idle_work, &raw const ext_idle_end),
        EVALUATOR if stopped => (
            &raw const ext_eval_stopped_work,
            &raw const ext_eval_stopped_end,
        ),
        EVALUATOR => (&raw const ext_eval_live_work, &raw const ext_eval_live_end),
        _ => fail(),
    };
    (begin.addr(), end.addr())
}

struct Work {
    pc: u64,
    counter: u64,
    epoch: u64,
}

fn check_work(frame: *mut Frame, id: usize, stopped: bool) -> Work {
    require(frame.addr() == top(id).addr() - 16 - size_of::<Frame>());
    let saved = unsafe { &*frame };
    let (begin, end) = work_range(id, stopped);
    require((begin..end).contains(&(saved.elr as usize)) && saved.spsr & 0x8f == 5);
    let tag = cookie(id);
    require(saved.gpr[19] == tag && saved.tpidr_el1 == tag && saved.simd[31] == [tag; 2]);
    require(unsafe { ptr::read_volatile(top(id).sub(16).cast::<u64>()) } == tag);
    Work {
        pc: saved.elr,
        counter: saved.gpr[20],
        epoch: saved.gpr[21],
    }
}

struct Bench {
    policy: Extension<3>,
    frames: [*mut Frame; 4],
    service_work: [u64; 4],
    current: usize,
    slot: usize,
    origin: u64,
    quantum: u64,
    row: Sample,
}

impl Bench {
    fn start(&self) -> u64 {
        self.origin + self.slot as u64 * self.quantum
    }

    fn begin_slot(&mut self) -> *mut Frame {
        let snapshot = tasks(self.slot);
        let p = program(self.slot).iter().find_map(|op| {
            if let Op::Propose(p) = op {
                Some(*p)
            } else {
                None
            }
        });
        self.row = Sample {
            slot: self.slot as u64,
            ready_mask: snapshot
                .iter()
                .enumerate()
                .fold(0, |m, (i, t)| m | (u64::from(t.ready) << i)),
            proposed_id: p.map_or(NONE, |p| p.task as u64),
            proposed_generation: p.map_or(0, |p| p.generation),
            proposed_cpu: p.map_or(NONE, |p| u64::from(p.cpu)),
            eval_ppi: NONE,
            ..Sample::default()
        };
        if self.slot as u64 % RESERVATION.period == RESERVATION.offset {
            let decision = self.trusted_decision();
            return self.run_service(decision);
        }
        self.current = EVALUATOR;
        let entry = if self.slot >= 6 {
            ext_eval_stopped
        } else {
            ext_eval_live
        };
        let frame = fresh(EVALUATOR, entry);
        self.row.eval_cval = self.start() + self.quantum / 4;
        unsafe {
            ptr::write_volatile(
                &raw mut REQUEST,
                Some(Request {
                    slot: self.slot,
                    deadline: self.row.eval_cval,
                    policy: self.policy,
                    tasks: snapshot,
                }),
            );
            ptr::write_volatile(&raw mut REPLY, None);
        }
        arm(self.row.eval_cval);
        frame
    }

    fn trusted_decision(&mut self) -> Decision {
        self.policy.decide(
            self.slot as u64,
            HEARTBEAT.load(Ordering::Acquire),
            &tasks(self.slot),
            1,
            1,
            &[],
            0,
        )
    }

    fn finish_eval(&mut self, frame: *mut Frame, compare: u64, irq: u64, ppi: u64) -> *mut Frame {
        require(compare == self.row.eval_cval && compare <= irq);
        let reply = unsafe { ptr::read_volatile(&raw const REPLY) };
        let stopped = self.slot >= 6;
        // The watchdog fixture is a real non-returning branch. A complete
        // reply waits in an interruptible loop. An incomplete VM has no
        // committable state, even if the timer caught a Rust stack frame.
        self.row.eval_work = if reply.is_some() || stopped {
            let work = check_work(frame, EVALUATOR, stopped).counter;
            require((work == 0) == stopped);
            work
        } else {
            let top = top(EVALUATOR).addr();
            require((top - STACK_BYTES..=top - size_of::<Frame>()).contains(&frame.addr()));
            0
        };
        self.row.eval_pc = unsafe { (*frame).elr };
        self.row.eval_frame = frame.addr() as u64;
        self.row.eval_irq = irq;
        self.row.eval_ppi = ppi;
        let decision = if let Some(reply) = reply {
            require(!stopped && reply.slot == self.slot);
            self.policy = reply.policy;
            reply.decision
        } else {
            self.trusted_decision()
        };
        self.run_service(decision)
    }

    fn run_service(&mut self, decision: Decision) -> *mut Frame {
        let chosen = decision.selected;
        let id = chosen.map_or(IDLE, |p| p.task);
        if let Some(p) = chosen {
            let task = tasks(self.slot)[id];
            require(task.ready && p.generation == task.generation && p.cpu == 0 && p.ticks == 1);
        }
        self.current = id;
        self.row.reason = decision.reason as u64;
        self.row.steps = u64::from(decision.steps);
        self.row.selected = chosen.map_or(NONE, |p| p.task as u64);
        self.row.grant = chosen.map_or(0, |p| p.ticks);
        self.row.progress = HEARTBEAT.load(Ordering::Acquire);
        let next = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!next.is_null());
        self.row.dispatch_pc = unsafe { (*next).elr };
        self.row.decision_time = counter();
        self.row.service_cval = self.start() + self.quantum;
        EPOCH.store(self.slot as u64 + 1, Ordering::Release);
        arm(self.row.service_cval);
        next
    }

    fn finish_service(
        &mut self,
        frame: *mut Frame,
        compare: u64,
        irq: u64,
        ppi: u64,
    ) -> *mut Frame {
        let id = self.current;
        let saved = check_work(frame, id, false);
        require(
            compare == self.row.service_cval && compare <= irq && irq < compare + self.quantum / 4,
        );
        self.row.first_work = FIRST_WORK[self.slot].load(Ordering::Acquire);
        require(
            self.row.decision_time <= self.row.first_work
                && self.row.first_work < self.start() + self.quantum / 2,
        );
        require(saved.counter > self.service_work[id] && saved.epoch == self.slot as u64 + 1);
        self.row.service_pc = saved.pc;
        self.row.service_frame = frame.addr() as u64;
        self.row.service_irq = irq;
        self.row.service_ppi = ppi;
        self.row.service_work = saved.counter;
        self.service_work[id] = saved.counter;
        self.frames[id] = frame;
        // Compare the independently recorded guest result only after the
        // selected task actually ran and its absolute timer grant retired.
        let (reason, selected, steps) = EXPECTED[self.slot];
        require(
            self.row.reason == reason as u64
                && self.row.selected == selected
                && self.row.steps == steps,
        );
        unsafe {
            ptr::write_volatile(&raw mut extension_witness.samples[self.slot], self.row);
            ptr::write_volatile(&raw mut extension_witness.count, self.slot as u64 + 1);
        }
        self.slot += 1;
        if self.slot == SLOTS {
            report();
            park()
        }
        self.begin_slot()
    }
}

// Print once all timed windows have retired so PL011 cannot alter their timing.
fn report() {
    let mut reserved = 0;
    for i in 0..SLOTS {
        let row = unsafe { ptr::read_volatile(&raw const extension_witness.samples[i]) };
        if row.reason == Reason::Reserved as u64 && row.selected == 0 {
            reserved += 1;
            continue;
        }
        let line: &[u8] = match (
            row.reason,
            row.selected,
            row.proposed_cpu,
            row.proposed_generation,
        ) {
            (0, 2, 0, 17) => b"EXTENSION PROPOSAL ACCEPTED\n",
            (1, 1, 1, 17) => b"EXTENSION MASK FALLBACK\n",
            (1, 1, 0, 16) => b"EXTENSION STALE FALLBACK\n",
            (2, 1, NONE, 0) => b"EXTENSION FUEL FALLBACK\n",
            (3, 1, NONE, 0) => b"EXTENSION WATCHDOG FALLBACK\n",
            (3, NONE, NONE, 0) => b"EXTENSION EMPTY IDLE\n",
            _ => fail(),
        };
        uart_write(line);
    }
    require(reserved == 3);
    uart_write(b"EXTENSION RESERVED THREE\nEXTENSION PASS\n");
}

const EXPECTED: [(Reason, u64, u64); SLOTS] = [
    (Reason::Accepted, 2, 2),
    (Reason::Rejected, 1, 1),
    (Reason::Reserved, 0, 0),
    (Reason::Rejected, 1, 1),
    (Reason::Fuel, 1, 5),
    (Reason::Reserved, 0, 0),
    (Reason::Stalled, 1, 0),
    (Reason::Stalled, NONE, 0),
    (Reason::Reserved, 0, 0),
];

pub(super) fn run() -> ! {
    init_timer_irq();
    uart_write(b"EXTENSION BEGIN\n");
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let quantum = frequency / 10;
    require(quantum >= 4);
    let policy = Extension::new(RESERVATION, 0, 0, WATCHDOG).unwrap_or_else(|| fail());
    let entries: [unsafe extern "C" fn(); 4] = [ext_reserved, ext_fallback, ext_nominee, ext_idle];
    let frames = core::array::from_fn(|id| fresh(id, entries[id]));
    let origin = counter();
    require(origin.checked_add(quantum * (SLOTS as u64 + 1)).is_some());
    unsafe {
        ptr::write_volatile(&raw mut extension_witness.frequency, frequency);
        ptr::write_volatile(&raw mut extension_witness.origin, origin);
        ptr::write_volatile(&raw mut extension_witness.slot_ticks, quantum);
        ptr::write_volatile(&raw mut extension_witness.eval_ticks, quantum / 4);
        ptr::write_volatile(&raw mut extension_witness.fallback_ticks, quantum / 2);
        ptr::write_volatile(&raw mut extension_witness.watchdog, WATCHDOG);
        ACTIVE = Some(Bench {
            policy,
            frames,
            service_work: [0; 4],
            current: IDLE,
            slot: 0,
            origin,
            quantum,
            row: Sample::default(),
        });
        sched_start_frame(
            (*(&raw mut ACTIVE))
                .as_mut()
                .unwrap_or_else(|| fail())
                .begin_slot(),
        )
    }
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let irq = counter();
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    let compare: u64;
    unsafe {
        asm!("mrs {}, cntp_ctl_el0", "mrs {}, cntp_cval_el0", out(reg) control,
        out(reg) compare, options(nostack, preserves_flags))
    };
    let ppi = u64::from(iar & 0x3ff);
    require(ppi == u64::from(TIMER_IRQ) && control & 7 == 5);
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
    // Boot and the masked IRQ on CPU0 exclusively own ACTIVE; the VM uses
    // a separate working copy on its own interruptible stack.
    let bench = unsafe { (*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail()) };
    if bench.current == EVALUATOR {
        bench.finish_eval(frame, compare, irq, ppi)
    } else {
        bench.finish_service(frame, compare, irq, ppi)
    }
}
