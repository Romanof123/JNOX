//! Single-core native task-policy execution on the physical counter timeline.
use crate::native_elf::{LoadPlan, Permission};
use crate::task_policy_image::{self as image, TASK_COUNT};
use crate::task_policy_scheduler::{Config, Event, Kind, Scheduler};
use crate::{context, native_el0_mmu, park, uart_write};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};

global_asm!(include_str!("native_el0_vectors.S"), options(raw));
include!("native_v1_note.rs");

unsafe extern "C" {
    fn native_el0_enter(entry: u64, sp: u64, spsr: u64, locator: u64) -> !;
    fn native_sleep_switch(top: *mut u8);
}

#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct NativeFrame {
    x: [u64; 31],
    esr: u64,
    spsr: u64,
    elr: u64,
    sp_el0: u64,
    current_el: u64,
    fpcr: u64,
    fpsr: u64,
    q: [[u64; 2]; 32],
    tls: [u64; 2],
}
const _: () = assert!(
    size_of::<NativeFrame>() == 832
        && offset_of!(NativeFrame, esr) == 248
        && offset_of!(NativeFrame, spsr) == 256
        && offset_of!(NativeFrame, elr) == 264
        && offset_of!(NativeFrame, sp_el0) == 272
        && offset_of!(NativeFrame, current_el) == 280
        && offset_of!(NativeFrame, q) == 304
);

// Initialized once on this masked boot path, before the first EL0 entry.
static mut CLOCK_DOMAIN: (u64, u64) = (0, 0);

fn counter() -> u64 {
    let ticks: u64;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) ticks, options(nostack, preserves_flags)) };
    ticks
}

fn clock_gettime(clock_id: u64, address: usize, now: u64) -> i64 {
    if clock_id != 1 {
        return -22;
    }
    let (epoch, frequency) = unsafe { CLOCK_DOMAIN };
    let Some(elapsed) = now.checked_sub(epoch) else {
        return -75;
    };
    write_clock(address, elapsed, frequency)
}

fn write_clock(address: usize, elapsed: u64, frequency: u64) -> i64 {
    let ns = u128::from(elapsed) * 1_000_000_000 / u128::from(frequency);
    let Ok(seconds) = i64::try_from(ns / 1_000_000_000) else {
        return -75;
    };
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&seconds.to_le_bytes());
    bytes[8..].copy_from_slice(&((ns % 1_000_000_000) as i64).to_le_bytes());
    if native_el0_mmu::copy_to_user(address, &bytes) {
        0
    } else {
        -14
    }
}

const INITIAL_PSTATE: u64 = 0x340;
const EMPTY_FRAME: NativeFrame = NativeFrame {
    x: [0; 31],
    esr: 0,
    spsr: INITIAL_PSTATE,
    elr: 0,
    sp_el0: 0,
    current_el: 4,
    fpcr: 0,
    fpsr: 0,
    q: [[0; 2]; 32],
    tls: [0; 2],
};

fn arm_compare(earliest: Option<u64>) {
    unsafe {
        if let Some(target) = earliest {
            asm!("msr cntp_cval_el0, {target}", "msr cntp_ctl_el0, {enable}", "isb",
                 target = in(reg) target, enable = in(reg) 1u64, options(nostack, preserves_flags));
        } else {
            asm!(
                "msr cntp_ctl_el0, xzr",
                "isb",
                options(nostack, preserves_flags)
            );
        }
    }
}

fn sleep_until(f: &NativeFrame) -> Result<u64, i64> {
    if f.x[0] != 1 || f.x[1] != 1 {
        return Err(-22);
    }
    monotonic_target(f.x[2] as usize)
}

fn monotonic_target(address: usize) -> Result<u64, i64> {
    let mut request = [0u8; 16];
    if !native_el0_mmu::copy_from_user(address, &mut request) {
        return Err(-14);
    }
    let seconds = i64::from_le_bytes(request[..8].try_into().unwrap());
    let nanos = i64::from_le_bytes(request[8..].try_into().unwrap());
    if seconds < 0 || !(0..1_000_000_000).contains(&nanos) {
        return Err(-22);
    }
    let (epoch, frequency) = unsafe { CLOCK_DOMAIN };
    let hz = u128::from(frequency);
    // Signed LP64 seconds times an unsigned 64-bit frequency fits u128,
    // including the subsecond contribution and the 64-bit epoch.
    let ticks =
        u128::from(epoch) + seconds as u128 * hz + (nanos as u128 * hz).div_ceil(1_000_000_000);
    let Ok(target) = u64::try_from(ticks) else {
        return Err(-75);
    };
    Ok(target)
}

fn decimal(value: i64) {
    if value < 0 {
        uart_write(b"-");
    }
    unsigned(value.unsigned_abs());
}

fn unsigned(mut n: u64) {
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(&digits[i..]);
}

#[derive(Default, Clone, Copy)]
struct Counters {
    releases: u64,
    starts: u64,
    completes: u64,
    misses: u64,
    watchdogs: u64,
    faults: u64,
    exits: u64,
}

// The boot CPU owns these objects. User vectors mask IRQ before Rust entry;
// native_sleep_switch is the sole unmasked EL1 interval and holds no state borrow.
static mut SCHED: Scheduler<TASK_COUNT> = Scheduler::EMPTY;
static mut FRAMES: [NativeFrame; TASK_COUNT] = [EMPTY_FRAME; TASK_COUNT];
static mut COUNTS: [Counters; TASK_COUNT] = [Counters {
    releases: 0,
    starts: 0,
    completes: 0,
    misses: 0,
    watchdogs: 0,
    faults: 0,
    exits: 0,
}; TASK_COUNT];
static mut CURRENT: Option<usize> = None;
static mut IDLE: context::Stack = context::Stack([0; context::STACK_BYTES]);

fn scheduler() -> &'static mut Scheduler<TASK_COUNT> {
    unsafe { &mut *(&raw mut SCHED) }
}

pub(super) fn fail(code: u64) -> ! {
    arm_compare(None);
    uart_write(b"JNOX POLICY KERNEL ERROR code=");
    unsigned(code);
    uart_write(b"\n");
    park()
}

fn task(id: usize) -> image::Task {
    image::task(id).unwrap_or_else(|| fail(2))
}

fn field(label: &[u8], value: u64) {
    uart_write(label);
    unsigned(value);
}

fn header(kind: &[u8], id: usize, at: u64) {
    let t = task(id);
    let app = image::app(t.owner).unwrap_or_else(|| fail(2));
    uart_write(b"JNOX POLICY ");
    uart_write(kind);
    uart_write(b" app=");
    uart_write(app.name.as_bytes());
    uart_write(b" task=");
    uart_write(t.name.as_bytes());
    field(b" tick=", at);
}

fn record(e: Event, observed: u64, source: &[u8], status: Option<i32>) {
    let c = unsafe { &mut COUNTS[e.task] };
    let label: &[u8] = match e.kind {
        Kind::Release => {
            c.releases += 1;
            b"RELEASE"
        }
        Kind::Complete => {
            c.completes += 1;
            b"COMPLETE"
        }
        Kind::DeadlineMiss => {
            c.misses += 1;
            b"DEADLINE"
        }
        Kind::Watchdog => {
            c.watchdogs += 1;
            c.faults += 1;
            b"WATCHDOG"
        }
        Kind::Fault(_) => {
            c.faults += 1;
            b"FAULT"
        }
        Kind::Exit => {
            c.exits += 1;
            b"EXIT"
        }
        Kind::Sleep => b"SLEEP",
        Kind::Wake => b"WAKE",
    };
    header(label, e.task, e.at);
    field(b" observed=", observed);
    uart_write(b" source=");
    uart_write(source);
    if source == b"irq_user" || source == b"irq_idle" {
        field(b" irq=", u64::from(context::TIMER_IRQ));
        if let Some(interrupted) = unsafe { CURRENT } {
            let running = task(interrupted);
            uart_write(b" running_app=");
            uart_write(
                image::app(running.owner)
                    .unwrap_or_else(|| fail(2))
                    .name
                    .as_bytes(),
            );
            uart_write(b" running_task=");
            uart_write(running.name.as_bytes());
        }
    }
    match e.kind {
        Kind::Fault(code) => {
            uart_write(b" status=");
            decimal(code);
        }
        Kind::Exit => {
            uart_write(b" status=");
            decimal(i64::from(status.unwrap_or(0)));
        }
        _ => {}
    }
    uart_write(b"\n");
    if e.kind == Kind::Watchdog {
        header(b"FAULT", e.task, e.at);
        uart_write(b" reason=watchdog status=-110\n");
    }
}

fn advance(now: u64, source: &[u8]) {
    scheduler().advance(now, |e| record(e, now, source, None));
}

fn finish() -> ! {
    arm_compare(None);
    let now = counter();
    for id in 0..TASK_COUNT {
        let c = unsafe { COUNTS[id] };
        header(b"SUMMARY", id, now);
        field(b" releases=", c.releases);
        field(b" starts=", c.starts);
        field(b" completes=", c.completes);
        field(b" misses=", c.misses);
        field(b" watchdogs=", c.watchdogs);
        field(b" faults=", c.faults);
        field(b" exits=", c.exits);
        uart_write(b"\n");
    }
    uart_write(b"JNOX POLICY DONE\n");
    park()
}

fn select() -> usize {
    loop {
        let now = counter();
        advance(now, b"dispatch");
        arm_compare(scheduler().next_timer());
        if let Some(next) = scheduler().pick() {
            let t = task(next);
            if native_el0_mmu::active() != Some(t.owner) {
                unsafe { native_el0_mmu::switch(t.owner).unwrap_or_else(|_| fail(3)) };
            }
            let c = unsafe { &mut COUNTS[next] };
            if c.starts < c.releases {
                c.starts += 1;
                header(b"START", next, counter());
                uart_write(b"\n");
            }
            unsafe { CURRENT = Some(next) };
            return next;
        }
        if scheduler().all_terminal() {
            finish()
        }
        if scheduler().next_timer().is_none() {
            fail(4)
        }
        unsafe { CURRENT = None };
        let top = (&raw mut IDLE)
            .cast::<u8>()
            .wrapping_add(context::STACK_BYTES);
        unsafe { native_sleep_switch(top) };
    }
}

fn current() -> usize {
    let id = unsafe { CURRENT }.unwrap_or_else(|| fail(5));
    if native_el0_mmu::active() != Some(task(id).owner) {
        fail(6)
    }
    id
}

fn dispatch(frame: &mut NativeFrame, id: usize) {
    unsafe { FRAMES[id] = *frame };
    let next = select();
    unsafe { *frame = FRAMES[next] };
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_unexpected() -> ! {
    fail(7)
}

#[unsafe(no_mangle)]
extern "C" fn native_sleep_irq(frame: *mut NativeFrame) -> u64 {
    let iar = context::gic_read(context::GICC + 0x0c);
    let interrupt = iar & 0x3ff;
    if interrupt >= 1020 {
        return 0;
    }
    if interrupt != context::TIMER_IRQ {
        fail(8)
    }
    arm_compare(None);
    let now = counter();
    let source: &[u8] = if frame.is_null() {
        b"irq_idle"
    } else {
        b"irq_user"
    };
    advance(now, source);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    context::gic_write(context::GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    if let Some(f) = unsafe { frame.as_mut() } {
        let id = current();
        dispatch(f, id);
        0
    } else {
        // Resume the interrupted scheduler continuation. It reselects and
        // re-arms the timer before returning to EL0 or sleeping again.
        1
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_sync(f: &mut NativeFrame, vector: u64) {
    let now = counter();
    let id = current();
    advance(now, b"sync");
    if scheduler().is_terminal(id) {
        dispatch(f, id);
        return;
    }
    if vector != 0x400 || f.current_el != 4 || f.spsr & 0x1f != 0 {
        fail(9)
    }
    let (base, top) = native_el0_mmu::policy_stack(task(id).slot).unwrap_or_else(|| fail(10));
    if f.esr >> 26 != 0x15
        || f.esr & 0xffff != 0x4a01
        || !native_el0_mmu::user_return_pc(f.elr as usize)
        || !(base as u64 + 16..=top as u64).contains(&f.sp_el0)
        || f.sp_el0 % 16 != 0
    {
        scheduler().fault(id, -14, now, |e| record(e, now, b"exception", None));
        dispatch(f, id);
        return;
    }
    let status = f.x[0] as i32;
    let emit = |e| record(e, now, b"svc", Some(status));
    let result: i64 = match f.x[8] {
        1 | 82 if i64::from(status) as u64 != f.x[0] => -22,
        1 | 82 => {
            scheduler().exit(id, now, emit);
            0
        }
        4 => crate::native_console::write(f.x[0], f.x[1] as usize, f.x[2] as usize),
        6 => clock_gettime(f.x[0], f.x[1] as usize, now),
        7 => match sleep_until(f) {
            Ok(target) => {
                if target > now {
                    scheduler().sleep(id, target, now, emit);
                }
                0
            }
            Err(errno) => errno,
        },
        8 => {
            scheduler().yield_now(id);
            0
        }
        80 => {
            scheduler().complete(id, now, emit);
            0
        }
        81 if f.x[0] == 0 || f.x[0] > u64::from(u32::MAX) => -22,
        81 => {
            scheduler().fault(id, f.x[0] as i64, now, emit);
            0
        }
        _ => -38,
    };
    f.x[0] = result as u64;
    dispatch(f, id);
}

// The policy ABI specifies integer milliseconds. Reject clock/policy pairs
// without an exact hardware boundary instead of moving a watchdog deadline.
fn ticks(ms: u32, hz: u64) -> u64 {
    let product = u128::from(ms) * u128::from(hz);
    if product % 1000 != 0 {
        fail(11)
    }
    u64::try_from(product / 1000).unwrap_or_else(|_| fail(11))
}



pub(super) fn run() -> ! {
    let hz: u64;
    unsafe {
        asm!("msr daifset,#15", "msr cntp_ctl_el0,xzr","msr cntv_ctl_el0,xzr",
             "isb","mrs {},cntfrq_el0",out(reg) hz,options(nostack,preserves_flags));
        image::install().unwrap_or_else(|reason| {
            uart_write(reason.as_bytes());
            uart_write(b"\n");
            fail(12)
        });
        context::configure_timer_ppi();
        asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    if hz == 0 || TASK_COUNT == 0 {
        fail(13)
    }
    let epoch = counter();
    if !scheduler().reset(epoch, |id| {
        let t = task(id);
        let (base, top) = native_el0_mmu::policy_stack(t.slot).unwrap_or_else(|| fail(10));
        unsafe {
            FRAMES[id].elr = t.entry;
            FRAMES[id].sp_el0 = top as u64;
            FRAMES[id].tls[1] = base as u64;
        }
        Config {
            priority: u8::try_from(t.priority).unwrap_or_else(|_| fail(14)),
            period: ticks(t.period_ms, hz),
            deadline: ticks(t.deadline_ms, hz),
            watchdog: (t.wd_ms != 0).then(|| ticks(t.wd_ms, hz)),
        }
    }) {
        fail(14)
    }
    unsafe { CLOCK_DOMAIN = (epoch, hz) };
    uart_write(b"JNOX POLICY CLOCK");
    field(b" epoch=", epoch);
    field(b" hz=", hz);
    uart_write(b"\n");
    let first = select();
    let f = unsafe { FRAMES[first] };
    unsafe { native_el0_enter(f.elr, f.sp_el0, f.spsr, f.tls[1]) }
}
