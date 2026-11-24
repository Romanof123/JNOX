use super::context::*;
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

pub const CAP: usize = 4;
pub struct Task {
    pub entry: fn(),
    pub class: u8,
}
pub enum Mode {
    Strict(u64),
    Feedback { allotments: [u64; 3], period: u64 },
}
use super::multilevel_policy::Multilevel;
static mut STACKS: [Stack; CAP] = [const { Stack([0; STACK_BYTES]) }; CAP];
static mut STATE: Option<Runtime> = None;
struct Runtime {
    policy: Multilevel<CAP>,
    frames: [*mut Frame; CAP],
    live: [bool; CAP],
    owner: Option<usize>,
    start: u64,
    due: u64,
    service: [u64; CAP],
    total: u64,
    #[cfg(jnox_multilevel_limits)]
    actual_compare: u64,
    #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
    arm_control: u64,
    period: Option<u64>,
}
global_asm!(
    include_str!("context.S"),
    include_str!("fixed_runtime.S"),
    frame_size = const size_of::<Frame>(),
    elr = const offset_of!(Frame, elr),
    fpcr = const offset_of!(Frame, fpcr),
    tpidr_el1 = const offset_of!(Frame, tpidr_el1),
    tpidr_el0 = const offset_of!(Frame, tpidr_el0),
    simd = const offset_of!(Frame, simd),
    irq = sym interrupt, svc = sym syscall, fail = sym fail,
);

pub fn yield_now() {
    unsafe { asm!("svc #0", options(preserves_flags)) }
}

#[allow(dead_code)]
pub fn exit() -> ! {
    unsafe { asm!("svc #1", "brk #0", options(noreturn)) }
}

pub fn block() {
    unsafe { asm!("svc #2", options(preserves_flags)) }
}
pub fn wake(id: usize) -> bool {
    let result: usize;
    unsafe { asm!("svc #3", inout("x0") id => result, options(preserves_flags)) }
    result != 0
}
unsafe extern "C" {
    fn fixed_runtime_enter();
    fn fixed_runtime_idle() -> !;
}

pub fn frequency() -> u64 {
    let hz;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) hz, options(nostack, preserves_flags)) }
    hz
}

pub fn counter() -> u64 {
    let now;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) now, options(nostack, preserves_flags)) }
    now
}

#[cfg(jnox_multilevel_limits)]
#[unsafe(no_mangle)]
static mut multilevel_terminal_ctl: u64 = u64::MAX;

#[cfg(jnox_multilevel_limits)]
fn timer_control() -> u64 {
    let ctl;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) }
    ctl
}

fn disable_timer() {
    unsafe {
        asm!(
            "msr cntp_ctl_el0, xzr",
            "isb",
            options(nostack, preserves_flags)
        )
    }
}

pub(super) extern "C" fn fail() -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) }
    disable_timer();
    super::uart_write(b"MULTILEVEL FAIL\n");
    super::park()
}

fn stack_base(slot: usize) -> *mut u8 {
    (&raw mut STACKS)
        .cast::<u8>()
        .wrapping_add(slot * STACK_BYTES)
}

fn initial_frame(slot: usize, pc: usize, argument: usize) -> *mut Frame {
    let fp = stack_base(slot)
        .wrapping_add(STACK_BYTES - size_of::<Frame>())
        .cast::<Frame>();
    // Boot owns these distinct BSS-zeroed stacks until the first ERET.
    unsafe {
        (*fp).elr = pc as u64;
        (*fp).spsr = 0x345; // EL1h, IRQ enabled, D/A/F masked.
        (*fp).gpr[0] = argument as u64;
    }
    fp
}

pub fn run(tasks: &'static [Task], mode: Mode) -> ! {
    unsafe { asm!("msr daifset, #0xf", options(nostack, preserves_flags)) }
    if tasks.len() > CAP || frequency() == 0 || unsafe { (&*(&raw const STATE)).is_some() } {
        fail()
    }
    let classes = core::array::from_fn(|i| tasks.get(i).map_or(0, |t| t.class));
    let (policy, period) = match mode {
        Mode::Strict(quantum) => (Multilevel::fixed(classes, quantum), None),
        Mode::Feedback { allotments, period } if period > 0 => {
            (Multilevel::feedback(allotments), Some(period))
        }
        _ => fail(),
    };
    let mut runtime = Runtime {
        policy: policy.unwrap_or_else(|| fail()),
        period,
        owner: None,
        frames: [ptr::null_mut(); CAP],
        live: [false; CAP],
        service: [0; CAP],
        start: 0,
        due: 0,
        total: 0,
        #[cfg(jnox_multilevel_limits)]
        actual_compare: 0,
        #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
        arm_control: 0,
    };
    for (id, task) in tasks.iter().enumerate() {
        runtime.frames[id] = initial_frame(
            id,
            fixed_runtime_enter as *const () as usize,
            task.entry as usize,
        );
        runtime.live[id] = true;
        if !runtime.policy.wake(id) {
            fail()
        }
    }
    init_timer_irq();
    let state = unsafe { (&mut *(&raw mut STATE)).insert(runtime) };
    #[cfg(jnox_mlq_admission)]
    super::mlq_admission::epoch(counter());
    unsafe { sched_start_frame(state.select(Event::default())) }
}

#[derive(Default)]
#[allow(dead_code)]
struct Event {
    kind: u64,
    arg: u64,
    result: u64,
    owner: Option<usize>,
    start: u64,
    due: u64,
    stop: u64,
    boost: bool,
    #[cfg(jnox_mlfq_wake)]
    target_before_reset: u8,
    frame: *mut Frame,
    source: u64,
    #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
    pre_reset: [u64; 3],
    #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
    before: u64,
}

impl Runtime {
    fn select(&mut self, event: Event) -> *mut Frame {
        #[cfg(any(jnox_mlfq_wake, jnox_multilevel_trace))]
        let queue = self
            .policy
            .queued()
            .iter()
            .fold(0u64, |q, id| (q << 4) | (*id as u64 + 1));
        self.owner = self.policy.dispatch();
        #[cfg(jnox_multilevel_trace)]
        record(self, &event, queue);
        #[cfg(all(not(jnox_multilevel_trace), not(jnox_multilevel_limits)))]
        let _ = event;
        let Some(id) = self.owner else {
            disable_timer();
            #[cfg(jnox_multilevel_limits)]
            {
                let ctl = timer_control();
                unsafe { ptr::write_volatile(&raw mut multilevel_terminal_ctl, ctl); }
                #[cfg(jnox_multilevel_trace)]
                unsafe {
                    let j = &mut *(&raw mut ordinary_multilevel_witness);
                    j.limits[j.count as usize - 1][8] = ctl;
                }
                if ctl & 1 != 0 { fail() }
            }
            #[cfg(jnox_mlfq_wake)]
            {
                super::mlfq_wake::boundary(event.kind, event.result, event.owner.unwrap_or(CAP),
                    event.start, event.due, event.stop, self.total, event.boost, queue,
                    CAP, 0, self.policy.priority(0).unwrap_or_else(|| fail()),
                    event.frame.addr(), 0, event.target_before_reset);
                super::mlfq_wake::terminal();
            }
            #[cfg(jnox_mlq_admission)]
            super::mlq_admission::terminal(self.service);
            if self.live.contains(&true) {
                super::uart_write(b"MULTILEVEL STALL\n");
            } else {
                super::uart_write(b"MULTILEVEL DONE\n");
            }
            unsafe { fixed_runtime_idle() }
        };
        let mut grant = self.policy.remaining(id).unwrap_or_else(|| fail());
        if let Some(period) = self.period {
            grant = grant.min(period - self.total % period);
        }
        self.start = counter();
        #[cfg(jnox_mlq_admission)]
        super::mlq_admission::dispatch(id, self.start, self.service, event.owner, event.kind);
        #[cfg(jnox_mlfq_wake)]
        super::mlfq_wake::boundary(
            event.kind, event.result, event.owner.unwrap_or(CAP), event.start, event.due,
            event.stop, self.total, event.boost, queue, id, self.start,
            self.policy.priority(0).unwrap_or_else(|| fail()),
            event.frame.addr(), self.frames[id].addr(), event.target_before_reset,
        );
        self.due = self.start.checked_add(grant).unwrap_or_else(|| fail());
        let compare = self.due;
        #[cfg(jnox_app = "mlfq-delayed")]
        let compare = if event.owner.is_none() {
            let period = self.period.unwrap_or_else(|| fail());
            period.checked_mul(3).and_then(|n| n.checked_add(period / 4))
                .and_then(|n| self.start.checked_add(n)).unwrap_or_else(|| fail())
        } else { compare };
        #[cfg(any(jnox_app = "mlq-saturated", jnox_app = "mlfq-delayed"))]
        if event.owner.is_none() {
            // Both peers use this one absolute end, measured from initial dispatch.
            let units = if cfg!(jnox_app = "mlq-saturated") { 24 } else { 96 };
            let end = (frequency() / 100).checked_mul(units)
                .and_then(|n| self.start.checked_add(n)).unwrap_or_else(|| fail());
            super::multilevel_apps::WORK_END.store(end, core::sync::atomic::Ordering::Relaxed);
            #[cfg(jnox_multilevel_trace)]
            unsafe {
                let j = &mut *(&raw mut ordinary_multilevel_witness);
                j.limits[j.count as usize - 1][9] = end;
            }
        }
        unsafe {
            asm!("msr cntp_cval_el0, {}", "msr cntp_ctl_el0, {}", "isb",
            in(reg) compare, in(reg) 1u64, options(nostack, preserves_flags))
        }
        #[cfg(jnox_multilevel_limits)]
        {
            unsafe { asm!("mrs {}, cntp_cval_el0", out(reg) self.actual_compare, options(nostack, preserves_flags)); }
            let ctl = timer_control();
            if self.actual_compare != compare || ctl & 3 != 1 { fail() }
            #[cfg(jnox_multilevel_trace)]
            { self.arm_control = ctl; }
        }
        self.frames[id]
    }

    fn finish(&mut self, frame: *mut Frame, kind: u64, stop: u64, source: u64) -> *mut Frame {
        let id = self.owner.take().unwrap_or_else(|| fail());
        let base = stack_base(id).addr();
        if !frame.is_aligned()
            || !(base..=base + STACK_BYTES - size_of::<Frame>()).contains(&frame.addr())
        {
            fail()
        }
        self.frames[id] = frame;
        let arg = unsafe { (*frame).gpr[0] };
        let elapsed = stop.checked_sub(self.start).unwrap_or_else(|| fail());
        let before = self.total;
        self.total = before.checked_add(elapsed).unwrap_or_else(|| fail());
        self.service[id] = self.service[id]
            .checked_add(elapsed)
            .unwrap_or_else(|| fail());
        let mut result = 0;
        if kind == 3
            && (arg as usize) < CAP
            && self.live[arg as usize]
            && self.policy.wake(arg as usize)
        {
            result = 1;
        }
        if kind == 3 {
            unsafe {
                (*frame).gpr[0] = result;
            }
        }
        if kind == 1 {
            self.live[id] = false;
        }
        if !self
            .policy
            .close_interval(id, elapsed, matches!(kind, 1 | 2))
        {
            fail()
        }
        #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
        let pre_reset = [
            self.policy.queued().iter().fold(0, |q, id| (q << 4) | (*id as u64 + 1)),
            self.policy.priority(id).unwrap_or_else(|| fail()) as u64,
            self.policy.remaining(id).unwrap_or_else(|| fail()),
        ];
        let boost = self.period.is_some_and(|p| self.total / p != before / p);
        #[cfg(jnox_mlfq_wake)]
        let target_before_reset = self.policy.priority(0).unwrap_or_else(|| fail());
        if boost && !self.policy.boost() {
            fail()
        }
        self.select(Event {
            kind,
            arg,
            result,
            owner: Some(id),
            start: self.start,
            due: self.due,
            stop,
            boost,
            #[cfg(jnox_mlfq_wake)]
            target_before_reset,
            frame,
            source,
            #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
            pre_reset,
            #[cfg(all(jnox_multilevel_limits, jnox_multilevel_trace))]
            before,
        })
    }
}

unsafe fn state() -> &'static mut Runtime {
    unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap_or_else(|| fail()) }
}

extern "C" fn syscall(frame: *mut Frame) -> *mut Frame {
    let now = counter();
    disable_timer();
    let esr: u64;
    unsafe { asm!("mrs {}, esr_el1", out(reg) esr, options(nostack, preserves_flags)) }
    let kind = esr & 0xffff;
    if esr >> 26 != 0x15 || kind > 3 {
        fail()
    }
    unsafe { state().finish(frame, kind, now, esr) }
}

extern "C" fn interrupt(frame: *mut Frame) -> *mut Frame {
    let now = counter();
    let iar = gic_read(GICC + 0x0c);
    if iar & 1023 >= 1020 {
        return frame;
    }
    let ctl: u64;
    unsafe { asm!("mrs {}, cntp_ctl_el0", out(reg) ctl, options(nostack, preserves_flags)) }
    disable_timer();
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
    gic_write(GICC + 0x10, iar);
    let state = unsafe { state() };
    #[cfg(jnox_multilevel_limits)]
    if now < state.actual_compare { fail() }
    if iar & 1023 != TIMER_IRQ || ctl & 7 != 5 || now < state.due {
        fail()
    }
    state.finish(frame, 4, now, (ctl << 32) | u64::from(iar))
}

#[cfg(jnox_multilevel_trace)]
const WORDS: usize = 27 + size_of::<Frame>() / 8;
#[cfg(jnox_multilevel_trace)]
#[repr(C)]
struct Journal {
    count: u64,
    frequency: u64,
    rows: [[u64; WORDS]; 256],
    #[cfg(jnox_multilevel_limits)]
    limits: [[u64; 11]; 256],
}
#[cfg(jnox_multilevel_trace)]
#[unsafe(no_mangle)]
static mut ordinary_multilevel_witness: Journal = Journal {
    count: 0,
    frequency: 0,
    rows: [[0; WORDS]; 256],
    #[cfg(jnox_multilevel_limits)]
    limits: [[0; 11]; 256],
};

#[cfg(jnox_multilevel_trace)]
fn record(rt: &Runtime, e: &Event, queue: u64) {
    unsafe {
        let journal = &mut *(&raw mut ordinary_multilevel_witness);
        let i = journal.count as usize;
        let row = journal.rows.get_mut(i).unwrap_or_else(|| fail());
        journal.frequency = frequency();
        row[..15].copy_from_slice(&[
            e.kind,
            e.arg,
            e.result,
            e.owner.unwrap_or(CAP) as u64,
            e.start,
            e.due,
            e.stop,
            e.boost as u64,
            e.frame.addr() as u64,
            e.source,
            rt.owner.unwrap_or(CAP) as u64,
            queue,
            rt.total,
            rt.owner.map_or(0, |id| rt.frames[id].addr() as u64),
            rt.owner.map_or(0, |id| (*rt.frames[id]).elr),
        ]);
        for id in 0..CAP {
            row[15 + id] = rt.policy.priority(id).unwrap_or_else(|| fail()) as u64;
            row[19 + id] = rt.policy.remaining(id).unwrap_or_else(|| fail());
            row[23 + id] = rt.service[id];
        }
        if !e.frame.is_null() {
            ptr::copy_nonoverlapping(
                e.frame.cast::<u64>(),
                row[27..].as_mut_ptr(),
                size_of::<Frame>() / 8,
            );
            // Wake's return value is installed in x0 for ERET. Preserve raw entry x0 in the copy.
            row[27] = e.arg;
        }
        #[cfg(jnox_multilevel_limits)]
        {
            fn mask(flags: &[bool]) -> u64 {
                flags.iter().enumerate().fold(0, |bits, (id, &set)| bits | ((set as u64) << id))
            }
            #[cfg(any(jnox_app = "mlq-saturated", jnox_app = "mlfq-delayed"))]
            let end = super::multilevel_apps::WORK_END.load(core::sync::atomic::Ordering::Relaxed);
            #[cfg(not(any(jnox_app = "mlq-saturated", jnox_app = "mlfq-delayed")))]
            let end = 0;
            journal.limits[i] = [
                e.pre_reset[0], e.pre_reset[1], e.pre_reset[2], rt.policy.resets,
                mask(&rt.live), mask(rt.policy.asleep()), rt.actual_compare, rt.arm_control,
                u64::MAX, end, e.before,
            ];
        }
        ptr::write_volatile(&mut journal.count, i as u64 + 1);
    }
}
