//! Single-core trusted Rust batch jobs. Times and estimates use CNTPCT ticks.
use super::{
    batch_policy::{BatchPolicy, BatchScheduler},
    context::*,
    park, uart_write,
};
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
};

pub(crate) const CAPACITY: usize = 4;
pub(crate) type Work = fn(usize, Handle);
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Handle(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    Estimate,
    Capacity,
    Exhausted,
    Stale,
    Deadline,
}

static mut STACKS: [Stack; CAPACITY] = [const { Stack([0; STACK_BYTES]) }; CAPACITY];
static mut IDLE_STACK: Stack = Stack([0; STACK_BYTES]);
static mut STATE: Option<Runtime> = None;

global_asm!(include_str!("context.S"),
    frame_size = const size_of::<Frame>(), elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr), tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0), simd = const offset_of!(Frame, simd),
    irq = sym interrupt, svc = sym request, fail = sym fail);

pub(crate) fn counter() -> u64 {
    let n;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) n, options(nostack, preserves_flags)) };
    n
}

pub(crate) fn frequency() -> u64 {
    let n;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) n, options(nostack, preserves_flags)) };
    n
}

pub(crate) fn submit(work: Work, arg: usize, estimate: u64, delay: u64) -> Result<Handle, Error> {
    let result: u64;
    let status: u64;
    unsafe {
        asm!("svc #1", inlateout("x0") work as usize as u64 => result,
        inlateout("x1") arg as u64 => status, in("x2") estimate, in("x3") delay)
    };
    match status {
        0 => Ok(Handle(result)),
        1 => Err(Error::Estimate),
        2 => Err(Error::Capacity),
        3 => Err(Error::Exhausted),
        4 => Err(Error::Deadline),
        _ => fail(),
    }
}

/// A successful completion transfers control permanently.
pub(crate) fn complete(handle: Handle) -> Result<(), Error> {
    unsafe { asm!("svc #2", in("x0") handle.0) };
    Err(Error::Stale)
}

extern "C" fn invoke(entry: usize, arg: usize, token: u64) -> ! {
    // All entries originate from typed Rust submission on this trusted CPU.
    let work: Work = unsafe { core::mem::transmute(entry) };
    work(arg, Handle(token));
    let _ = complete(Handle(token));
    fail()
}

extern "C" fn fail() -> ! {
    uart_write(b"BATCH RUNTIME FAULT\n");
    park()
}

#[unsafe(export_name = "jnox_batch_idle")]
#[inline(never)]
extern "C" fn idle() -> ! {
    loop {
        // A pending timer wakes WFI while masked; take it on unmask so a
        // release cannot arrive between observing idle state and sleeping.
        unsafe { asm!("msr daifset, #2", "isb", options(nostack)) };
        #[cfg(jnox_batch_trace)]
        {
            let pc: u64;
            let sp: u64;
            unsafe { asm!("adr {}, .", "mov {}, sp", out(reg) pc, out(reg) sp, options(nostack)) };
            super::batch_trace::idle_observation(pc, sp);
        }
        unsafe { asm!("dsb sy", "wfi", "msr daifclr, #2", "isb", options(nostack)) };
    }
}

unsafe fn fresh(stack: *mut Stack, entry: usize) -> *mut Frame {
    let f = unsafe { stack.add(1).cast::<Frame>().sub(1) };
    unsafe {
        ptr::write_bytes(f, 0, 1);
        (*f).elr = entry as u64;
        (*f).spsr = 0x345;
    }
    f
}

#[derive(Clone, Copy)]
struct Job {
    token: u64,
    estimate: u64,
    due: Option<u64>,
    frame: *mut Frame,
}
struct Runtime {
    scheduler: BatchScheduler<CAPACITY>,
    slots: [Option<Job>; CAPACITY],
    current: Option<usize>,
    idle: *mut Frame,
    sequence: u64,
}

impl Runtime {
    fn admit(
        &mut self,
        now: u64,
        entry: usize,
        arg: u64,
        estimate: u64,
        delay: u64,
    ) -> Result<Handle, Error> {
        if estimate == 0 || estimate > u32::MAX as u64 {
            return Err(Error::Estimate);
        }
        let due = now.checked_add(delay).ok_or(Error::Deadline)?;
        let id = self
            .slots
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Capacity)?;
        let token = self.sequence.checked_add(1).ok_or(Error::Exhausted)?;
        // An empty slot has no suspended or executing task.
        let frame = unsafe {
            fresh(
                (&raw mut STACKS).cast::<Stack>().add(id),
                invoke as *const () as usize,
            )
        };
        unsafe {
            (&mut (*frame).gpr)[..3].copy_from_slice(&[entry as u64, arg, token]);
        }
        self.sequence = token;
        self.slots[id] = Some(Job {
            token,
            estimate,
            due: Some(due),
            frame,
        });
        Ok(Handle(token))
    }

    #[cfg(jnox_batch_trace)]
    fn token(&self) -> u64 {
        self.current.map_or(0, |id| self.slots[id].unwrap().token)
    }

    #[cfg(jnox_batch_trace)]
    fn remaining(&self) -> u64 {
        self.current
            .and_then(|id| self.scheduler.remaining(id))
            .unwrap_or(0)
    }

    fn save(&mut self, frame: *mut Frame) {
        if let Some(id) = self.current {
            self.slots[id].as_mut().unwrap().frame = frame;
        } else {
            self.idle = frame;
        }
    }

    fn dispatch(&mut self, now: u64) -> *mut Frame {
        // Service simultaneous or overdue timers in deadline/admission order.
        // Their contractual release time contributes to HRRN waiting age.
        loop {
            let pending = self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(id, slot)| {
                    let job = slot.as_ref()?;
                    Some((job.due?, job.token, id))
                })
                .min();
            match pending {
                Some((due, _, id)) if due <= now => {
                    let job = self.slots[id].as_mut().unwrap();
                    self.scheduler
                        .release_due(now, due, id, job.estimate)
                        .unwrap();
                    job.due = None;
                }
                Some((due, _, _)) => {
                    unsafe {
                        asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
                        in(reg) due, in(reg) 1u64, options(nostack))
                    };
                    break;
                }
                None => {
                    unsafe { asm!("msr cntp_ctl_el0, xzr", "isb", options(nostack)) };
                    break;
                }
            }
        }
        self.current = self.scheduler.decide(now).unwrap();
        if let Some(id) = self.current {
            return self.slots[id].unwrap().frame;
        }
        #[cfg(jnox_batch_trace)]
        if self.slots.iter().all(Option::is_none) {
            super::batch_trace::mark_empty();
        }
        self.idle
    }
}

// Exception handlers run with IRQ masked. No borrow crosses exception return.
fn runtime() -> &'static mut Runtime {
    unsafe { (*(&raw mut STATE)).as_mut().unwrap() }
}

extern "C" fn interrupt(frame: *mut Frame) -> *mut Frame {
    let iar = gic_read(GICC + 0x0c);
    if iar & 1023 == 1023 {
        return frame;
    }
    if iar & 1023 != TIMER_IRQ {
        fail();
    }
    unsafe { asm!("msr cntp_ctl_el0, xzr", "isb", "dsb sy", options(nostack)) };
    gic_write(GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack)) };
    let rt = runtime();
    rt.save(frame);
    let now = counter();
    #[cfg(jnox_batch_trace)]
    let old = rt.token();
    let next = rt.dispatch(now);
    #[cfg(jnox_batch_trace)]
    super::batch_trace::record(1, now, old, rt.token(), frame, next, [0; 5], rt.remaining());
    next
}

extern "C" fn request(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack)) };
    if esr >> 26 != 0x15 {
        fail();
    }
    let now = counter();
    let rt = runtime();
    rt.save(frame);
    #[cfg(jnox_batch_trace)]
    let old = rt.token();
    let regs = unsafe { &mut (*frame).gpr };
    #[cfg(jnox_batch_trace)]
    let args = if esr & 0xffff == 1 {
        [regs[1], regs[2], regs[3]]
    } else {
        [regs[0], 0, 0]
    };
    match esr & 0xffff {
        1 => match rt.admit(now, regs[0] as usize, regs[1], regs[2], regs[3]) {
            Ok(handle) => {
                regs[0] = handle.0;
                regs[1] = 0;
            }
            Err(error) => {
                regs[1] = match error {
                    Error::Estimate => 1,
                    Error::Capacity => 2,
                    Error::Exhausted => 3,
                    Error::Deadline => 4,
                    Error::Stale => fail(),
                }
            }
        },
        2 => {
            let id = rt.current.unwrap();
            if rt.slots[id].unwrap().token == regs[0] {
                rt.scheduler.complete(now, id).unwrap();
                rt.slots[id] = None;
            }
        }
        _ => fail(),
    }
    #[cfg(jnox_batch_trace)]
    let outcome = if esr & 0xffff == 1 {
        [regs[1], if regs[1] == 0 { regs[0] } else { 0 }]
    } else {
        [u64::from(rt.slots[rt.current.unwrap()].is_some()), 0]
    };
    let next = rt.dispatch(now);
    #[cfg(jnox_batch_trace)]
    super::batch_trace::record(
        2 + (esr & 0xffff),
        now,
        old,
        rt.token(),
        frame,
        next,
        [args[0], args[1], args[2], outcome[0], outcome[1]],
        rt.remaining(),
    );
    next
}

pub(crate) fn start(policy: BatchPolicy, work: Work, estimate: u64) -> ! {
    init_timer_irq();
    let now = counter();
    let mut rt = Runtime {
        scheduler: BatchScheduler::new(policy, now),
        slots: [None; CAPACITY],
        idle: unsafe { fresh(&raw mut IDLE_STACK, idle as *const () as usize) },
        current: None,
        sequence: 0,
    };
    rt.admit(now, work as usize, 0, estimate, 0).unwrap();
    unsafe {
        STATE = Some(rt);
        let rt = runtime();
        let next = rt.dispatch(now);
        #[cfg(jnox_batch_trace)]
        super::batch_trace::record(
            0,
            now,
            0,
            rt.token(),
            ptr::null_mut(),
            next,
            [0, estimate, 0, 0, 1],
            rt.remaining(),
        );
        sched_start_frame(next)
    }
}
