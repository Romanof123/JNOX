//! IRQ-retired one-slot grants. Workers publish their actual ring reductions;
//! the manifest supplies job demands and never supplies a dispatch order.
use super::{
    amc_policy::{AmcRuntime, Dispatch, TaskSpec},
    context::*,
    domain_policy::{CriticalityMode, ExecutionBudget},
    park, uart_write,
};
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Acquire, Release},
    },
};

global_asm!(".set JNOX_STAMP_X9,1",include_str!("context.S"),
    frame_size=const size_of::<Frame>(),elr=const offset_of!(Frame,elr),
    fpcr=const offset_of!(Frame,fpcr),tpidr_el1=const offset_of!(Frame,tpidr_el1),
    tpidr_el0=const offset_of!(Frame,tpidr_el0),simd=const offset_of!(Frame,simd),
    irq=sym irq,svc=sym unexpected,fail=sym unexpected);

const RING: [u64; 8] = [2, 5, 1, 6, 3, 7, 4, 8];
const JOBS: usize = 6;
const SLOTS: usize = 6;
const CAP: usize = 64;
const EMPTY: usize = usize::MAX;
static mut STACKS: [Stack; JOBS] = [const { Stack([0; STACK_BYTES]) }; JOBS];
static mut STATE: Option<Bench> = None;
static SLOT: AtomicUsize = AtomicUsize::new(EMPTY);
#[derive(Copy, Clone, Default)]
struct Sample {
    tick: u64,
    n: u64,
    sum: u64,
}
#[derive(Copy, Clone, Default)]
struct Work {
    first: Sample,
    last: Sample,
}
static mut WORK: [Work; SLOTS] = [Work {
    first: Sample {
        tick: 0,
        n: 0,
        sum: 0,
    },
    last: Sample {
        tick: 0,
        n: 0,
        sum: 0,
    },
}; SLOTS];

#[inline(always)]
fn tick() -> u64 {
    let t;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)t,options(nostack,preserves_flags)) };
    t
}
#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn ordinary_amc_worker(id: usize, generation: usize) -> ! {
    let mut n = 0u64;
    let mut sum = 0u64;
    loop {
        let slot = SLOT.load(Acquire);
        let at = (id.wrapping_add(generation).wrapping_add(n as usize)) & 7;
        // Keep the input read observable. Only this instance owns n and sum.
        sum = sum.wrapping_add(unsafe { ptr::read_volatile(RING.as_ptr().add(at)) });
        n = n.wrapping_add(1);
        let mask: u64;
        unsafe {
            asm!("mrs {},daif","msr daifset,#2",out(reg)mask,options(nostack,preserves_flags))
        };
        if slot < SLOTS && SLOT.load(Acquire) == slot {
            let sample = Sample {
                tick: tick(),
                n,
                sum,
            };
            // The one-CPU IRQ cannot see a torn three-word transaction.
            unsafe {
                let record = (&raw mut WORK).cast::<Work>().add(slot);
                if (*record).first.n == 0 {
                    (*record).first = sample;
                }
                (*record).last = sample;
            }
        }
        unsafe { asm!("msr daif,{}",in(reg)mask,options(nostack,preserves_flags)) };
    }
}
fn num(mut n: u64) {
    let mut buf = [0; 20];
    let mut i = 20;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&buf[i..]);
}
fn line(tag: &[u8], words: &[u64]) {
    uart_write(b"AMR ");
    uart_write(tag);
    for &n in words {
        uart_write(b" ");
        num(n)
    }
    uart_write(b"\n");
}
fn fail(reason: u64) -> ! {
    unsafe { asm!("msr daifset,#15", options(nostack)) };
    // Retain already committed raw observations, including the first failed
    // boundary, without borrowing a scheduler transaction still on the stack.
    line(b"PARTIAL", &[reason]);
    dump_rows();
    line(b"FAIL", &[reason]);
    park()
}
extern "C" fn unexpected() -> ! {
    fail(1)
}
fn check(ok: bool, reason: u64) {
    if !ok {
        fail(reason)
    }
}
#[derive(Copy, Clone)]
struct Row {
    tag: &'static [u8],
    len: usize,
    words: [u64; 12],
}
static mut ROWS: [Row; CAP] = [Row {
    tag: b"",
    len: 0,
    words: [0; 12],
}; CAP];
static mut COUNT: usize = 0;
fn record(tag: &'static [u8], words: &[u64]) {
    unsafe {
        check(COUNT < CAP && words.len() <= 12, 2);
        let row = &mut *(&raw mut ROWS).cast::<Row>().add(COUNT);
        row.tag = tag;
        row.len = words.len();
        row.words[..words.len()].copy_from_slice(words);
        COUNT += 1;
    }
}
const SPEC: [TaskSpec; 4] = [
    TaskSpec {
        budget: ExecutionBudget::Low(2),
        priority: 1,
        relative_deadline: 3,
    },
    TaskSpec {
        budget: ExecutionBudget::High { lo: 1, hi: 3 },
        priority: 4,
        relative_deadline: 4,
    },
    TaskSpec {
        budget: ExecutionBudget::Low(2),
        priority: 2,
        relative_deadline: 3,
    },
    TaskSpec {
        budget: ExecutionBudget::High { lo: 1, hi: 1 },
        priority: 3,
        relative_deadline: 5,
    },
];
fn dump_rows() {
    unsafe {
        for i in 0..COUNT {
            let row = &*(&raw const ROWS).cast::<Row>().add(i);
            line(row.tag, &row.words[..row.len]);
        }
    }
}
#[derive(Copy, Clone, Default)]
struct Instance {
    id: usize,
    generation: usize,
    frame: usize,
    last: Sample,
}
struct Bench {
    policy: AmcRuntime<4>,
    case: u64,
    freq: u64,
    origin: u64,
    quantum: u64,
    instances: [Instance; JOBS],
    released: usize,
    next_gen: [usize; 4],
    current: [usize; 4],
    owner: usize,
}
fn bounds(instance: usize) -> (usize, usize) {
    let lo = (&raw mut STACKS).cast::<Stack>().wrapping_add(instance) as usize;
    (lo, lo + STACK_BYTES)
}
impl Bench {
    fn boundary(&self, n: u64) -> u64 {
        self.quantum
            .checked_mul(n)
            .and_then(|t| self.origin.checked_add(t))
            .unwrap_or_else(|| fail(3))
    }
    fn release(&mut self, id: usize, demand: u64) {
        check(self.released < JOBS && self.policy.release(id, demand), 4);
        let i = self.released;
        self.released += 1;
        let g = self.next_gen[id];
        self.next_gen[id] += 1;
        let (lo, hi) = bounds(i);
        let f = (hi - size_of::<Frame>()) as *mut Frame;
        let entry = ordinary_amc_worker as *const () as usize as u64;
        unsafe {
            f.write_bytes(0, 1);
            (*f).gpr[0] = id as u64;
            (*f).gpr[1] = g as u64;
            (*f).elr = entry;
            (*f).spsr = 0x345;
        }
        self.instances[i] = Instance {
            id,
            generation: g,
            frame: f.addr(),
            last: Sample::default(),
        };
        self.current[id] = i;
        let t = self.policy.now();
        record(
            b"RELEASE",
            &[
                t,
                id as u64,
                g as u64,
                demand,
                t + SPEC[id].relative_deadline,
                lo as u64,
                hi as u64,
                entry,
            ],
        );
    }
    fn dispatch(&mut self) -> *mut Frame {
        let t = self.policy.now();
        if t == 1 {
            let demand = match self.case {
                0 => [1, 1, 0, 0],
                1 => [2, 3, 2, 1],
                _ => [2, 4, 2, 1],
            };
            for (id, d) in demand.into_iter().enumerate() {
                if d != 0 {
                    self.release(id, d)
                }
            }
        }
        if t == 2 && self.case != 0 {
            let rem = [0, 1, 2, 3].map(|id| self.policy.remaining(id).unwrap_or(0));
            record(b"CARRY", &[t, rem[0], rem[1], rem[2], rem[3]]);
        }
        if t == 5 && self.case != 0 {
            let refused = self.policy.reset(false);
            let enabled = self.policy.reset(true);
            record(b"RESET", &[t, refused.into(), enabled.into()]);
            check(!refused && enabled, 5);
            self.release(0, 1);
        }
        let mut decision = self.policy.start_slot().unwrap_or_else(|| fail(6));
        if let Dispatch::Exhausted { task } = decision {
            record(
                b"FAULT",
                &[
                    t,
                    task as u64,
                    self.instances[self.current[task]].generation as u64,
                ],
            );
            decision = self.policy.start_slot().unwrap_or_else(|| fail(7));
        }
        let commit = tick();
        let Dispatch::Run { task, discarded } = decision else {
            fail(8)
        };
        let i = self.current[task];
        let job = self.instances[i];
        if discarded != 0 {
            let new_lo = self.policy.release(0, 1);
            let reset = self.policy.reset(true);
            record(b"DENY", &[t, new_lo.into(), reset.into()]);
            check(!new_lo && !reset, 9);
        }
        let compare = self.boundary(t + 1);
        let readback: u64;
        unsafe {
            asm!("msr cntp_cval_el0,{}","msr cntp_ctl_el0,{}","isb","mrs {},cntp_cval_el0",
            in(reg)compare,in(reg)1u64,out(reg)readback,options(nostack,preserves_flags))
        };
        let f = job.frame as *mut Frame;
        let fr = unsafe { &*f };
        let hi = self.policy.mode() == CriticalityMode::High;
        record(
            b"GRANT",
            &[
                t,
                task as u64,
                job.generation as u64,
                hi.into(),
                discarded as u64,
                commit,
                job.frame as u64,
                fr.elr,
                (job.frame + size_of::<Frame>()) as u64,
                compare,
                readback,
                fr.gpr[19],
            ],
        );
        check(readback == compare && tick() < compare, 10);
        self.owner = i;
        SLOT.store(t as usize, Release);
        f
    }
    fn retire(&mut self, f: *mut Frame, iar: u32, ctl: u64, compare: u64) -> *mut Frame {
        let t = self.policy.now();
        let i = self.owner;
        let j = &mut self.instances[i];
        let (lo, hi) = bounds(i);
        check(
            f.addr() % 16 == 0 && (lo..=hi - size_of::<Frame>()).contains(&f.addr()),
            11,
        );
        let fr = unsafe { &*f };
        let entry = fr.reserved;
        record(
            b"IRQ",
            &[
                t,
                j.id as u64,
                j.generation as u64,
                iar as u64,
                compare,
                ctl,
                entry,
                f.addr() as u64,
                fr.elr,
                (f.addr() + size_of::<Frame>()) as u64,
                fr.gpr[19],
            ],
        );
        let w = unsafe { WORK[t as usize] };
        record(
            b"WORK",
            &[
                t,
                j.id as u64,
                j.generation as u64,
                w.first.tick,
                w.first.n,
                w.first.sum,
                w.last.tick,
                w.last.n,
                w.last.sum,
            ],
        );
        check(w.first.n > j.last.n && w.last.n >= w.first.n, 12);
        j.frame = f.addr();
        j.last = w.last;
        let end = self.policy.finish_slot().unwrap_or_else(|| fail(13));
        let now = tick();
        record(
            b"RETIRE",
            &[
                end.end,
                end.task as u64,
                j.generation as u64,
                end.used,
                end.remaining,
                end.deadline,
                now,
            ],
        );
        check(
            !end.deadline_missed()
                && compare == self.boundary(end.end)
                && entry >= compare
                && now < self.boundary(end.end + 1),
            14,
        );
        if end.end == if self.case == 0 { 3 } else { 6 } {
            self.export()
        }
        self.dispatch()
    }
    fn export(&self) -> ! {
        line(
            b"BEGIN",
            &[1, self.case, self.freq, self.origin, self.quantum],
        );
        dump_rows();
        for j in &self.instances[..self.released] {
            line(
                b"FINAL",
                &[
                    j.id as u64,
                    j.generation as u64,
                    j.last.n,
                    j.last.sum,
                    j.last.tick,
                ],
            );
        }
        line(b"END", &[]);
        park()
    }
}
pub(super) fn run() -> ! {
    init_timer_irq();
    let freq: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)freq,options(nostack,preserves_flags)) };
    check(freq >= 20, 15);
    let case = match env!("JNOX_AMC_APP") {
        "exact-lo" => 0,
        "exact-hi" => 1,
        "over-hi" => 2,
        _ => fail(16),
    };
    unsafe {
        *(&raw mut STATE) = Some(Bench {
            policy: AmcRuntime::new(SPEC).unwrap_or_else(|| fail(17)),
            case,
            freq,
            origin: 0,
            quantum: freq / 20,
            instances: [Instance::default(); JOBS],
            released: 0,
            next_gen: [0; 4],
            current: [EMPTY; 4],
            owner: EMPTY,
        });
        let s = (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail(18));
        s.release(0, 1);
        s.origin = tick();
        let f = s.dispatch();
        sched_start_frame(f)
    }
}
extern "C" fn irq(f: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0xc);
    let ctl: u64;
    let compare: u64;
    unsafe {
        asm!("mrs {},cntp_ctl_el0","mrs {},cntp_cval_el0",out(reg)ctl,out(reg)compare,options(nostack,preserves_flags))
    };
    check(iar & 1023 == TIMER_IRQ && ctl & 7 == 5, 19);
    unsafe {
        asm!("msr cntp_ctl_el0,{}","isb","dsb sy",in(reg)2u64,options(nostack,preserves_flags))
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    unsafe {
        (*(&raw mut STATE))
            .as_mut()
            .unwrap_or_else(|| fail(20))
            .retire(f, iar, ctl, compare)
    }
}
