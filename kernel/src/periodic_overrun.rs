//! Finite periodic release calendar, including queued generations and late completion.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::context::*;
use super::dispatch_policy::{DispatchJob, DispatchPolicy};
use super::ready_queue::ReadyQueue;
use super::{park, uart_write};

const IDLE: usize = 6;
const NROWS: usize = 256;
const PERIOD: [u64; 2] = [10, 12];
const DEADLINE: [u64; 2] = [9, 4];
const EMPTY: u8 = 0;
const READY: u8 = 1;
const SUBMITTED: u8 = 2;
const DONE: u8 = 3;

unsafe extern "C" {
    fn overrun_idle();
    fn overrun_long();
    fn overrun_short();
    static overrun_idle_loop: u8;
    static overrun_work: u8;
    static overrun_work_end: u8;
    static overrun_short_return: u8;
    static overrun_long_return: u8;
}

global_asm!(include_str!("context.S"), include_str!("periodic_overrun.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym irq, svc = sym complete, fail = sym fail);

#[repr(C)]
struct Witness {
    origin: u64,
    quantum: u64,
    frequency: u64,
    count: u64,
    rows: [[u64; 9]; NROWS],
}

#[unsafe(no_mangle)]
static mut periodic_overrun_witness: Witness = unsafe { core::mem::zeroed() };
static mut STACKS: [Stack; 7] = [const { Stack([0; STACK_BYTES]) }; 7];
static mut BENCH: Option<Bench> = None;

#[derive(Clone, Copy)]
struct Job {
    state: u8,
    missed: bool,
    release: u64,
    frame: *mut Frame,
}

impl Job {
    const EMPTY: Self = Self {
        state: EMPTY,
        missed: false,
        release: 0,
        frame: ptr::null_mut(),
    };
}

struct Bench {
    jobs: [Job; 6],
    current: usize,
    idle: *mut Frame,
    tick: u64,
    origin: u64,
    quantum: u64,
    rows: usize,
}

fn require(ok: bool) {
    if !ok {
        fail()
    }
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack)) };
    uart_write(b"PERIODIC OVERRUN FAIL\n");
    park()
}

fn counter() -> u64 {
    let v: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) v, options(nostack)) };
    v
}

fn arm(cval: u64) {
    unsafe {
        asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                  in(reg) cval, in(reg) 1u64, options(nostack))
    };
}

fn stack_frame(slot: usize) -> *mut Frame {
    require(slot <= IDLE);
    let top = (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((slot + 1) * STACK_BYTES);
    unsafe { top.sub(size_of::<Frame>()).cast() }
}

fn fresh(slot: usize) -> *mut Frame {
    let f = stack_frame(slot);
    let entry = match slot {
        IDLE => overrun_idle,
        2 => overrun_long,
        _ => overrun_short,
    };
    unsafe {
        f.write_bytes(0, 1);
        (*f).elr = entry as *const () as u64;
        (*f).spsr = 0x3c5;
        (*f).gpr[19] = slot as u64;
    }
    f
}

fn bench() -> &'static mut Bench {
    // Sole CPU, called only inside one masked exception handler at a time.
    unsafe { (*(&raw mut BENCH)).as_mut().unwrap_or_else(|| fail()) }
}

impl Bench {
    fn compare(&self, tick: u64) -> u64 {
        self.origin + tick * self.quantum
    }

    fn record(&mut self, event: u64, slot: usize, sample: u64, frame: *mut Frame) {
        require(self.rows < NROWS);
        let (release, deadline) = if slot == IDLE {
            (u64::MAX, u64::MAX)
        } else {
            let r = self.jobs[slot].release;
            (r, r + DEADLINE[slot % 2])
        };
        let (pc, progress) = if frame.is_null() {
            (0, 0)
        } else {
            unsafe { ((*frame).elr, (*frame).gpr[20]) }
        };
        let row = [
            self.tick,
            event,
            slot as u64,
            release,
            deadline,
            sample,
            pc,
            progress,
            frame.addr() as u64,
        ];
        unsafe {
            ptr::write_volatile(&raw mut periodic_overrun_witness.rows[self.rows], row);
            self.rows += 1;
            ptr::write_volatile(&raw mut periodic_overrun_witness.count, self.rows as u64);
        }
    }

    fn select(&mut self) -> *mut Frame {
        let mut queue = ReadyQueue::<6>::new();
        // Only the oldest unfinished generation of either task is eligible.
        for task in 0..2 {
            if let Some(slot) = (task..6)
                .step_by(2)
                .find(|&s| matches!(self.jobs[s].state, READY | SUBMITTED))
            {
                if self.jobs[slot].state == READY {
                    require(queue.push_back(slot));
                }
            }
        }
        let policy = if self.tick < 26 {
            DispatchPolicy::RateMonotonic
        } else {
            DispatchPolicy::DeadlineMonotonic
        };
        let slot = policy
            .pop_ready(&mut queue, self.tick, |s| {
                Some(DispatchJob {
                    release: self.jobs[s].release,
                    period: PERIOD[s % 2],
                    relative_deadline: DEADLINE[s % 2],
                    ..DispatchJob::fixed_priority(0)
                })
            })
            .unwrap_or(IDLE);
        self.current = slot;
        let frame = if slot == IDLE {
            self.idle
        } else {
            self.jobs[slot].frame
        };
        require(frame == stack_frame(slot));
        self.record(6, slot, counter(), frame);
        frame
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack)) };
    let quantum = frequency / 20;
    require(quantum != 0);
    let origin = counter() + quantum;
    let idle = fresh(IDLE);
    unsafe {
        periodic_overrun_witness.origin = origin;
        periodic_overrun_witness.quantum = quantum;
        periodic_overrun_witness.frequency = frequency;
        BENCH = Some(Bench {
            jobs: [Job::EMPTY; 6],
            current: IDLE,
            idle,
            tick: u64::MAX,
            origin,
            quantum,
            rows: 0,
        });
        arm(origin);
        sched_start_frame(idle)
    }
}

extern "C" fn complete(frame: *mut Frame) -> *mut Frame {
    let sample = counter();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack)) };
    require(esr >> 26 == 0x15 && esr & 0xffff == 1);
    let b = bench();
    let slot = b.current;
    require(slot < IDLE && frame == stack_frame(slot) && b.jobs[slot].state == READY);
    require(sample < b.compare(b.tick + 1));
    let saved = unsafe { &*frame };
    require(saved.gpr[19] == slot as u64);
    let due = b.jobs[slot].release + DEADLINE[slot % 2];
    if slot == 2 {
        require(
            saved.elr == (&raw const overrun_long_return).addr() as u64
                && saved.gpr[20] > 1
                && b.tick % 26 == 21
                && sample > b.compare(due),
        );
    } else {
        require(
            saved.elr == (&raw const overrun_short_return).addr() as u64 && saved.gpr[20] == 128,
        );
    }
    b.record(4, slot, sample, frame);
    if slot == 0 {
        require(sample < b.compare(due));
        b.jobs[slot].state = SUBMITTED;
    } else {
        require((sample > b.compare(due)) == b.jobs[slot].missed);
        b.jobs[slot].state = DONE;
        b.record(5, slot, sample, frame);
    }
    b.select()
}

extern "C" fn irq(frame: *mut Frame) -> *mut Frame {
    let sample = counter();
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack)) };
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    unsafe { asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy", in(reg) 2u64, options(nostack)) };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack)) };
    let b = bench();
    b.tick = b.tick.wrapping_add(1);
    let local = b.tick % 26;
    require((b.compare(b.tick)..b.compare(b.tick + 1)).contains(&sample));
    let slot = b.current;
    require(frame == stack_frame(slot));
    let saved = unsafe { &mut *frame };
    require(saved.spsr & 0x8f == 5 && saved.gpr[19] == slot as u64);
    if slot == IDLE {
        let start = (&raw const overrun_idle_loop).addr() as u64;
        require((start..start + 8).contains(&saved.elr));
        b.idle = frame;
    } else {
        require(
            slot == 2
                && ((&raw const overrun_work).addr()..(&raw const overrun_work_end).addr())
                    .contains(&(saved.elr as usize)),
        );
        b.jobs[slot].frame = frame;
        if local == 21 {
            saved.gpr[21] = 1;
        }
    }
    b.record(1, slot, sample, frame);
    if local == 0 {
        b.jobs = [Job::EMPTY; 6];
    }
    for s in 0..6 {
        let job = b.jobs[s];
        if job.state == EMPTY || job.state == DONE {
            continue;
        }
        let due = job.release + DEADLINE[s % 2];
        if job.state == SUBMITTED && due == b.tick {
            b.jobs[s].state = DONE;
            b.record(5, s, sample, ptr::null_mut());
        } else if due <= b.tick && !job.missed {
            b.jobs[s].missed = true;
            b.record(3, s, sample, ptr::null_mut());
        }
    }
    for task in 0..2 {
        if local % PERIOD[task] == 0 {
            let s = 2 * (local / PERIOD[task]) as usize + task;
            require(s < 6 && b.jobs[s].state == EMPTY);
            b.jobs[s] = Job {
                state: READY,
                missed: false,
                release: b.tick,
                frame: fresh(s),
            };
            b.record(2, s, sample, ptr::null_mut());
        }
    }
    if local == 25 {
        require(b.jobs.iter().all(|j| j.state == DONE));
        require(b.jobs.iter().filter(|j| j.missed).count() == if b.tick == 25 { 2 } else { 1 });
        if b.tick == 25 {
            uart_write(b"OVERRUN RM DONE\n");
        } else {
            uart_write(b"OVERRUN DM DONE\nPERIODIC OVERRUN PASS\n");
            park();
        }
    }
    let selected = b.select();
    let next = b.compare(b.tick + 1);
    require(counter() < next);
    arm(next);
    selected
}
