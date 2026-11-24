//! One-CPU strict-class and feedback dispatch with live timer/SVC accounting.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::*;
use super::multilevel_policy::{Multilevel, Stop};
use super::{park, uart_write};

const NONE: u8 = 255;
const YIELD: u8 = 1;
const SLEEP: u8 = 2;
const EXPIRE: u8 = 3;
const BOOST_PERIOD: u64 = 12; // Whole charged service ticks on this busy CPU.
const INTERACTIVE: usize = 0;

unsafe extern "C" {
    fn mlq_high_a();
    fn mlq_high_b();
    fn mlq_low();
    fn mlfq_yielder();
    fn mlfq_interactive();
    fn mlfq_cpu();
    static mlq_high_a_work: u8;
    static mlq_high_a_end: u8;
    static mlq_high_b_work: u8;
    static mlq_high_b_end: u8;
    static mlfq_cpu_work: u8;
    static mlfq_cpu_end: u8;
    static mlfq_yielder_work: u8;
    static mlfq_yielder_end: u8;
    static mlq_high_a_sleep: u8;
    static mlq_high_b_sleep: u8;
    static mlq_low_sleep: u8;
    static mlfq_after_yield_one: u8;
    static mlfq_after_yield_two: u8;
    static mlfq_yielder_sleep: u8;
    static mlfq_interactive_wake: u8;
    static mlfq_interactive_end: u8;
    static mlfq_cpu_sleep: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("multilevel.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym timer_irq, svc = sym task_call, fail = sym fail,
    expiries = sym EXPIRIES, one_tick = sym ONE_TICK,
);

// ELR and frame describe the outgoing context; next_pc is the selected ERET
// address. The two snapshots bracket the optional wake and global boost.
#[repr(C)]
#[derive(Clone, Copy)]
struct Sample {
    pc: u64,
    frame: u64,
    next_pc: u64,
    start: u64,
    stop: u64,
    deadline: u64,
    work: u64,
    epoch: u8,
    op: u8,
    caller: u8,
    selected: u8,
    grant: u8,
    charge: u8,
    total: u8,
    boost: u8,
    awake: u8,
    ppi: u8,
    rank: [u8; 3],
    left: [u8; 3],
    next_rank: [u8; 3],
    next_left: [u8; 3],
    reserved: [u8; 2],
}

#[repr(C)]
struct Witness {
    count: u64,
    tick: u64,
    samples: [Sample; 17],
}

#[unsafe(no_mangle)]
static mut multilevel_witness: Witness = Witness {
    count: 0,
    tick: 0,
    samples: [Sample {
        pc: 0,
        frame: 0,
        next_pc: 0,
        start: 0,
        stop: 0,
        deadline: 0,
        work: 0,
        epoch: 0,
        op: 0,
        caller: 0,
        selected: 0,
        grant: 0,
        charge: 0,
        total: 0,
        boost: 0,
        awake: 0,
        ppi: 0,
        rank: [0; 3],
        left: [0; 3],
        next_rank: [0; 3],
        next_left: [0; 3],
        reserved: [0; 2],
    }; 17],
};
const _: () = assert!(size_of::<Sample>() == 80);
static ONE_TICK: AtomicU64 = AtomicU64::new(0);
static EXPIRIES: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static mut STACKS: [Stack; 6] = [const { Stack([0; STACK_BYTES]) }; 6];
static mut ACTIVE: Option<Bench> = None;

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"MULTILEVEL FAIL\n");
    park()
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

fn counter() -> u64 {
    let value;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack, preserves_flags)) };
    value
}

fn disable_timer() {
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            "dsb sy",
            options(nostack, preserves_flags)
        )
    };
}

struct Bench {
    epoch: usize,
    policy: Multilevel<3>,
    frames: [*mut Frame; 3],
    last_work: [u64; 3],
    awake: u8,
    current: Option<usize>,
    tick: u64,
    start: u64,
    deadline: u64,
    total: u64,
    next_boost: u64,
    step: usize,
}

fn with_bench<R>(f: impl FnOnce(&mut Bench) -> R) -> R {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    // Only the masked boot CPU or masked exception callback accesses ACTIVE.
    unsafe { f((*(&raw mut ACTIVE)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn new(epoch: usize, tick: u64) -> Self {
        let policy = if epoch == 0 {
            Multilevel::fixed([0, 2, 2], 1)
        } else {
            Multilevel::feedback([3, 4, 6])
        }
        .unwrap_or_else(|| fail());
        Self {
            epoch,
            policy,
            frames: [ptr::null_mut(); 3],
            last_work: [0; 3],
            awake: 0,
            current: None,
            tick,
            start: 0,
            deadline: 0,
            total: 0,
            next_boost: BOOST_PERIOD,
            step: 0,
        }
    }

    fn top(&self, id: usize) -> *mut u8 {
        (&raw mut STACKS)
            .cast::<u8>()
            .wrapping_add((self.epoch * 3 + id + 1) * STACK_BYTES)
    }

    fn cookie(&self, id: usize) -> u64 {
        0x5000 + (3 * self.epoch + id) as u64
    }

    fn wake(&mut self, id: usize) {
        require(self.awake & (1 << id) == 0 && self.policy.wake(id));
        self.awake |= 1 << id;
    }

    fn begin(&mut self) -> *mut Frame {
        let entries: [unsafe extern "C" fn(); 3] = if self.epoch == 0 {
            [mlq_low, mlq_high_a, mlq_high_b]
        } else {
            [mlfq_interactive, mlfq_yielder, mlfq_cpu]
        };
        for (id, entry) in entries.into_iter().enumerate() {
            EXPIRIES[id].store(0, Ordering::Release);
            unsafe {
                let f = self.top(id).sub(size_of::<Frame>()).cast::<Frame>();
                (*f).elr = entry as usize as u64;
                (*f).spsr = 0x3c5;
                (*f).gpr[19] = self.cookie(id);
                (*f).gpr[23] = id as u64;
                self.frames[id] = f;
            }
        }
        for id in if self.epoch == 0 {
            [0, 1, 2]
        } else {
            [1, 0, 2]
        } {
            self.wake(id)
        }
        let next = self.select().unwrap_or_else(|| fail());
        self.start_clock();
        next
    }

    fn select(&mut self) -> Option<*mut Frame> {
        self.current = self.policy.dispatch();
        let id = self.current?;
        require(self.awake & (1 << id) != 0);
        let frame = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        require(!frame.is_null());
        Some(frame)
    }

    fn start_clock(&mut self) {
        let grant = self
            .policy
            .remaining(self.current.unwrap_or_else(|| fail()))
            .unwrap_or_else(|| fail());
        self.start = counter();
        self.deadline = self.start + self.tick * grant;
        ONE_TICK.store(self.start + self.tick, Ordering::Release);
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb", in(reg) self.deadline,
                in(reg) 1u64, options(nostack, preserves_flags));
        }
    }

    fn snapshot(&self) -> ([u8; 3], [u8; 3]) {
        (
            core::array::from_fn(|i| self.policy.priority(i).unwrap_or_else(|| fail())),
            core::array::from_fn(|i| self.policy.remaining(i).unwrap_or_else(|| fail()) as u8),
        )
    }

    fn stop(&mut self, frame: *mut Frame, op: u8, stopped: u64, ppi: u8) -> *mut Frame {
        let id = self.current.take().unwrap_or_else(|| fail());
        require(
            self.frames[id].is_null()
                && frame.addr() == self.top(id).addr() - 16 - size_of::<Frame>(),
        );
        let saved = unsafe { &*frame };
        let cookie = self.cookie(id);
        require(saved.gpr[19] == cookie && saved.gpr[23] == id as u64 && saved.tpidr_el1 == cookie);
        require(unsafe { ptr::read_volatile(self.top(id).sub(16).cast::<u64>()) } == cookie);
        require(saved.spsr & 0x8f == 5);
        let pc = saved.elr;
        check_site(self.epoch, self.step, id, op, pc as usize);
        let work = saved.gpr[20];
        let grant = self.policy.remaining(id).unwrap_or_else(|| fail());
        let elapsed = stopped.checked_sub(self.start).unwrap_or_else(|| fail()) / self.tick;
        let (charge, cause) = match op {
            EXPIRE => {
                require(stopped >= self.deadline && elapsed >= grant);
                EXPIRIES[id].fetch_add(1, Ordering::Release);
                (grant, Stop::Expire)
            }
            YIELD | SLEEP => {
                require(stopped < self.deadline && elapsed < grant);
                (
                    elapsed,
                    if op == YIELD {
                        Stop::Yield
                    } else {
                        Stop::Sleep
                    },
                )
            }
            _ => fail(),
        };
        require(work >= self.last_work[id] && (charge == 0 || work > self.last_work[id]));
        self.last_work[id] = work;
        self.frames[id] = frame;
        require(self.policy.stop(id, charge, cause));
        if op == SLEEP {
            self.awake &= !(1 << id)
        }
        self.total += charge;
        let (rank, left) = self.snapshot();
        let boost = self.epoch == 1 && self.total >= self.next_boost;
        if boost {
            // External release at service tick 12. The interactive thread
            // retained its old accounting while asleep.
            require(self.total == self.next_boost);
            self.wake(INTERACTIVE);
            require(self.policy.boost());
            self.next_boost += BOOST_PERIOD;
        }
        let (next_rank, next_left) = self.snapshot();
        let next = self.select();
        let sample = Sample {
            pc,
            frame: frame.addr() as u64,
            next_pc: next.map_or(0, |f| unsafe { (*f).elr }),
            start: self.start,
            stop: stopped,
            deadline: self.deadline,
            work,
            epoch: self.epoch as u8,
            op,
            caller: id as u8,
            selected: self.current.map_or(NONE, |i| i as u8),
            grant: grant as u8,
            charge: charge as u8,
            total: self.total as u8,
            boost: boost as u8,
            awake: self.awake,
            ppi,
            rank,
            left,
            next_rank,
            next_left,
            reserved: [0; 2],
        };
        unsafe {
            let index = ptr::read_volatile(&raw const multilevel_witness.count) as usize;
            require(index < 17);
            ptr::write_volatile(&raw mut multilevel_witness.samples[index], sample);
            ptr::write_volatile(&raw mut multilevel_witness.count, index as u64 + 1);
        }
        check_decision(self.step, sample);
        self.step += 1;
        if let Some(next) = next {
            self.start_clock();
            return next;
        }
        require(self.awake == 0 && self.step == SCHEDULES[self.epoch].len());
        if self.epoch == 0 {
            uart_write(b"MLQ STRICT FOUR\n");
            *self = Self::new(1, self.tick);
            return self.begin();
        }
        uart_write(b"MLFQ BOOST ORDER\nMULTILEVEL PASS\n");
        park()
    }
}

// This oracle runs after selection and journal append. Each row gives the
// outgoing task, exception kind, measured charge, and next task.
const SCHEDULES: [&[[u8; 4]]; 2] = [
    &[
        [1, EXPIRE, 1, 2],
        [2, EXPIRE, 1, 1],
        [1, EXPIRE, 1, 2],
        [2, EXPIRE, 1, 1],
        [1, SLEEP, 0, 2],
        [2, SLEEP, 0, 0],
        [0, SLEEP, 0, NONE],
    ],
    &[
        [1, YIELD, 1, 0],
        [0, SLEEP, 1, 2],
        [2, EXPIRE, 3, 1],
        [1, YIELD, 1, 1],
        [1, EXPIRE, 1, 2],
        [2, EXPIRE, 4, 1],
        [1, SLEEP, 1, 2],
        [2, EXPIRE, 3, 0],
        [0, SLEEP, 0, 2],
        [2, SLEEP, 0, NONE],
    ],
];

fn check_decision(step: usize, s: Sample) {
    require(SCHEDULES[s.epoch as usize].get(step) == Some(&[s.caller, s.op, s.charge, s.selected]));
    require(s.boost == u8::from(s.epoch == 1 && step == 6));
    let ok = match (s.epoch, step) {
        (0, 0..=3) => s.awake == 7 && s.rank == [0, 2, 2] && s.left == [1; 3],
        (1, 0) => s.rank[1] == 2 && s.left[1] == 2,
        (1, 3) => s.rank[1] == 2 && s.left[1] == 1,
        (1, 4) => s.rank[1] == 1 && s.left[1] == 4 && s.awake == 6,
        (1, 6) => {
            s.total == 12
                && s.awake == 5
                && s.rank == [2, 1, 0]
                && s.left == [2, 3, 6]
                && s.next_rank == [2; 3]
                && s.next_left == [3; 3]
        }
        (1, 7) => {
            s.total == 15
                && s.rank[2] == 1
                && s.left[2] == 4
                && s.next_pc == (&raw const mlfq_interactive_wake).addr() as u64
        }
        _ => true,
    };
    require(ok);
    if s.epoch == 1 && step == 4 {
        uart_write(b"MLFQ YIELDS DEMOTED\n")
    }
}

fn check_site(epoch: usize, step: usize, id: usize, op: u8, pc: usize) {
    if op == EXPIRE {
        let (lo, hi) = match (epoch, id) {
            (0, 1) => (&raw const mlq_high_a_work, &raw const mlq_high_a_end),
            (0, 2) => (&raw const mlq_high_b_work, &raw const mlq_high_b_end),
            (1, 1) => (&raw const mlfq_yielder_work, &raw const mlfq_yielder_end),
            (1, 2) => (&raw const mlfq_cpu_work, &raw const mlfq_cpu_end),
            _ => fail(),
        };
        require((lo.addr()..hi.addr()).contains(&pc));
    } else {
        let site = match (epoch, step) {
            (0, 4) => &raw const mlq_high_a_sleep,
            (0, 5) => &raw const mlq_high_b_sleep,
            (0, 6) => &raw const mlq_low_sleep,
            (1, 0) => &raw const mlfq_after_yield_one,
            (1, 1) => &raw const mlfq_interactive_wake,
            (1, 3) => &raw const mlfq_after_yield_two,
            (1, 6) => &raw const mlfq_yielder_sleep,
            (1, 8) => &raw const mlfq_interactive_end,
            (1, 9) => &raw const mlfq_cpu_sleep,
            _ => fail(),
        };
        require(site.addr() == pc);
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let tick = frequency / 25;
    require(tick != 0);
    unsafe {
        ptr::write_volatile(&raw mut multilevel_witness.tick, tick);
        ACTIVE = Some(Bench::new(0, tick));
        sched_start_frame(with_bench(Bench::begin))
    }
}

extern "C" fn task_call(frame: *mut Frame) -> *mut Frame {
    let stopped = counter();
    disable_timer();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) };
    let op = esr & 0xffff;
    require(esr >> 26 == 0x15 && (1..=2).contains(&op));
    with_bench(|b| b.stop(frame, op as u8, stopped, NONE))
}

extern "C" fn timer_irq(frame: *mut Frame) -> *mut Frame {
    let stopped = counter();
    let iar = gic_read(GICC + 0x0c);
    let control: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) control, options(nostack, preserves_flags)) };
    let ppi = iar & 0x3ff;
    require(ppi == TIMER_IRQ && control & 7 == 5);
    disable_timer();
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|b| b.stop(frame, EXPIRE, stopped, ppi as u8))
}
