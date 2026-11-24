//! Bounded single-core inversion, transitive timeout and immediate-ceiling bench.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::mutex_policy::{Acquire, LockSet, LockSpec, Protocol};
use super::ready_queue::ReadyQueue;
use super::{park, uart_write};

const BASE: usize = 0;
const LOW: usize = 1;
const MEDIUM: usize = 2;
const HIGH: usize = 3;
const BASE_PRIORITIES: [u8; 4] = [1, 2, 5, 9];
const NONE: u8 = 255;
const LOCK: u8 = 1;
const UNLOCK: u8 = 2;
const RELEASE: u8 = 3;
const YIELD: u8 = 4;
const EXIT: u8 = 5;
const EXPIRY: u8 = 6;

unsafe extern "C" {
    fn mutex_classic_low();
    fn mutex_classic_high();
    fn mutex_medium();
    fn mutex_chain_base();
    fn mutex_chain_low();
    fn mutex_chain_high();
    fn mutex_ceiling_low();
    fn mutex_ceiling_high();
    static mutex_classic_handoff: u8;
    static mutex_chain_handoff: u8;
    static mutex_high_timeout: u8;
    static mutex_ceiling_handoff: u8;
    static mutex_expiry_begin: u8;
    static mutex_expiry_end: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("mutex.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym service_call, fail = sym fail, expired = sym EXPIRED,
);

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Dormant,
    Ready,
    Running,
    Waiting,
    Finished,
}

// These records describe the outgoing exception and the result of scheduling.
// ready includes the selected task. Wait masks and owners are indexed by X,Y.
#[repr(C)]
#[derive(Clone, Copy)]
struct Sample {
    pc: u64,
    frame: u64,
    epoch: u8,
    op: u8,
    argument: u8,
    caller: u8,
    selected: u8,
    ready: u8,
    wait: [u8; 2],
    owner: [u8; 2],
    effective: [u8; 4],
    woken: u8,
    ppi: u8,
}

#[repr(C)]
struct Witness {
    count: u64,
    deadline: u64,
    observed: u64,
    samples: [Sample; 40],
}

// The debugger reads this one symbol after the guest parks. Preserve writes
// independently of the guest's internal checks and the UART verdict.
#[unsafe(no_mangle)]
static mut mutex_witness: Witness = Witness {
    count: 0,
    deadline: 0,
    observed: 0,
    samples: [Sample {
        pc: 0,
        frame: 0,
        epoch: 0,
        op: 0,
        argument: 0,
        caller: 0,
        selected: 0,
        ready: 0,
        wait: [0; 2],
        owner: [0; 2],
        effective: [0; 4],
        woken: 0,
        ppi: 0,
    }; 40],
};

const _: () = assert!(size_of::<Sample>() == 32);
static mut STACKS: [Stack; 12] = [const { Stack([0; STACK_BYTES]) }; 12];
static mut ACTIVE: Option<Bench> = None;
static EXPIRED: AtomicU64 = AtomicU64::new(0);

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"MUTEX FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn tag(id: Option<usize>) -> u8 {
    id.map_or(NONE, |n| n as u8)
}

fn counter() -> u64 {
    let value;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack, preserves_flags)) };
    value
}

struct Bench {
    epoch: usize,
    locks: LockSet<4, 2>,
    ready: ReadyQueue<4>,
    state: [State; 4],
    frames: [*mut Frame; 4],
    current: Option<usize>,
    step: usize,
    timeout_at: Option<u64>,
}

fn with_bench<R>(f: impl FnOnce(&mut Bench) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    // The only CPU borrows ACTIVE in masked boot or exception context.
    unsafe { f((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn new(epoch: usize) -> Self {
        let spec = if epoch == 2 {
            [
                LockSpec {
                    users: 0b0110,
                    protocol: Protocol::Immediate(5),
                },
                LockSpec {
                    users: 0b1010,
                    protocol: Protocol::Immediate(9),
                },
            ]
        } else {
            [
                LockSpec {
                    users: 0b1010,
                    protocol: Protocol::Inherit,
                },
                LockSpec {
                    users: 0b0011,
                    protocol: Protocol::Inherit,
                },
            ]
        };
        Self {
            epoch,
            locks: LockSet::new(BASE_PRIORITIES, spec).unwrap_or_else(|| fail()),
            ready: ReadyQueue::new(),
            state: [State::Dormant; 4],
            frames: [ptr::null_mut(); 4],
            current: None,
            step: 0,
            timeout_at: None,
        }
    }

    fn top(&self, task: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.epoch * 4 + task + 1) * STACK_BYTES)
    }

    fn cookie(&self, task: usize) -> u64 {
        0x4900 + (self.epoch * 4 + task) as u64
    }

    fn start(&mut self) -> *mut Frame {
        let entries: [Option<unsafe extern "C" fn()>; 4] = match self.epoch {
            0 => [
                None,
                Some(mutex_classic_low),
                Some(mutex_medium),
                Some(mutex_classic_high),
            ],
            1 => [
                Some(mutex_chain_base),
                Some(mutex_chain_low),
                Some(mutex_medium),
                Some(mutex_chain_high),
            ],
            2 => [
                None,
                Some(mutex_ceiling_low),
                Some(mutex_medium),
                Some(mutex_ceiling_high),
            ],
            _ => fail(),
        };
        for (task, entry) in entries.into_iter().enumerate() {
            if let Some(entry) = entry {
                // Each epoch has untouched BSS-backed private stacks.
                unsafe {
                    let f = self.top(task).sub(size_of::<Frame>()).cast::<Frame>();
                    (*f).elr = entry as usize as u64;
                    (*f).spsr = 0x3c5; // EL1h. BASE unmasks IRQ in its work range.
                    (*f).gpr[19] = self.cookie(task);
                    self.frames[task] = f;
                }
            }
        }
        self.enqueue(if self.epoch == 1 { BASE } else { LOW }, false);
        self.dispatch().unwrap_or_else(|| fail())
    }

    fn enqueue(&mut self, task: usize, front: bool) {
        require(!self.frames[task].is_null() && self.locks.waiting(task).is_none());
        require(!matches!(self.state[task], State::Ready | State::Finished));
        require(if front {
            self.ready.push_front(task)
        } else {
            self.ready.push_back(task)
        });
        self.state[task] = State::Ready;
    }

    fn dispatch(&mut self) -> Option<*mut Frame> {
        require(self.current.is_none());
        let locks = &self.locks;
        let next = self
            .ready
            .pop_highest(|id| locks.effective(id).unwrap_or_else(|| fail()))?;
        require(self.state[next] == State::Ready);
        self.state[next] = State::Running;
        self.current = Some(next);
        let frame = core::mem::replace(&mut self.frames[next], ptr::null_mut());
        require(!frame.is_null());
        Some(frame)
    }

    fn save(&mut self, frame: *mut Frame) -> usize {
        let caller = self.current.take().unwrap_or_else(|| fail());
        require(self.state[caller] == State::Running && self.frames[caller].is_null());
        let top = self.top(caller);
        require(frame.addr() == top.addr() - size_of::<Frame>() - 16);
        let saved = unsafe { &*frame };
        let word = unsafe { ptr::read_volatile(top.sub(16).cast::<u64>()) };
        require(word == self.cookie(caller) && saved.gpr[19] == word && saved.tpidr_el1 == word);
        require(saved.spsr & 0xf == 5);
        self.frames[caller] = frame;
        caller
    }

    fn continuation(&self, task: usize) -> usize {
        match (self.epoch, task) {
            (0, HIGH) => (&raw const mutex_classic_handoff).addr(),
            (1, LOW) => (&raw const mutex_chain_handoff).addr(),
            (1, HIGH) => (&raw const mutex_high_timeout).addr(),
            (2, HIGH) => (&raw const mutex_ceiling_handoff).addr(),
            _ => fail(),
        }
    }

    fn wake_waiter(&mut self, task: usize, status: u64) {
        require(self.state[task] == State::Waiting && !self.frames[task].is_null());
        let frame = self.frames[task];
        require(unsafe { (*frame).elr as usize } == self.continuation(task));
        unsafe { (*frame).gpr[0] = status };
        self.enqueue(task, false);
    }

    fn arm_timeout(&mut self) {
        let frequency: u64;
        unsafe {
            asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags))
        };
        require(self.timeout_at.is_none() && frequency >= 100);
        let deadline = counter()
            .checked_add(frequency / 100)
            .unwrap_or_else(|| fail());
        self.timeout_at = Some(deadline);
        unsafe {
            ptr::write_volatile(&raw mut mutex_witness.deadline, deadline);
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                in(reg) deadline, in(reg) 1u64, options(nostack, preserves_flags));
        }
    }

    fn apply(&mut self, caller: usize, op: u8, arg: u8, frame: *mut Frame) -> Option<usize> {
        let object = usize::from(arg);
        match op {
            LOCK => match self.locks.acquire(caller, object).unwrap_or_else(|| fail()) {
                Acquire::Held => unsafe { (*frame).gpr[0] = 0 },
                Acquire::Blocked => {
                    require(unsafe { (*frame).elr as usize } == self.continuation(caller));
                    self.state[caller] = State::Waiting;
                    if self.epoch == 1 && caller == HIGH {
                        self.arm_timeout()
                    }
                }
            },
            UNLOCK => {
                let successor = self.locks.release(caller, object).unwrap_or_else(|| fail());
                require(self.locks.owner(object) == successor);
                if let Some(task) = successor {
                    self.wake_waiter(task, 0)
                }
                return successor;
            }
            RELEASE => {
                require(arg > 0 && arg < 16);
                for task in 0..4 {
                    if arg & (1 << task) != 0 {
                        require(self.state[task] == State::Dormant);
                        self.enqueue(task, false);
                    }
                }
            }
            YIELD => {}
            EXIT => {
                require((0..2).all(|lock| self.locks.owner(lock) != Some(caller)));
                self.state[caller] = State::Finished;
            }
            EXPIRY => {
                let deadline = self.timeout_at.take().unwrap_or_else(|| fail());
                let now = counter();
                require(now >= deadline && self.locks.waiting(object) == Some(0));
                require(self.locks.timeout(object));
                self.wake_waiter(object, 1);
                unsafe { ptr::write_volatile(&raw mut mutex_witness.observed, now) };
                EXPIRED.store(1, Ordering::Release);
                return Some(object);
            }
            _ => fail(),
        }
        None
    }

    fn transition(&mut self, frame: *mut Frame, op: u8, arg: u8, ppi: u8) -> *mut Frame {
        let caller = self.save(frame);
        let pc = unsafe { (*frame).elr };
        let woken = self.apply(caller, op, arg, frame);
        if self.state[caller] == State::Running {
            self.enqueue(caller, op != YIELD)
        }
        let next = self.dispatch();
        let mut sample = Sample {
            pc,
            frame: frame.addr() as u64,
            epoch: self.epoch as u8,
            op,
            argument: arg,
            caller: caller as u8,
            selected: tag(self.current),
            ready: 0,
            wait: [0; 2],
            owner: core::array::from_fn(|i| tag(self.locks.owner(i))),
            effective: core::array::from_fn(|i| self.locks.effective(i).unwrap_or_else(|| fail())),
            woken: tag(woken),
            ppi,
        };
        for task in 0..4 {
            if matches!(self.state[task], State::Ready | State::Running) {
                sample.ready |= 1 << task
            }
            if let Some(lock) = self.locks.waiting(task) {
                require(self.state[task] == State::Waiting);
                sample.wait[lock] |= 1 << task;
            } else {
                require(self.state[task] != State::Waiting);
            }
        }
        // Record the actual dispatch before evaluating the acceptance oracle.
        unsafe {
            let index = ptr::read_volatile(&raw const mutex_witness.count) as usize;
            require(index < 40);
            ptr::write_volatile(&raw mut mutex_witness.samples[index], sample);
            ptr::write_volatile(&raw mut mutex_witness.count, index as u64 + 1);
        }
        check_sample(self.step, sample);
        self.step += 1;
        if let Some(next) = next {
            return next;
        }
        require(self.step == SEQUENCES[self.epoch].len() && sample.owner == [NONE; 2]);
        require(sample.wait == [0; 2] && sample.effective == BASE_PRIORITIES);
        uart_write(
            [
                b"MUTEX PI TRANSFER\n" as &[u8],
                b"MUTEX CHAIN TIMEOUT\n",
                b"MUTEX CEILING NESTED\n",
            ][self.epoch],
        );
        if self.epoch == 2 {
            uart_write(b"MUTEX PASS\n");
            park()
        }
        *self = Self::new(self.epoch + 1);
        self.start()
    }
}

// Triples are caller, operation and expected selected task. The dispatcher
// has already removed the actual winner before this oracle is consulted.
const SEQUENCES: [&[[u8; 3]]; 3] = [
    &[
        [1, LOCK, 1],
        [1, RELEASE, 3],
        [3, LOCK, 1],
        [1, UNLOCK, 3],
        [3, UNLOCK, 3],
        [3, EXIT, 2],
        [2, EXIT, 1],
        [1, EXIT, NONE],
    ],
    &[
        [0, LOCK, 0],
        [0, RELEASE, 1],
        [1, LOCK, 1],
        [1, LOCK, 0],
        [0, RELEASE, 3],
        [3, LOCK, 0],
        [0, EXPIRY, 3],
        [3, EXIT, 2],
        [2, EXIT, 0],
        [0, UNLOCK, 1],
        [1, UNLOCK, 1],
        [1, UNLOCK, 1],
        [1, EXIT, 0],
        [0, EXIT, NONE],
    ],
    &[
        [1, LOCK, 1],
        [1, RELEASE, 1],
        [1, LOCK, 1],
        [1, RELEASE, 1],
        [1, YIELD, 3],
        [3, LOCK, 1],
        [1, UNLOCK, 3],
        [3, UNLOCK, 3],
        [3, EXIT, 1],
        [1, UNLOCK, 2],
        [2, EXIT, 1],
        [1, EXIT, NONE],
    ],
];

fn check_sample(step: usize, s: Sample) {
    require(SEQUENCES[s.epoch as usize].get(step) == Some(&[s.caller, s.op, s.selected]));
    let medium_ready = s.ready & (1 << MEDIUM) != 0;
    let ok = match (s.epoch, step) {
        (0, 2) => medium_ready && s.wait == [8, 0] && s.owner[0] == 1 && s.effective[LOW] == 9,
        (0, 3) => s.woken == 3 && s.owner[0] == 3 && s.effective[LOW] == 2,
        (1, 4) => medium_ready && s.owner == [1, 0] && s.wait == [0, 2] && s.effective[BASE] == 2,
        (1, 5) => medium_ready && s.wait == [8, 2] && s.effective == [9, 9, 5, 9],
        (1, 6) => {
            medium_ready
                && s.wait == [0, 2]
                && s.owner == [1, 0]
                && s.woken == 3
                && s.effective == [2, 2, 5, 9]
        }
        (1, 9) => s.woken == 1 && s.owner == [1, 1] && s.effective[BASE] == 1,
        (2, 0) => s.wait == [0; 2] && s.owner == [1, NONE] && s.effective[LOW] == 5,
        (2, 2) => medium_ready && s.wait == [0; 2] && s.owner == [1, 1] && s.effective[LOW] == 9,
        (2, 5) => medium_ready && s.wait == [0, 8] && s.effective[LOW] == 9,
        (2, 6) => s.owner == [1, 3] && s.woken == 3 && s.effective[LOW] == 5,
        (2, 9) => medium_ready && s.owner == [NONE; 2] && s.effective[LOW] == 2,
        _ => true,
    };
    require(ok);
}

pub(super) fn run() -> ! {
    init_timer_irq();
    unsafe {
        ACTIVE = Some(Bench::new(0));
        sched_start_frame(with_bench(Bench::start))
    }
}

extern "C" fn service_call(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    let immediate = esr & 0xffff;
    require(esr >> 26 == 0x15 && (1..=5).contains(&immediate));
    let arg = if immediate <= 3 {
        u8::try_from(unsafe { (*frame).gpr[0] }).unwrap_or_else(|_| fail())
    } else {
        0
    };
    with_bench(|b| b.transition(frame, immediate as u8, arg, NONE))
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5);
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
    with_bench(|b| {
        require(b.epoch == 1 && b.current == Some(BASE) && EXPIRED.load(Ordering::Relaxed) == 0);
        require(frame.addr() == b.top(BASE).addr() - size_of::<Frame>() - 16);
        let saved = unsafe { &*frame };
        let work = (&raw const mutex_expiry_begin).addr()..(&raw const mutex_expiry_end).addr();
        require(
            work.contains(&(saved.elr as usize)) && saved.spsr & 0x8f == 5 && saved.gpr[20] > 0,
        );
        unsafe { ptr::write_volatile(b.top(BASE).sub(8).cast::<u64>(), saved.gpr[20]) };
        b.transition(frame, EXPIRY, HIGH as u8, (iar & 0x3ff) as u8)
    })
}
