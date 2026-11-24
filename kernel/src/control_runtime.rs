use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

use super::{
    context::*,
    control_app as app,
    dispatch_policy::{DispatchJob, DispatchPolicy},
    park,
    ready_queue::ReadyQueue,
    uart_write,
};

#[cfg(jnox_control_cold)]
#[path = "control_cold.rs"]
mod cold;

const IDLE: usize = 2;
const LAST: u64 = 25; // 0..=25: thirteen complete jobs, horizon phase 24
const CAPACITY: usize = 1024;
pub(super) const CONTROL_US: u64 = 250;
pub(super) const SENDER_US: u64 = 750;

global_asm!(".set JNOX_CONTROL, 1"," .if {phase}
.set JNOX_PERMIT,1
.endif", include_str!("context.S"), phase=const cfg!(jnox_permit_phase) as usize, frame_size=const size_of::<Frame>(),
    elr=const offset_of!(Frame, elr), fpcr=const offset_of!(Frame, fpcr),
    tpidr_el1=const offset_of!(Frame, tpidr_el1), tpidr_el0=const offset_of!(Frame,tpidr_el0),
    simd=const offset_of!(Frame,simd), irq=sym irq, svc=sym svc, fail=sym fail);

static mut STACKS: [Stack; 3] = [const { Stack([0; STACK_BYTES]) }; 3];
static mut STATE: Option<Runtime> = None;
static mut TRACE: [[u64; 4]; CAPACITY] = [[0; 4]; CAPACITY];
static mut USED: usize = 0;
static mut LONGEST_MASKED: (u64, u64, u64) = (0, 0, 0);

pub(super) fn now() -> u64 {
    let n;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)n,options(nostack,preserves_flags)) };
    n
}

pub(super) fn log(event: u64, stamp: u64, a: u64, b: u64) {
    // All event producers hold the single CPU IRQ mask.
    unsafe {
        #[cfg(jnox_permit_off)]
        if USED >= CAPACITY - 6 {
            app::coverage_stop(3);
        }
        if USED == CAPACITY {
            fail()
        }
        (&raw mut TRACE)
            .cast::<[u64; 4]>()
            .add(USED)
            .write([event, stamp, a, b]);
        USED += 1;
    }
}

pub(super) fn locked<R>(f: impl FnOnce() -> R) -> R {
    let prior: u64;
    unsafe { asm!("mrs {},daif","msr daifset,#2",out(reg)prior,options(nostack)) };
    let begin = now();
    let r = f();
    let end = now();
    #[cfg(jnox_permit_phase)]
    if let Some(s) = unsafe { (*(&raw const STATE)).as_ref() }
        && s.current < IDLE
    {
        log(94, begin, end, s.current as u64);
    }
    unsafe {
        LONGEST_MASKED = LONGEST_MASKED.max((end - begin, begin, end));
    }
    unsafe { asm!("msr daif,{}",in(reg)prior,options(nostack)) };
    r
}

pub(super) fn spacing_failure() {
    state().timing_fault(8)
}

fn fail() -> ! {
    unsafe { asm!("msr daifset,#2", options(nostack)) };
    app::write_output(false);
    uart_write(b"CONTROL INVALID\n");
    park()
}

fn number(mut n: u64) {
    let mut buf = [b'0'; 20];
    let mut pos = buf.len();
    loop {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&buf[pos..]);
}

#[cfg(jnox_permit_off)]
pub(super) fn containment_detail(why: u64, detected: u64, off: u64, bit: u64) -> ! {
    for field in [why, detected, off, bit] {
        number(field);
        uart_write(b" ")
    }
    uart_write(b"\n");
    dump()
}
fn dump() -> ! {
    unsafe { asm!("msr cntp_ctl_el0,xzr", "isb", options(nostack)) };
    uart_write(b"CONTROL RAW v1\n");
    for i in 0..unsafe { USED } {
        let row = unsafe { (&raw const TRACE).cast::<[u64; 4]>().add(i).read() };
        for value in row {
            number(value);
            uart_write(b" ");
        }
        uart_write(b"\n");
    }
    uart_write(b"CONTROL END\n");
    park()
}

#[cfg(jnox_permit_phase)]
#[path = "permit_phase.rs"]
pub(super) mod permit;

struct Runtime {
    frequency: u64,
    epoch: u64,
    step: u64,
    next: u64,
    current: usize,
    jobs: [Option<DispatchJob>; 2],
    frames: [*mut Frame; 3],
    queue: ReadyQueue<2>,
    waiting: bool,
    slice_start: u64,
    service: [u64; 2],
    fault: bool,
    stopping: bool,
    missed: Option<u64>,
}

fn state() -> &'static mut Runtime {
    // Access only within IRQ-masked sections; no reference survives a switch.
    unsafe { (*(&raw mut STATE)).as_mut().unwrap_or_else(|| fail()) }
}

fn arm(target: u64, index: u64) {
    unsafe {
        asm!("msr cntp_cval_el0,{}","msr cntp_ctl_el0,{}","isb",in(reg)target,in(reg)1u64,options(nostack))
    };
    log(2, now(), index, target);
}

fn fresh(id: usize) -> *mut Frame {
    #[cfg(not(jnox_control_descriptor))]
    let entries: [extern "C" fn() -> !; 3] = [control, sender, idle];
    // Only the descriptor arm changes the control entry ABI; the CPU enters
    // through ERET, so this table holds code addresses, never a mismatched call.
    #[cfg(jnox_control_descriptor)]
    let entries = [control as *const (), sender as *const (), idle as *const ()];
    unsafe {
        let frame = (&raw mut STACKS)
            .cast::<u8>()
            .add((id + 1) * STACK_BYTES - size_of::<Frame>())
            .cast::<Frame>();
        frame.write_bytes(0, 1);
        (*frame).elr = entries[id] as usize as u64;
        (*frame).spsr = 0x345; // EL1h, IRQ enabled
        frame
    }
}

impl Runtime {
    fn target(&self, index: u64) -> u64 {
        self.epoch + index * self.step
    }
    fn enqueue(&mut self, id: usize) {
        if !self.queue.push_back(id) {
            fail()
        }
    }
    fn timing_fault(&mut self, why: u64) {
        self.fault = true;
        log(15, now(), why, self.next);
        app::inhibit(why);
        #[cfg(jnox_permit_phase)]
        permit::timing_stop(why);
    }
    fn release(&mut self, id: usize, at: u64) {
        if self.jobs[id].is_some() {
            self.timing_fault(4);
            return;
        }
        let period = self.step * (if id == 0 { 2 } else { 4 });
        self.jobs[id] = Some(DispatchJob {
            release: at,
            period,
            relative_deadline: period,
            ..DispatchJob::fixed_priority(0)
        });
        self.frames[id] = fresh(id);
        #[cfg(jnox_control_descriptor)]
        if id == 0 && self.missed.is_none() {
            // IRQ has advanced next from the release's measured phase index.
            // The overlap guard above ensures the previous private frame is dead.
            let second = self.next;
            let first = second.checked_sub(1).unwrap_or_else(|| fail());
            if first % 2 != 0 || second > LAST {
                fail()
            }
            let second_target = at.checked_add(self.step).unwrap_or_else(|| fail());
            // AAPCS64 x0..x4. Initialize once, before making this job runnable.
            unsafe {
                (&mut (*self.frames[id]).gpr)[..5].copy_from_slice(&[
                    first,
                    at,
                    second,
                    second_target,
                    self.step,
                ]);
            }
        }
        self.service[id] = 0;
        log(4, now(), id as u64, at);
        self.enqueue(id);
    }
    fn account(&mut self, entry: u64) {
        if self.current < IDLE {
            let delta = entry - self.slice_start;
            self.service[self.current] += delta;
            log(6, entry, self.current as u64, delta);
        }
    }
    fn select(&mut self, entry: u64, kind: u64) -> *mut Frame {
        #[cfg(jnox_permit_phase)]
        if permit::stopped() && (self.missed.is_none() || self.stopping) {
            log(kind, entry, now(), IDLE as u64);
            permit::terminal(self);
        }
        let policy = if cfg!(jnox_control_dm) {
            DispatchPolicy::DeadlineMonotonic
        } else {
            DispatchPolicy::RateMonotonic
        };
        let jobs = &self.jobs;
        let id = policy
            .pop_ready(&mut self.queue, now(), |id| jobs[id])
            .unwrap_or(IDLE);
        self.current = id;
        let f = core::mem::replace(&mut self.frames[id], ptr::null_mut());
        if f.is_null() {
            fail()
        }
        log(5, now(), id as u64, self.next);
        let until = now();
        log(kind, entry, until, id as u64);
        self.slice_start = now();
        f
    }
}

extern "C" fn irq(frame: *mut Frame, entry: u64) -> *mut Frame {
    // Preserve an event across the idle work check -> WFE race. No task
    // other than idle executes WFE while this workload is active.
    unsafe { asm!("sev", options(nostack, preserves_flags)) };
    #[cfg(jnox_control_cold)]
    let handler = now();
    let iar = gic_read(GICC + 0x0c);
    #[cfg(jnox_control_cold)]
    if cold::on_irq(frame, entry, handler, iar) {
        return frame;
    }
    #[cfg(jnox_permit_phase)]
    {
        let (ctl, cval): (u64, u64);
        unsafe {
            asm!("mrs {},cntp_ctl_el0","mrs {},cntp_cval_el0",out(reg)ctl,out(reg)cval,options(nostack))
        };
        log(87, entry, ctl, iar as u64);
        log(88, entry, cval, gic_read(GICD + 0x200) as u64);
    }
    if iar & 0x3ff != TIMER_IRQ {
        fail()
    }
    unsafe { asm!("msr cntp_ctl_el0,xzr", "isb", options(nostack)) };
    gic_write(GICC + 0x10, iar);
    let s = state();
    s.account(entry);
    let id = s.current;
    log(45, entry, id as u64, unsafe { (*frame).elr });
    log(46, entry, requested_mask(frame), s.next);
    s.frames[id] = frame;
    if id < IDLE {
        s.enqueue(id)
    }
    #[cfg(jnox_permit_phase)]
    if entry < s.target(s.next) && permit::service_wake(s, entry) {
        permit::program(s);
        return s.select(entry, 34);
    }
    let requested = s.next;
    if entry < s.target(requested) {
        fail()
    }
    let index = (entry - s.epoch) / s.step;
    let at = s.target(index);
    log(3, entry, requested, index);
    if index != requested || entry - s.target(requested) > s.step / 5 {
        s.timing_fault(1)
    }
    s.next = index + 1;
    if index != requested {
        // Fail this run and perform one evaluation on the control stack against
        // the actual time. Do not release a burst of historical jobs.
        s.missed = Some(index);
        if s.jobs[0].is_none() {
            s.release(0, at)
        } else if s.waiting {
            s.waiting = false;
            s.enqueue(0)
        }
        return s.select(entry, 34);
    }
    if index > LAST {
        s.timing_fault(5);
        s.stopping = true
    }
    if index % 2 == 0 {
        s.release(0, at);
        if index % 4 == 0 {
            s.release(1, at)
        }
    } else if s.waiting {
        s.waiting = false;
        s.enqueue(0);
        log(7, now(), 0, at);
    } else {
        s.timing_fault(2);
    }
    if index < LAST {
        #[cfg(not(jnox_permit_phase))]
        arm(s.target(s.next), s.next);
        #[cfg(jnox_permit_phase)]
        {
            permit::service_wake(s, entry);
            permit::program(s);
        }
    }
    s.select(entry, 34)
}

extern "C" fn svc(frame: *mut Frame, entry: u64) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {},esr_el1",out(reg)esr,options(nostack)) };
    if esr >> 26 != 0x15 {
        fail()
    }
    let s = state();
    let id = s.current;
    if id >= IDLE {
        fail()
    }
    s.account(entry);
    if id == 0 && s.missed.is_some() && !s.stopping {
        // The late IRQ arrived after the most recent safety evaluation. Re-enter
        // the worker on its current stack at a dedicated non-returning function.
        unsafe { (*frame).elr = final_check as *const () as usize as u64 };
        s.frames[0] = frame;
        s.enqueue(0);
        return s.select(entry, 35);
    }
    #[cfg(jnox_permit_phase)]
    if id == 1 && esr & 0xffff == 3 {
        // Sleep carries prior CPU slices into the same activation's final debit.
        s.frames[1] = frame;
        if s.service[1] == 0
            || u128::from(s.service[1]) * 1_000_000
                >= u128::from(SENDER_US) * u128::from(s.frequency)
        {
            s.timing_fault(3);
        } else {
            permit::sleep(s, entry);
        }
        return s.select(entry, 35);
    }
    let use_ticks = s.service[id];
    log(8, entry, id as u64, use_ticks);
    let budget = if id == 0 { CONTROL_US } else { SENDER_US };
    if use_ticks == 0
        || u128::from(use_ticks) * 1_000_000 > u128::from(budget) * u128::from(s.frequency)
    {
        s.timing_fault(3)
    }
    s.service[id] = 0;
    match esr & 0xffff {
        1 if id == 0 => {
            s.frames[0] = frame;
            s.waiting = true;
        }
        2 => {
            let job = s.jobs[id].take().unwrap_or_else(|| fail());
            log(9, entry, id as u64, job.release);
            if entry - job.release > job.relative_deadline {
                s.timing_fault(6)
            }
            if id == 0 && s.next > LAST {
                s.stopping = true
            }
        }
        _ => fail(),
    }
    s.select(entry, 35)
}

extern "C" fn final_check() -> ! {
    locked(|| {
        let index = state().missed.take().unwrap_or_else(|| fail());
        let (step, target) = (state().step, state().target(index));
        app::check(index, step);
        log(10, now(), index, target);
        state().stopping = true;
    });
    unsafe { asm!("svc #2", options(noreturn)) }
}

extern "C" fn control(
    #[cfg(jnox_control_descriptor)] first_index: u64,
    #[cfg(jnox_control_descriptor)] first_target: u64,
    #[cfg(jnox_control_descriptor)] second_index: u64,
    #[cfg(jnox_control_descriptor)] second_target: u64,
    #[cfg(jnox_control_descriptor)] descriptor_step: u64,
) -> ! {
    #[cfg(jnox_control_descriptor)]
    let phases = [(first_index, first_target), (second_index, second_target)];
    for phase in 0..2 {
        if locked(|| state().missed.is_some()) {
            final_check()
        }
        #[cfg(jnox_control_descriptor)]
        let (index, target, step) = {
            let (index, target) = phases[phase];
            (index, target, descriptor_step)
        };
        #[cfg(not(jnox_control_descriptor))]
        let (index, target, step) = locked(|| {
            let s = state();
            let r = s.jobs[0].unwrap().release;
            ((r - s.epoch) / s.step + phase, r + phase * s.step, s.step)
        });
        if cfg!(jnox_control_case = "demand") && index >= 12 {
            let start = now();
            while now() - start < step / 8 {
                core::hint::spin_loop()
            }
        }
        locked(|| {
            if state().missed.is_some() {
                final_check()
            }
            app::check(index, step);
            let finish = now();
            log(10, finish, index, target);
            if finish < target || finish - target > step * 2 / 5 {
                state().timing_fault(7)
            }
        });
        if phase == 0 {
            unsafe { asm!("svc #1", options(nostack)) }
        }
    }
    unsafe { asm!("svc #2", options(noreturn)) }
}

#[cfg(jnox_permit_phase)]
extern "C" fn sender() -> ! {
    permit::sender()
}
#[cfg(not(jnox_permit_phase))]
extern "C" fn sender() -> ! {
    if !locked(app::sender_owns) {
        locked(|| log(37, now(), 1, state().next));
        unsafe { asm!("svc #2", options(noreturn)) }
    }
    let (index, target, step) = locked(|| {
        let s = state();
        let r = s.jobs[1].unwrap().release;
        ((r - s.epoch) / s.step, r + s.step / 5, s.step)
    });
    // The worker may be late. Sample the hardware on the actual issue path.
    while now().saturating_add(step / 200) < target {
        core::hint::spin_loop()
    }
    locked(|| {
        let issued = sample_at(target);
        app::send(index, step, issued);
    });
    unsafe { asm!("svc #2", options(noreturn)) }
}

pub(super) fn sample_at(target: u64) -> u64 {
    let sampled: u64;
    unsafe {
        asm!("2:","isb","mrs {sampled},cntpct_el0","cmp {sampled},{target}","b.lo 2b",
        sampled=out(reg)sampled,target=in(reg)target,options(nostack))
    };
    sampled
}

fn requested_mask(frame: *mut Frame) -> u64 {
    unsafe { (*frame).spsr & 0x3c0 }
}

extern "C" fn idle() -> ! {
    // Start the one immutable epoch after idle's frame is restored. No
    // worker/IRQ path is warmed, and every scored release remains visible.
    #[cfg(jnox_control_cold)]
    cold::idle_start();
    #[cfg(not(jnox_control_cold))]
    locked(|| {
        let s = state();
        log(43, now(), s.frequency, s.step);
        s.epoch = now() + s.step;
        log(1, s.epoch, s.frequency, LAST);
        arm(s.epoch, 0);
    });
    let mut last_wait = None;
    loop {
        let (wait, index) = locked(|| {
            app::observe();
            let s = state();
            if s.stopping {
                let (duration, start, end) = unsafe { LONGEST_MASKED };
                log(33, start, end, duration);
                #[cfg(jnox_permit_phase)]
                permit::end_witness();
                log(18, now(), app::read_output() as u64, s.fault as u64);
                dump()
            }
            if app::HANDOFF {
                app::supervise(s.epoch, s.step);
            } else if let Some(target) = app::command_target(s.step) {
                if s.next == 7 {
                    if now().saturating_add(s.step / 200) < target {
                        return (false, s.next);
                    }
                    #[cfg(not(jnox_permit_phase))]
                    app::command(sample_at(target));
                    #[cfg(jnox_permit_phase)]
                    permit::supervisor(target, s.step);
                }
            }
            if cfg!(jnox_control_case = "late") && s.next == 12 {
                while now() < s.target(12) + s.step / 4 {
                    core::hint::spin_loop()
                }
            }
            (!app::HANDOFF || !app::pending_subphase(s.next), s.next)
        });
        if wait {
            if last_wait != Some(index) {
                let mask: u64;
                unsafe { asm!("mrs {},daif",out(reg)mask,options(nostack,preserves_flags)) };
                let sampled = now();
                locked(|| log(44, sampled, index, mask));
                last_wait = Some(index);
            }
            // The exception handler sends SEV even when idle was interrupted
            // immediately before WFE. A pending event prevents a lost wake.
            unsafe { asm!("wfe", options(nostack, preserves_flags)) }
        }
    }
}

pub(super) fn run() -> ! {
    init_timer_irq();
    let frequency: u64;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)frequency,options(nostack)) };
    #[cfg(not(jnox_control_cold))]
    #[cfg(not(jnox_permit_phase))]
    if frequency == 0 || frequency % 400 != 0 {
        fail()
    }
    #[cfg(jnox_control_cold)]
    cold::boot(frequency);
    let mut case_hash = 0u64;
    for byte in env!("JNOX_BUILT_CONTROL_CASE").bytes() {
        case_hash = case_hash.wrapping_mul(31).wrapping_add(byte as u64)
    }
    log(32, now(), cfg!(jnox_control_dm) as u64, case_hash);
    app::init();
    #[cfg(jnox_permit_phase)]
    if !permit::admit(frequency) {
        log(18, now(), app::read_output() as u64, 0);
        dump()
    }
    #[cfg(jnox_control_cold)]
    cold::initial_off();
    if app::SCRIPTED {
        app::script(frequency);
        dump()
    }
    let step = frequency / 400;
    unsafe {
        STATE = Some(Runtime {
            frequency,
            epoch: 0,
            step,
            next: 0,
            current: IDLE,
            jobs: [None; 2],
            frames: [ptr::null_mut(); 3],
            queue: ReadyQueue::new(),
            waiting: false,
            slice_start: 0,
            service: [0; 2],
            fault: false,
            stopping: false,
            missed: None,
        })
    };
    unsafe { sched_start_frame(fresh(IDLE)) }
}
