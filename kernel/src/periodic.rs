//! Two six-tick periodic workloads driven by the EL1 physical timer.
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::context::*;
use super::dispatch_policy::{DispatchJob, DispatchPolicy};
use super::ready_queue::ReadyQueue;
use super::{park, uart_write};

const IDLE: usize = 2;
const PERIOD: [u64; 2] = [2, 3];
const DEADLINE: [u64; 2] = [2, 1];

unsafe extern "C" {
    fn periodic_controller();
    fn periodic_a();
    fn periodic_b();
    static periodic_loop: u8;
    static periodic_loop_end: u8;
}

#[cfg(feature = "periodic-preempt-bench")]
unsafe extern "C" {
    fn periodic_running_a();
    static periodic_work: u8;
    static periodic_work_end: u8;
}

global_asm!(
    include_str!("context.S"), include_str!("periodic.S"),
    preempt = const cfg!(feature = "periodic-preempt-bench") as usize,
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    marker = sym marker, irq = sym irq, svc = sym complete, fail = sym fail,
);

static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut BENCH: Option<Bench> = None;

struct Bench {
    queue: ReadyQueue<2>,
    jobs: [Option<DispatchJob>; 2],
    current: usize,
    controller: *mut Frame,
    progress: u64,
    completed: usize,
    marked: bool,
    interrupts: u8,
    compare: u64,
    quantum: u64,
    #[cfg(feature = "periodic-preempt-bench")]
    running: RunningA,
}

#[cfg(feature = "periodic-preempt-bench")]
#[derive(Default)]
struct RunningA {
    started: bool,
    frame: *mut Frame,
    pc: u64,
    expected_progress: u64,
}

extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) };
    uart_write(b"PERIODIC FAIL\n");
    park()
}

fn require(condition: bool) {
    if !condition {
        fail()
    }
}

fn counter() -> u64 {
    let value: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack, preserves_flags)) };
    value
}

fn arm(compare: u64) {
    unsafe {
        asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
             in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags))
    };
}

fn stack_top(id: usize) -> *mut u8 {
    require(id <= IDLE);
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add((id + 1) * STACK_BYTES)
}

// Workers enter with IRQ masked for their short marker and SVC. Completion
// checks the hardware counter so a masked, expired release still fails.
fn fresh(id: usize) -> *mut Frame {
    let entries = [periodic_a, periodic_b, periodic_controller];
    unsafe {
        let frame = stack_top(id).sub(size_of::<Frame>()).cast::<Frame>();
        frame.write_bytes(0, 1);
        (*frame).elr = entries[id] as usize as u64;
        (*frame).spsr = 0x3c5;
        frame
    }
}

fn with_bench<R>(f: impl FnOnce(&mut Bench) -> R) -> R {
    // All callers run on the sole CPU with IRQ masked. No borrow survives eret.
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    require(daif & 0x80 != 0);
    unsafe { f((*(&raw mut BENCH)).as_mut().unwrap_or_else(|| fail())) }
}

impl Bench {
    fn tick(&self) -> u8 {
        #[cfg(feature = "periodic-preempt-bench")]
        if self.interrupts > 12 {
            return (self.interrupts - 13) % 3;
        }
        (self.interrupts - 1) % 6
    }

    fn dispatch(&mut self) -> *mut Frame {
        let policy = if self.interrupts <= 6 || (13..=15).contains(&self.interrupts) {
            DispatchPolicy::RateMonotonic
        } else {
            DispatchPolicy::DeadlineMonotonic
        };
        let tick = u64::from(self.tick());
        let jobs = &self.jobs;
        if let Some(id) = policy.pop_ready(&mut self.queue, tick, |id| jobs[id]) {
            self.current = id;
            #[cfg(feature = "periodic-preempt-bench")]
            if self.interrupts > 12 && id == 0 {
                if self.tick() == 0 {
                    let frame = fresh(id);
                    unsafe { (*frame).elr = periodic_running_a as *const () as u64 };
                    return frame;
                }
                let frame = core::mem::replace(&mut self.running.frame, ptr::null_mut());
                require(!frame.is_null() && unsafe { (*frame).elr } == self.running.pc);
                return frame;
            }
            fresh(id)
        } else {
            require(self.jobs.iter().all(Option::is_none));
            self.current = IDLE;
            let frame = core::mem::replace(&mut self.controller, ptr::null_mut());
            require(!frame.is_null());
            frame
        }
    }

    fn release(&mut self, id: usize, tick: u64) {
        require(self.jobs[id].is_none() && self.queue.push_back(id));
        self.jobs[id] = Some(DispatchJob {
            release: tick,
            period: PERIOD[id],
            relative_deadline: DEADLINE[id],
            ..DispatchJob::fixed_priority(0)
        });
    }

    #[cfg(feature = "periodic-preempt-bench")]
    fn record_worker(&mut self, frame: *mut Frame) {
        require(self.current == 0 && self.running.started && self.running.frame.is_null());
        let sp = unsafe { stack_top(0).sub(16).cast::<u64>() };
        require(frame.addr() + size_of::<Frame>() == sp.addr());
        let saved = unsafe { &*frame };
        let start = (&raw const periodic_work).addr();
        require((start..(&raw const periodic_work_end).addr()).contains(&(saved.elr as usize)));
        require(saved.spsr & 0x8f == 5 && saved.gpr[19] == 0xa19 && saved.gpr[20] > 0);
        unsafe {
            require(sp.read_volatile() == 0xa19 && sp.add(1).read_volatile() == 0);
            sp.add(1).write_volatile(1);
        }
        // At ADD we still increment once. At LDR the new flag is visible
        // immediately. At CBZ the old zero forces one more iteration.
        let offset = saved.elr as usize - start;
        require(matches!(offset, 0 | 4 | 8) && (offset != 8 || saved.gpr[9] == 0));
        self.running.expected_progress = saved.gpr[20] + u64::from(offset != 4);
        self.running.pc = saved.elr;
        self.running.frame = frame;
        require(self.queue.push_back(0));
    }

    fn record_controller(&mut self, frame: *mut Frame) {
        require(self.current == IDLE && self.controller.is_null());
        require(frame.addr() == stack_top(IDLE).addr() - 16 - size_of::<Frame>());
        let saved = unsafe { &*frame };
        require(
            ((&raw const periodic_loop).addr()..(&raw const periodic_loop_end).addr())
                .contains(&(saved.elr as usize)),
        );
        require(saved.spsr & 0x8f == 5 && saved.gpr[19] == 0xc19 && saved.gpr[20] > self.progress);
        let witness = unsafe { ptr::read_volatile(stack_top(IDLE).sub(16).cast::<u64>()) };
        require(witness == saved.gpr[19]);
        self.progress = saved.gpr[20];
        self.controller = frame;
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    let quantum = frequency / 20;
    require(quantum > 0);
    let frame = fresh(IDLE);
    let compare = counter().checked_add(quantum).unwrap_or_else(|| fail());
    unsafe {
        BENCH = Some(Bench {
            queue: ReadyQueue::new(),
            jobs: [None; 2],
            current: IDLE,
            controller: ptr::null_mut(),
            progress: 0,
            completed: 0,
            marked: false,
            interrupts: 0,
            compare,
            quantum,
            #[cfg(feature = "periodic-preempt-bench")]
            running: RunningA::default(),
        });
        arm(compare);
        sched_start_frame(frame)
    }
}

extern "C" fn marker(id: usize, sp: usize, _progress: u64) {
    with_bench(|b| {
        #[cfg(feature = "periodic-preempt-bench")]
        if b.interrupts > 12 {
            let rm = b.interrupts <= 15;
            let base = if rm { 10 } else { 12 };
            require(sp == stack_top(b.current).addr() && !b.marked && counter() < b.compare);
            let line: &[u8] = match id {
                2 => {
                    require(b.current == 0 && b.tick() == 0 && !b.running.started);
                    b.running.started = true;
                    if rm { b"RM A START\n" } else { b"DM A START\n" }
                }
                3 => {
                    require(b.current == 0 && b.tick() == 1 && b.running.pc != 0);
                    require(b.running.frame.is_null() && _progress == b.running.expected_progress);
                    require(b.completed == base + usize::from(!rm));
                    b.marked = true;
                    if rm {
                        b"RM A CONTINUE\n"
                    } else {
                        b"DM A CONTINUE\n"
                    }
                }
                1 => {
                    require(b.current == 1 && b.tick() == 1 && b.running.started);
                    require(b.completed == base + usize::from(rm));
                    b.marked = true;
                    if rm { b"RM B RUN\n" } else { b"DM B RUN\n" }
                }
                _ => fail(),
            };
            uart_write(line);
            return;
        }
        require(id == b.current && sp == stack_top(id).addr() && !b.marked);
        let mut line = *b"RM 0 A\n";
        if b.interrupts > 6 {
            line[..2].copy_from_slice(b"DM");
        }
        line[3] += b.tick();
        line[5] += id as u8;
        uart_write(&line);
        b.marked = true;
    });
}

extern "C" fn complete(frame: *mut Frame) -> *mut Frame {
    let syndrome: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) syndrome, options(nostack, preserves_flags)) };
    require(syndrome >> 26 == 0x15 && syndrome & 0xffff == 1);
    with_bench(|b| {
        let id = b.current;
        require(id < IDLE && b.marked && counter() < b.compare);
        require(frame.addr() == stack_top(id).addr() - size_of::<Frame>());
        require(b.jobs[id].take().is_some());
        b.marked = false;
        b.completed += 1;
        b.dispatch()
    })
}

extern "C" fn irq(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) };
    require(iar & 0x3ff == TIMER_IRQ && ctl & 7 == 5);
    unsafe {
        asm!("msr cntp_ctl_el0, {}", "isb", "dsb sy", in(reg) 2u64, options(nostack, preserves_flags))
    };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    with_bench(|b| {
        let next = b.compare.checked_add(b.quantum).unwrap_or_else(|| fail());
        require((b.compare..next).contains(&counter()));
        b.interrupts += 1;
        #[cfg(feature = "periodic-preempt-bench")]
        if b.interrupts > 12 && b.tick() == 1 {
            b.record_worker(frame);
        } else {
            b.record_controller(frame);
        }
        #[cfg(not(feature = "periodic-preempt-bench"))]
        b.record_controller(frame);
        let tick = u64::from(b.tick());
        if tick == 5 {
            require(b.completed == 5 * usize::from(b.interrupts / 6));
            #[cfg(not(feature = "periodic-preempt-bench"))]
            if b.interrupts == 12 {
                uart_write(b"PERIODIC PASS\n");
                park();
            }
        }
        if b.interrupts <= 12 {
            for id in [1, 0] {
                if tick % PERIOD[id] == 0 {
                    b.release(id, tick);
                }
            }
        }
        #[cfg(feature = "periodic-preempt-bench")]
        if b.interrupts > 12 {
            match tick {
                0 => {
                    b.running = RunningA::default();
                    b.release(0, tick);
                }
                1 => b.release(1, tick),
                2 => {
                    require(b.jobs.iter().all(Option::is_none) && b.running.frame.is_null());
                    require(b.completed == if b.interrupts == 15 { 12 } else { 14 });
                    if b.interrupts == 18 {
                        require(counter() < next);
                        uart_write(b"PREEMPT PASS\n");
                        park();
                    }
                }
                _ => fail(),
            }
        }
        b.compare = next;
        let selected = b.dispatch();
        require(counter() < next);
        arm(next);
        selected
    })
}
