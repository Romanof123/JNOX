//! Six non-yielding slots on one AArch64 CPU with GICv2/CNTP enforcement.
//! The fixed workload admits one active job per task. Both LO jobs and the
//! second HI job have received zero service when the first HI job overruns.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::amc_policy::{AmcRuntime, Dispatch, TaskSpec};
use super::context::*;
use super::domain_policy::{CriticalityMode, ExecutionBudget};
use super::{park, uart_write};

const TASKS: usize = 4;
const SLOTS: usize = 6;
const OWNERS: [usize; SLOTS] = [0, 1, 1, 1, 3, 0];
// One stack per release, including the two discarded releases. The fresh LO
// frame after reset is disjoint from both previous generations of that task.
const INSTANCES: usize = 6;
static mut STACKS: [Stack; INSTANCES] = [const { Stack([0; STACK_BYTES]) }; INSTANCES];
static mut ACTIVE: Option<Bench> = None;
static HI_SIGNAL: AtomicU64 = AtomicU64::new(0);
static HI_WORK: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    fn amc_lo_initial();
    fn amc_hi();
    fn amc_lo_pending();
    fn amc_survivor();
    fn amc_lo_fresh();
    static amc_lo_initial_begin: u8;
    static amc_lo_initial_end: u8;
    static amc_hi_begin: u8;
    static amc_hi_end: u8;
    static amc_survivor_begin: u8;
    static amc_survivor_end: u8;
    static amc_lo_fresh_begin: u8;
    static amc_lo_fresh_end: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("amc.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym fail, fail = sym fail,
    signal = sym HI_SIGNAL, observation = sym HI_WORK,
);

#[derive(Clone, Copy, Default)]
struct Sample {
    id: usize,
    hi_mode: bool,
    end: u64,
    used: u64,
    remaining: u64,
    deadline: u64,
    compare: u64,
    irq: u64,
    elr: u64,
    sp: u64,
    progress: u64,
    value: u64,
}

struct Bench {
    policy: AmcRuntime<TASKS>,
    frequency: u64,
    quantum: u64,
    origin: u64,
    current: usize,
    instances: [usize; TASKS],
    frames: [*mut Frame; TASKS],
    samples: [Sample; SLOTS],
    carry: [u64; TASKS],
    discarded: u32,
    commit: u64,
    denied_release: bool,
    early_reset: bool,
    fault: Option<usize>,
    unapproved: bool,
    approved: bool,
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"AMC FAIL\n");
    park()
}

fn require(condition: bool) {
    if !condition {
        fail()
    }
}

fn counter() -> u64 {
    let now;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) now, options(nostack, preserves_flags)) };
    now
}

fn top(instance: usize) -> *mut u8 {
    require(instance < INSTANCES);
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((instance + 1) * STACK_BYTES)
}

fn witness(instance: usize) -> u64 {
    0xac00 + instance as u64
}

fn with_bench<T>(f: impl FnOnce(&mut Bench) -> T) -> T {
    let mask: u64;
    unsafe { asm!("mrs {}, daif", out(reg) mask, options(nostack, preserves_flags)) };
    require(mask & 0x80 != 0);
    // Boot and the IRQ handler serialize all accesses on the sole CPU. A
    // borrow ends before exception return. Assembly touches only the atomics.
    unsafe { f((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn new(frequency: u64) -> Self {
        use ExecutionBudget::{High, Low};
        let specs = [
            TaskSpec {
                budget: Low(2),
                priority: 1,
                relative_deadline: 3,
            },
            TaskSpec {
                budget: High { lo: 1, hi: 3 },
                priority: 4,
                relative_deadline: 4,
            },
            TaskSpec {
                budget: Low(2),
                priority: 2,
                relative_deadline: 3,
            },
            TaskSpec {
                budget: High { lo: 1, hi: 1 },
                priority: 3,
                relative_deadline: 5,
            },
        ];
        require(frequency >= 20);
        Self {
            policy: AmcRuntime::new(specs).unwrap_or_else(|| fail()),
            frequency,
            quantum: frequency / 20,
            origin: 0,
            current: TASKS,
            instances: [INSTANCES; TASKS],
            frames: [ptr::null_mut(); TASKS],
            samples: [Sample::default(); SLOTS],
            carry: [0; TASKS],
            discarded: 0,
            commit: 0,
            denied_release: false,
            early_reset: false,
            fault: None,
            unapproved: false,
            approved: false,
        }
    }

    fn boundary(&self, t: u64) -> u64 {
        self.quantum
            .checked_mul(t)
            .and_then(|delta| self.origin.checked_add(delta))
            .unwrap_or_else(|| fail())
    }

    fn release(&mut self, id: usize, work: u64, instance: usize, entry: unsafe extern "C" fn()) {
        require(self.frames[id].is_null() && self.policy.release(id, work));
        self.instances[id] = instance;
        // Each release gets a separate zero-initialized stack. The running
        // handler may still be using an older generation of the same task.
        unsafe {
            let frame = top(instance).sub(size_of::<Frame>()).cast::<Frame>();
            (*frame).elr = entry as usize as u64;
            (*frame).spsr = 0x3c5;
            (*frame).gpr[19] = witness(instance);
            self.frames[id] = frame;
        }
    }

    fn arrivals(&mut self, t: u64) {
        match t {
            0 => self.release(0, 1, 0, amc_lo_initial),
            1 => {
                self.release(0, 2, 1, amc_lo_pending);
                self.release(1, 4, 2, amc_hi);
                self.release(2, 2, 3, amc_lo_pending);
                self.release(3, 1, 4, amc_survivor);
            }
            5 => {
                self.unapproved = self.policy.reset(false);
                require(!self.unapproved && self.policy.mode() == CriticalityMode::High);
                self.approved = self.policy.reset(true);
                require(self.approved && self.policy.mode() == CriticalityMode::Low);
                self.release(0, 1, 5, amc_lo_fresh);
            }
            _ => {}
        }
    }

    fn switch(&mut self, discarded: u32) {
        self.commit = counter();
        require(self.carry == [2, 3, 2, 1] && discarded == 5);
        self.discarded = discarded;
        for id in [0, 2] {
            require(self.policy.remaining(id).is_none());
            let frame = self.frames[id];
            require(
                !frame.is_null() && unsafe { (*frame).elr } == amc_lo_pending as *const () as u64,
            );
            self.frames[id] = ptr::null_mut();
        }
        self.denied_release = !self.policy.release(2, 1);
        self.early_reset = self.policy.reset(true);
        require(
            self.denied_release && !self.early_reset && self.policy.mode() == CriticalityMode::High,
        );
        HI_SIGNAL.store(1, Ordering::Release);
    }

    fn dispatch(&mut self) -> *mut Frame {
        let t = self.policy.now();
        self.arrivals(t);
        if t == 2 {
            require(self.policy.mode() == CriticalityMode::Low);
            self.carry =
                core::array::from_fn(|id| self.policy.remaining(id).unwrap_or_else(|| fail()));
        }
        let mut decision = self.policy.start_slot().unwrap_or_else(|| fail());
        if let Dispatch::Exhausted { task } = decision {
            require(t == 4 && task == 1 && self.fault.is_none());
            require(self.samples[3].used == 3 && self.samples[3].remaining == 1);
            require(self.policy.remaining(task).is_none() && !self.frames[task].is_null());
            self.fault = Some(task);
            self.frames[task] = ptr::null_mut();
            require(self.policy.now() == t);
            decision = self.policy.start_slot().unwrap_or_else(|| fail());
        }
        let Dispatch::Run { task, discarded } = decision else {
            fail()
        };
        require(OWNERS.get(t as usize) == Some(&task));
        let hi_mode = self.policy.mode() == CriticalityMode::High;
        require(hi_mode == (2..=4).contains(&t));
        if t == 2 {
            self.switch(discarded)
        } else {
            require(discarded == 0)
        }

        let frame = core::mem::replace(&mut self.frames[task], ptr::null_mut());
        require(!frame.is_null());
        if task == 1 && t > 1 {
            let prior = self.samples[t as usize - 1];
            require(unsafe { (*frame).elr == prior.elr && (*frame).gpr[20] == prior.progress });
        }
        self.current = task;
        self.samples[t as usize].hi_mode = hi_mode;
        let compare = self.boundary(t + 1);
        require(counter() < compare);
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
            in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags))
        };
        frame
    }

    fn retire(&mut self, frame: *mut Frame, compare: u64, irq: u64) -> *mut Frame {
        let t = self.policy.now();
        let id = self.current;
        let instance = self.instances[id];
        require(OWNERS.get(t as usize) == Some(&id) && self.frames[id].is_null());
        require(compare == self.boundary(t + 1) && (compare..self.boundary(t + 2)).contains(&irq));
        let sp = top(instance).addr() - 16;
        require(frame.addr() == sp - size_of::<Frame>());
        let saved = unsafe { &*frame };
        let (begin, end, result) = match instance {
            0 => (
                &raw const amc_lo_initial_begin,
                &raw const amc_lo_initial_end,
                36,
            ),
            2 => (&raw const amc_hi_begin, &raw const amc_hi_end, 0),
            4 => (
                &raw const amc_survivor_begin,
                &raw const amc_survivor_end,
                528,
            ),
            5 => (
                &raw const amc_lo_fresh_begin,
                &raw const amc_lo_fresh_end,
                136,
            ),
            _ => fail(),
        };
        let tag = witness(instance);
        require((begin.addr()..end.addr()).contains(&(saved.elr as usize)));
        require(saved.spsr & 0x8f == 5 && saved.tpidr_el1 == tag && saved.gpr[19] == tag);
        require(
            saved.simd[31] == [tag; 2] && unsafe { ptr::read_volatile(sp as *const u64) } == tag,
        );
        require(saved.gpr[23] == result && saved.gpr[20] > 1);
        if id == 1 && t > 1 {
            require(saved.gpr[20] > self.samples[t as usize - 1].progress);
        }
        self.frames[id] = frame;
        let completed = self.policy.finish_slot().unwrap_or_else(|| fail());
        require(completed.task == id && completed.end == t + 1 && !completed.deadline_missed());
        if completed.remaining == 0 {
            self.frames[id] = ptr::null_mut();
        }
        let row = &mut self.samples[t as usize];
        *row = Sample {
            id,
            hi_mode: row.hi_mode,
            end: completed.end,
            used: completed.used,
            remaining: completed.remaining,
            deadline: completed.deadline,
            compare,
            irq,
            elr: saved.elr,
            sp: sp as u64,
            progress: saved.gpr[20],
            value: saved.gpr[23],
        };
        if t == 4 {
            require(completed.remaining == 0 && completed.deadline == 6 && irq < self.boundary(6));
        }
        if completed.end == SLOTS as u64 {
            self.report();
            park()
        }
        self.dispatch()
    }

    fn report(&self) {
        let work = HI_WORK.load(Ordering::Acquire);
        let trigger = self.samples[1];
        require(
            trigger.irq <= self.commit && self.commit <= work && work < self.samples[2].compare,
        );
        require(self.fault == Some(1) && self.frames.iter().all(|frame| frame.is_null()));
        require((0..TASKS).all(|id| self.policy.remaining(id).is_none()));
        uart_write(b"AMC CLOCK");
        field(b" freq=", self.frequency);
        field(b" origin=", self.origin);
        field(b" quantum=", self.quantum);
        uart_write(b"\n");
        for (t, row) in self.samples.iter().enumerate() {
            uart_write(b"AMC SLOT");
            field(b" t=", t as u64);
            field(b" id=", row.id as u64);
            field(b" mode=", u64::from(row.hi_mode));
            field(b" used=", row.used);
            field(b" left=", row.remaining);
            field(b" deadline=", row.deadline);
            field(b" cval=", row.compare);
            field(b" irq=", row.irq);
            field(b" elr=", row.elr);
            field(b" sp=", row.sp);
            field(b" progress=", row.progress);
            field(b" value=", row.value);
            uart_write(b"\n");
        }
        uart_write(b"AMC SWITCH t=2 carry=");
        for (id, remaining) in self.carry.iter().enumerate() {
            if id != 0 {
                uart_write(b",")
            }
            number(*remaining);
        }
        field(b" drop=", self.discarded.into());
        field(b" irq=", trigger.irq);
        field(b" commit=", self.commit);
        field(b" work=", work);
        field(b" latency=", work - trigger.compare);
        uart_write(b"\nAMC GUARD");
        field(b" new_lo=", u64::from(!self.denied_release));
        field(b" early_reset=", u64::from(self.early_reset));
        field(b" fourth=", u64::from(self.fault.is_none()));
        field(b" fault=", self.fault.unwrap_or_else(|| fail()) as u64);
        let survivor = self.samples[4];
        uart_write(b"\nAMC DEADLINE");
        field(b" id=", survivor.id as u64);
        field(b" end=", survivor.end);
        field(b" due=", survivor.deadline);
        field(b" irq=", survivor.irq);
        field(b" limit=", self.boundary(survivor.deadline));
        field(b" value=", survivor.value);
        uart_write(b"\nAMC RESET");
        field(b" unapproved=", u64::from(self.unapproved));
        field(b" approved=", u64::from(self.approved));
        uart_write(b"\nAMC STOP\nAMC PASS\n");
    }
}

fn number(mut n: u64) {
    let mut digits = [0; 20];
    let mut first = digits.len();
    loop {
        first -= 1;
        digits[first] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&digits[first..]);
}

fn field(name: &[u8], n: u64) {
    uart_write(name);
    number(n);
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    uart_write(b"AMC BEGIN\n");
    unsafe {
        ACTIVE = Some(Bench::new(frequency));
        let first = with_bench(|bench| {
            bench.origin = counter();
            bench.dispatch()
        });
        sched_start_frame(first)
    }
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let irq = counter();
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    let compare: u64;
    unsafe {
        asm!("mrs {}, cntp_ctl_el0", "mrs {}, cntp_cval_el0",
        out(reg) control, out(reg) compare, options(nostack, preserves_flags))
    };
    require(iar & 0x3ff == TIMER_IRQ && control & 7 == 5);
    unsafe {
        asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy",
        in(reg) 2u64, options(nostack, preserves_flags))
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|bench| bench.retire(frame, compare, irq))
}
