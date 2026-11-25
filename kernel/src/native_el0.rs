//! Statically packaged native-v1 processes on a dedicated CPU0 virt boot.
use crate::native_elf::{self, LoadPlan, Permission};
use crate::native_el0_mmu::{self, STACK_BOTTOM, STACK_TOP};
use crate::fixed_policy::{Action, Config, Fixed, Mode};
use crate::ready_queue::ReadyQueue;
use crate::{context, park, uart_write};
use core::arch::{asm, global_asm};
use core::mem::{offset_of, size_of};
use core::ptr;

#[cfg(jnox_native_weighted)]
#[path = "native_weighted.rs"]
mod weighted;
#[cfg(jnox_native_edf)]
#[path = "native_edf.rs"]
mod edf;
#[cfg(jnox_native_weighted)]
use weighted as measured;
#[cfg(jnox_native_edf)]
use edf as measured;

#[cfg(all(jnox_native_weighted, not(jnox_native_weighted_continuous)))]
global_asm!(".set NATIVE_MEASURED,1\n.macro measured_return\nbl native_weighted_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_weighted_continuous)]
global_asm!(".set NATIVE_WEIGHTED_CONTINUOUS,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_weighted_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_edf)]
global_asm!(".set NATIVE_MEASURED,1\n.macro measured_return\nbl native_edf_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_periodic)]
#[path = "native_periodic.rs"]
mod periodic;
#[cfg(jnox_native_periodic)]
use periodic as measured;
#[cfg(jnox_native_periodic)]
global_asm!(".set NATIVE_MEASURED,1\n.macro measured_return\nbl native_periodic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_cbs)]
#[path = "native_cbs.rs"]
mod cbs;
#[cfg(jnox_native_cbs)]
use cbs as measured;
#[cfg(jnox_native_cbs)]
global_asm!(".set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cbs_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_multilevel)]
#[path = "native_multilevel.rs"]
mod multilevel;
#[cfg(jnox_native_multilevel)]
use multilevel as measured;
#[cfg(jnox_native_multilevel)]
global_asm!(".set NATIVE_MEASURED,1\n.macro measured_return\nbl native_multilevel_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(all(jnox_native_threads, not(jnox_native_thread_fixed)))]
#[path = "native_threads.rs"]
mod threads;
#[cfg(jnox_native_thread_fixed)]
#[path = "native_fixed_threads.rs"]
mod threads;
#[cfg(jnox_native_threads)]
global_asm!(".set NATIVE_MEASURED,1\n.macro measured_return\nbl native_threads_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_partition)]
#[path = "native_cyclic.rs"]
mod cyclic;
#[cfg(jnox_native_amc)]
#[path = "native_amc.rs"]
mod amc;
#[cfg(jnox_native_amc)]
include!("native_amc_vectors.rs");
#[cfg(jnox_native_aperiodic)]
#[path = "native_aperiodic.rs"]
mod aperiodic;
#[cfg(jnox_native_aperiodic)]
global_asm!(".set NATIVE_SERVER,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cyclic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_extension)]
#[path = "extension_policy.rs"]
mod extension_policy;
#[cfg(jnox_native_extension)]
#[path = "group_policy.rs"]
mod group_policy;
#[cfg(jnox_native_extension)]
#[path = "native_extension.rs"]
mod extension;
#[cfg(jnox_native_extension)]
include!("native_extension_vectors.rs");
#[cfg(jnox_native_hierarchy)]
#[path = "native_hierarchy.rs"]
mod hierarchy;
#[cfg(jnox_native_hierarchy)]
global_asm!(".set NATIVE_HIERARCHY,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cyclic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_partition)]
global_asm!(".set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cyclic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_batch)]
#[path = "native_batch.rs"]
mod batch;
#[cfg(jnox_native_batch)]
global_asm!(".set NATIVE_BATCH,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_batch_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_proportional)]
#[path = "native_proportional.rs"]
mod proportional;
#[cfg(jnox_native_proportional)]
global_asm!(".set NATIVE_PROPORTIONAL,1\n.set NATIVE_WEIGHTED_CONTINUOUS,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_prop_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(not(any(jnox_native_amc, jnox_native_batch, jnox_native_proportional, jnox_native_extension, jnox_native_hierarchy, jnox_native_aperiodic, jnox_native_partition, jnox_native_weighted, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel, jnox_native_threads)))]
global_asm!(include_str!("native_el0_vectors.S"), options(raw));
unsafe extern "C" {
    fn native_el0_enter(entry: u64, sp: u64, spsr: u64, locator: u64) -> !;
    fn native_sleep_switch(idle_top: *mut u8);
    static native_el0_vectors: u8;
}

const ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_user.elf"));
#[used]
#[unsafe(no_mangle)]
static jnox_native_el0_image: [u8; ELF.len()] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/native_user.elf"));

#[cfg(jnox_native_peer)]
const PEER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_peer.elf"));

#[cfg(jnox_native_peer2)]
const PEER2: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_peer2.elf"));

#[cfg(all(jnox_native_amc, jnox_native_peer3))]
const PEER3: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/native_peer3.elf"));
include!(concat!(env!("OUT_DIR"), "/native_schedule.rs"));
const COUNT: usize = NATIVE_PRIORITIES.len();
const _: () = assert!(COUNT >= 1 && COUNT <= 3 + cfg!(jnox_native_amc) as usize);

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
const _: () = assert!(size_of::<NativeFrame>() == 832
    && offset_of!(NativeFrame, esr) == 248 && offset_of!(NativeFrame, spsr) == 256
    && offset_of!(NativeFrame, elr) == 264 && offset_of!(NativeFrame, sp_el0) == 272
    && offset_of!(NativeFrame, current_el) == 280 && offset_of!(NativeFrame, q) == 304);

#[repr(C)]
struct Journal {
    header: [u64; 16],
    rows: [[u64; 16]; 8],
}
#[unsafe(no_mangle)]
static mut jnox_native_el0_journal: Journal = Journal { header: [0; 16], rows: [[0; 16]; 8] };

fn head(index: usize, value: u64) {
    // One CPU. Boot and the EL1 vectors are masked and never nested.
    unsafe { ptr::write_volatile((&raw mut jnox_native_el0_journal.header).cast::<u64>().add(index), value) };
}

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
    let Some(elapsed) = now.checked_sub(epoch) else { return -75 };
    write_clock(address,elapsed,frequency)
}

fn write_clock(address:usize, elapsed:u64, frequency:u64)->i64 {
    let ns = u128::from(elapsed) * 1_000_000_000 / u128::from(frequency);
    let Ok(seconds) = i64::try_from(ns / 1_000_000_000) else { return -75 };
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&seconds.to_le_bytes());
    bytes[8..].copy_from_slice(&((ns % 1_000_000_000) as i64).to_le_bytes());
    if native_el0_mmu::copy_to_user(address, &bytes) { 0 } else { -14 }
}

#[derive(Clone, Copy, PartialEq)]
enum TaskState { Absent, Ready, Running, Sleeping(u64), Exited(i32) }

const INITIAL_PSTATE: u64 = if COUNT > 1 || cfg!(any(jnox_native_amc, jnox_native_batch, jnox_native_proportional)) || cfg!(jnox_native_weighted_continuous) || cfg!(jnox_native_threads) || cfg!(any(jnox_native_extension, jnox_native_hierarchy, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel)) { 0x340 } else { 0x3c0 };
const EMPTY_FRAME: NativeFrame = NativeFrame {
    x: [0; 31], esr: 0, spsr: INITIAL_PSTATE, elr: 0, sp_el0: 0,
    current_el: 4, fpcr: 0, fpsr: 0, q: [[0; 2]; 32], tls: [0; 2],
};

// CPU0 masks IRQs on every scheduler entry. The idle continuation keeps no
// reference into scheduler storage across the interval with IRQs enabled.
static mut FRAMES: [NativeFrame; COUNT] = [EMPTY_FRAME; COUNT];
static mut STATES: [TaskState; COUNT] = [TaskState::Absent; COUNT];
static mut READY: ReadyQueue<COUNT> = ReadyQueue::new();
static mut CURRENT: usize = 0;
static mut IDLE: context::Stack = context::Stack([0; context::STACK_BYTES]);

fn enqueue(pid: usize) {
    unsafe {
        if !(*(&raw mut READY)).push_back(pid) { fail(9) }
        STATES[pid] = TaskState::Ready;
    }
}

fn update_timer(now: u64) -> bool {
    let mut earliest: Option<u64> = None;
    let mut woke = false;
    for (pid, state) in unsafe { STATES }.into_iter().enumerate() {
        if let TaskState::Sleeping(target) = state {
            if target <= now {
                enqueue(pid);
                woke = true;
            } else {
                earliest = Some(earliest.map_or(target, |prior| prior.min(target)));
            }
        }
    }
    arm_compare(earliest);
    woke
}

fn arm_compare(earliest: Option<u64>) {
    unsafe {
        if let Some(target) = earliest {
            asm!("msr cntp_cval_el0, {target}", "msr cntp_ctl_el0, {enable}", "isb",
                 target = in(reg) target, enable = in(reg) 1u64, options(nostack, preserves_flags));
        } else {
            asm!("msr cntp_ctl_el0, xzr", "isb", options(nostack, preserves_flags));
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
    let ticks = u128::from(epoch) + seconds as u128 * hz
        + (nanos as u128 * hz).div_ceil(1_000_000_000);
    let Ok(target) = u64::try_from(ticks) else { return Err(-75) };
    Ok(target)
}

fn dispatch(frame: &mut NativeFrame) {
    let prior = unsafe { CURRENT };
    unsafe { FRAMES[prior] = *frame };
    loop {
        update_timer(counter());
        if let Some(next) = unsafe { (*(&raw mut READY)).pop_highest(|_| 0) } {
            unsafe {
                if next != prior {
                    native_el0_mmu::switch(next).unwrap_or_else(|_| fail(9));
                    *frame = FRAMES[next];
                }
                CURRENT = next;
                STATES[next] = TaskState::Running;
            }
            if next != prior { event(b"DISPATCH", next, None); }
            return;
        }
        if !unsafe { STATES }.iter().any(|s| matches!(s, TaskState::Sleeping(_))) {
            head(10, 1);
            head(1, 3);
            unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
            park()
        }
        let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
        unsafe { native_sleep_switch(top) };
    }
}

// Fixed owns every selection and service counter in fixed mode. The native
// layer owns address spaces, architectural contexts and application statuses.
static mut FIXED: Option<Fixed> = None;
static mut EXIT_STATUS: [i32; COUNT] = [0; COUNT];

fn policy() -> &'static mut Fixed {
    // Masked, non-nested CPU0 policy entry. Never retain this borrow across idle.
    unsafe { (*(&raw mut FIXED)).as_mut().unwrap_or_else(|| fail(9)) }
}

fn activate(next: usize, frame: &mut NativeFrame) {
    unsafe {
        let prior = CURRENT;
        if prior != next {
            FRAMES[prior] = *frame;
            native_el0_mmu::switch(next).unwrap_or_else(|_| fail(9));
            *frame = FRAMES[next];
            CURRENT = next;
        }
    }
}

const NO_PID: u64 = u64::MAX;
const RECORD_LIMIT: usize = 128;
const FIELDS: usize = 21;
static mut DECISIONS: [[u64; FIELDS]; RECORD_LIMIT] = [[0; FIELDS]; RECORD_LIMIT];
static mut RECORDS: usize = 0;
static mut DROPPED: u64 = 0;

struct Before {
    pid: Option<usize>,
    wake: Option<u64>,
    expiry: Option<u64>,
}

fn record(now: u64, reason: u64, argument: u64, before: Before, interrupted: Option<&NativeFrame>, compare: Option<u64>) {
    let index = unsafe { RECORDS };
    if index == RECORD_LIMIT {
        unsafe { DROPPED = DROPPED.saturating_add(1) };
        return;
    }
    let p = policy();
    let next = p.current();
    let restored = next.map(|id| {
        if Some(id) == before.pid {
            if let Some(frame) = interrupted { return (frame.elr, frame.sp_el0); }
        }
        unsafe { (FRAMES[id].elr, FRAMES[id].sp_el0) }
    });
    let mut row = [0; FIELDS];
    row[..12].copy_from_slice(&[
        now, reason, argument, before.pid.map_or(NO_PID, |id| id as u64),
        next.map_or(NO_PID, |id| id as u64),
        interrupted.map_or(0, |f| f.elr), interrupted.map_or(0, |f| f.sp_el0),
        restored.map_or(0, |f| f.0), restored.map_or(0, |f| f.1),
        before.wake.unwrap_or(0), before.expiry.unwrap_or(0), compare.unwrap_or(0),
    ]);
    row[12..15].fill(NO_PID);
    p.ready_order(&mut row[12..15]);
    for id in 0..COUNT {
        row[15 + id] = p.service(id).unwrap_or_else(|| fail(9));
        row[18 + id] = p.remaining(id);
    }
    unsafe {
        DECISIONS[index] = row;
        RECORDS = index + 1;
    }
}

fn start_fixed() -> usize {
    let config = NATIVE_PRIORITIES.map(|priority| Config { priority, delay: 0 });
    let mode = match NATIVE_POLICY {
        "fifo" => Mode::Fifo,
        "rr" => Mode::RoundRobin(NATIVE_QUANTUM_TICKS),
        _ => fail(9),
    };
    let now = counter();
    unsafe { FIXED = Fixed::new(&config, mode, now) };
    let p = policy();
    let next = p.current().unwrap_or_else(|| fail(9));
    let compare = p.deadline().unwrap_or_else(|_| fail(10));
    arm_compare(compare);
    record(now, 0, 0, Before { pid: None, wake: None, expiry: None }, None, compare);
    next
}

fn fixed_event(now: u64, action: Action, reason: u64, frame: Option<&NativeFrame>) -> Option<usize> {
    let p = policy();
    let before = Before {
        pid: p.current(), wake: p.next_wake(), expiry: p.expiry().unwrap_or_else(|_| fail(10)),
    };
    let argument = match action { Action::Sleep(delay) => delay, _ => 0 };
    if p.event(now, action) != Some(true) { fail(9) }
    let next = p.current();
    let compare = p.deadline().unwrap_or_else(|_| fail(10));
    arm_compare(compare);
    record(now, reason, argument, before, frame, compare);
    next
}

fn finish_fixed() -> ! {
    head(10, 1);
    head(1, 3);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    for index in 0..unsafe { RECORDS } {
        uart_write(b"NATIVE FIXED EVENT");
        for value in unsafe { DECISIONS[index] } {
            uart_write(b" ");
            unsigned(value);
        }
        uart_write(b"\n");
    }
    uart_write(b"NATIVE FIXED DROPPED ");
    unsigned(unsafe { DROPPED });
    uart_write(b"\n");
    for (pid, status) in unsafe { EXIT_STATUS }.into_iter().enumerate() {
        if COUNT == 1 {
            uart_write(b"NATIVE EL0 EXIT ");
            decimal(i64::from(status));
            uart_write(b"\n");
        } else {
            event(b"EXIT", pid, Some(i64::from(status)));
        }
    }
    park()
}

fn event(kind: &[u8], pid: usize, value: Option<i64>) {
    if COUNT > 1 {
        uart_write(b"NATIVE EL0 ");
        uart_write(kind);
        uart_write(b" PID ");
        decimal(pid as i64);
        if let Some(value) = value {
            uart_write(b" ");
            decimal(value);
        }
        uart_write(b"\n");
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_sleep_irq(frame: *mut NativeFrame, entry: u64) -> u64 {
    let iar = context::gic_read(context::GICC + 0x0c);
    let id = iar & 0x3ff;
    #[cfg(jnox_native_batch)]
    { return batch::irq(entry, frame, iar); }
    #[cfg(jnox_native_extension)]
    { return extension::irq(entry, frame, iar); }
    #[cfg(jnox_native_hierarchy)]
    { return hierarchy::irq(entry, frame, iar); }
    #[cfg(jnox_native_proportional)]
    { return proportional::irq(entry, frame, iar); }
    #[cfg(jnox_native_amc)]
    { return amc::irq(entry, frame, iar); }
    #[cfg(jnox_native_aperiodic)]
    { return aperiodic::irq(entry, frame, iar); }
    #[cfg(jnox_native_weighted_continuous)]
    { return weighted::interrupt(entry, frame, iar); }
    #[cfg(jnox_native_partition)]
    { return cyclic::irq(entry, frame, iar); }
    #[cfg(jnox_native_cbs)]
    if let Some(f) = unsafe { frame.as_ref() } { cbs::witness(entry, f, 0); }
    #[cfg(jnox_native_edf)]
    if NATIVE_DEADLINE_SERVICE_MASK != 0 {
        return edf::recurring::interrupt(entry, frame, iar);
    }
    if id >= 1020 { return 0 }
    #[cfg(jnox_native_threads)]
    if id == context::TIMER_IRQ {
        let ready = threads::irq(entry, unsafe { frame.as_mut() });
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
        context::gic_write(context::GICC + 0x10, iar);
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
        return ready as u64;
    }
    #[cfg(any(jnox_native_weighted, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel))]
    if id == context::TIMER_IRQ {
        #[cfg(jnox_native_weighted)]
        let next = weighted::timer(entry, unsafe { frame.as_ref() });
        #[cfg(jnox_native_edf)]
        let next = edf::irq(entry, unsafe { frame.as_ref() });
        #[cfg(jnox_native_periodic)]
        let next = periodic::irq(entry, unsafe { frame.as_ref() });
        #[cfg(jnox_native_cbs)]
        let next = cbs::irq(entry, unsafe { frame.as_ref() });
        #[cfg(jnox_native_multilevel)]
        let next = multilevel::irq(entry, unsafe { frame.as_ref() });
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
        context::gic_write(context::GICC + 0x10, iar);
        unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
        if let (Some(next), Some(interrupted)) = (next, unsafe { frame.as_mut() }) {
            activate(next, interrupted);
        }
        #[cfg(jnox_native_cbs)]
        if next.is_none() {
            if let Some(interrupted) = unsafe { frame.as_mut() } {
                unsafe { FRAMES[CURRENT] = *interrupted };
                let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
                unsafe { native_sleep_switch(top) };
                activate(cbs::owner().unwrap_or_else(|| fail(17)), interrupted);
            }
        }
        return next.is_some() as u64;
    }
    let _ = entry;
    let mut next_owner = None;
    let ready = if id != context::TIMER_IRQ {
        false
    } else if cfg!(jnox_native_fixed) {
        let now = counter();
        // Null marks an EL1 idle IRQ. The blocked owner's EL0 frame was saved
        // by the syscall and is separate from this idle continuation.
        next_owner = fixed_event(now, Action::Poll, 4, unsafe { frame.as_ref() });
        next_owner.is_some()
    } else {
        update_timer(counter())
    };
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    context::gic_write(context::GICC + 0x10, iar);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    if let (Some(next), Some(interrupted)) = (next_owner, unsafe { frame.as_mut() }) {
        activate(next, interrupted);
    }
    ready as u64
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

pub(super) fn fail(code: u64) -> ! {
    head(2, code);
    head(1, 4);
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
    #[cfg(jnox_native_weighted)]
    weighted::dump();
    uart_write(b"NATIVE EL0 FAULT ");
    decimal(code as i64);
    uart_write(b"\n");
    park()
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_unexpected() -> ! {
    fail(1)
}

#[unsafe(no_mangle)]
extern "C" fn native_el0_sync(frame: *mut NativeFrame, vector: u64, entry: u64) {
    // The lower-A64 vector created this private, 16-byte aligned EL1 frame.
    let f = unsafe { &mut *frame };
    let vbar: u64;
    let ttbr: u64;
    let far: u64;
    unsafe {
        asm!("mrs {}, vbar_el1", "mrs {}, ttbr0_el1", "mrs {}, far_el1",
            out(reg) vbar, out(reg) ttbr, out(reg) far, options(nostack, preserves_flags));
    }
    let count = unsafe { ptr::read_volatile(&raw const jnox_native_el0_journal.header[8]) as usize };
    #[cfg(jnox_native_proportional)]
    proportional::validate_sync(vector, f, vbar, ttbr, far, entry);
    if vector != 0x400 || vbar != (&raw const native_el0_vectors).addr() as u64
        || f.current_el != 4 || f.spsr & 0x1f != 0 || f.esr >> 26 != 0x15
        || f.esr & 0xffff != 0x4a01 || !native_el0_mmu::user_range(f.elr as usize, 4, false)
        || !(STACK_BOTTOM as u64..=STACK_TOP as u64).contains(&f.sp_el0) || f.sp_el0 % 16 != 0
    {
        #[cfg(jnox_native_edf)]
        if NATIVE_DEADLINE_SERVICE_MASK != 0 { edf::recurring::fault(entry, f); }
        fail(2);
    }
    #[cfg(jnox_native_edf)]
    if NATIVE_DEADLINE_SERVICE_MASK != 0 {
        edf::recurring::syscall(entry, f);
        return;
    }
    let exit = f.x[8] == 1;
    if exit && f.x[0] as i32 as i64 as u64 != f.x[0] {
        fail(3);
    }
    #[cfg(jnox_native_batch)]
    { batch::syscall(entry, f); return; }
    #[cfg(jnox_native_extension)]
    { extension::syscall(entry, f); return; }
    #[cfg(jnox_native_hierarchy)]
    { hierarchy::syscall(entry, f); return; }
    #[cfg(jnox_native_proportional)]
    { proportional::syscall(entry, f); return; }
    #[cfg(jnox_native_amc)]
    { amc::syscall(entry, f); return; }
    #[cfg(jnox_native_aperiodic)]
    { aperiodic::syscall(entry, f); return; }
    #[cfg(jnox_native_partition)]
    { cyclic::syscall(entry, f); return; }
    #[cfg(jnox_native_threads)]
    { threads::syscall(entry, f); return; }
    // Unsupported services do not dereference a user pointer, acquire
    // descriptors, emit output or claim to have completed a transfer.
    let now = if cfg!(any(jnox_native_weighted, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel)) { entry } else { counter() };
    #[cfg(jnox_native_cbs)]
    cbs::witness(now, f, f.x[8]);
    let mut deadline = None;
    let result = match f.x[8] {
        1 => 0,
        6 => clock_gettime(f.x[0], f.x[1] as usize, now),
        7 => match sleep_until(f) {
            Ok(target) if cfg!(any(jnox_native_fixed, jnox_native_weighted, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel)) && target.saturating_sub(now) > i64::MAX as u64 => -75,
            Ok(target) => { deadline = (target > now).then_some(target); 0 }
            Err(error) => error,
        },
        8 if cfg!(jnox_native_process) => 0,
        9 if cfg!(jnox_native_periodic) => 0,
        9..=19 => -95,
        _ => -38,
    };
    #[cfg(jnox_native_weighted_continuous)]
    weighted::syscall_sample(now, f, result);
    // Keep the first seven calls and the most recent call once the journal fills.
    let index = count.min(7);
    let row = [index as u64, vector, f.current_el, f.esr, f.spsr, f.elr, f.sp_el0,
        f.x[8], f.x[0], f.x[1], f.x[2], result as u64, if exit { 2 } else { 1 }, vbar, ttbr, far];
    unsafe {
        let out = (&raw mut jnox_native_el0_journal.rows).cast::<[u64; 16]>().add(index);
        ptr::write_volatile(out, row);
        asm!("dmb ishst", options(nostack, preserves_flags));
    }
    head(8, index as u64 + 1);
    let pid = unsafe { CURRENT };
    if (f.x[8] == 9 && !cfg!(jnox_native_periodic)) || (10..=19).contains(&f.x[8]) {
        f.x[0] = result as u64;
        return;
    }
    #[cfg(any(jnox_native_weighted, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel))]
    {
        let service = f.x[8];
        if exit {
            head(9, f.x[0]);
            unsafe { EXIT_STATUS[pid] = f.x[0] as i32 };
        } else { f.x[0] = result as u64; }
        #[cfg(jnox_native_weighted)]
        if service == 6 { weighted::clock(pid, now, result, f.elr); }
        #[cfg(jnox_native_periodic)]
        if result < 0 { periodic::rejected(now, pid, service, result); }
        #[cfg(jnox_native_periodic)]
        let due_poll = periodic::due(now);
        #[cfg(not(jnox_native_periodic))]
        let due_poll = false;
        let reason = if exit { 3 } else if deadline.is_some() { 2 }
            else if service == 8 && result == 0 { 1 }
            else if service == 9 && cfg!(jnox_native_periodic) { 9 }
            else if cfg!(jnox_native_weighted_continuous) && result == 0 && (service == 6 || service == 7) { service }
            else if due_poll { 5 } else { return };
        if let Some(next) = measured::boundary(now, reason, deadline, Some(f)) {
            activate(next, f);
        } else if measured::finished() {
            measured::finish();
        } else {
            unsafe { FRAMES[pid] = *f };
            let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
            unsafe { native_sleep_switch(top) };
            activate(measured::owner().unwrap_or_else(|| fail(if cfg!(jnox_native_periodic) { 16 } else if cfg!(jnox_native_edf) { 12 } else { 11 })), f);
        }
        return;
    }
    if cfg!(jnox_native_fixed) {
        let service = f.x[8];
        let action = if exit {
            let status = f.x[0] as i32;
            head(9, f.x[0]);
            unsafe { EXIT_STATUS[pid] = status };
            Action::Exit
        } else if let Some(target) = deadline {
            Action::Sleep(target - now)
        } else if service == 8 && result == 0 {
            Action::Yield
        } else {
            Action::Poll
        };
        if !exit { f.x[0] = result as u64; }
        let reason = match action { Action::Exit => 3, Action::Sleep(_) => 2, Action::Yield => 1, Action::Poll => 5 };
        if let Some(next) = fixed_event(now, action, reason, Some(f)) {
            activate(next, f);
        } else if policy().finished() {
            finish_fixed();
        } else {
            unsafe { FRAMES[pid] = *f };
            let top = (&raw mut IDLE).cast::<u8>().wrapping_add(context::STACK_BYTES);
            unsafe { native_sleep_switch(top) };
            activate(policy().current().unwrap_or_else(|| fail(9)), f);
        }
        return;
    }
    if exit {
        let status = f.x[0] as i32;
        head(9, f.x[0]);
        unsafe { STATES[pid] = TaskState::Exited(status) };
        event(b"EXIT", pid, Some(i64::from(status)));
        if COUNT == 1 {
            head(10, 1);
            head(1, 3);
            unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };
            uart_write(b"NATIVE EL0 EXIT ");
            decimal(i64::from(status));
            uart_write(b"\n");
        }
        dispatch(f);
    } else {
        f.x[0] = result as u64;
        if let Some(target) = deadline {
            unsafe { STATES[pid] = TaskState::Sleeping(target) };
            event(b"SLEEP", pid, None);
            dispatch(f);
        } else if f.x[8] == 8 && result == 0 {
            update_timer(counter());
            enqueue(pid);
            dispatch(f);
        }
    }
}

#[path = "native_stack.rs"]
mod native_stack;

// Boot scratch is kernel-owned and populated once before installing any user
// mapping. Avoid allocating three maximum-sized tails on the EL1 boot stack.
static mut STARTUP_TAILS: [[u8; native_stack::MAX_TAIL]; COUNT] = [[0; native_stack::MAX_TAIL]; COUNT];
#[unsafe(no_mangle)]
static mut jnox_native_startup_slots: [[u64; 4]; COUNT] = [[0; 4]; COUNT];

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn section_bytes<'a>(image: &'a [u8], section: &[u8]) -> Option<&'a [u8]> {
    let start = u64_at(section, 24)? as usize;
    let len = u64_at(section, 32)? as usize;
    image.get(start..start.checked_add(len)?)
}

/// The generic ELF plan covers loadable segments. Independently bind the
/// exact allocated SHT_NOTE to the native v1 SDK personality and its RO bytes.
fn native_v1_note(image: &[u8], plan: &LoadPlan<'_>) -> Option<()> {
    const NOTE: &[u8] = b"\x05\0\0\0\x04\0\0\0\x01\0\0\0JNOX\0\0\0\0\x01\0\0\0";
    let mut sizes = image.get(58..64)?.chunks_exact(2);
    let mut word = || Some(usize::from(u16::from_le_bytes(sizes.next()?.try_into().ok()?)));
    let stride = word()?;
    let count = word()?;
    let names_id = word()?;
    if stride != 64 || !(1..=128).contains(&count) || names_id >= count {
        return None;
    }
    let start = usize::try_from(u64_at(image, 40)?).ok()?;
    let table = image.get(start..start.checked_add(count * stride)?)?;
    let names_header = &table[names_id * stride..(names_id + 1) * stride];
    if u32_at(names_header, 4)? != 3 {
        return None;
    }
    let names = section_bytes(image, names_header)?;
    let mut found = false;
    for sh in table.chunks_exact(stride) {
        let tail = names.get(u32_at(sh, 0)? as usize..)?;
        let name = tail.get(..tail.iter().position(|&b| b == 0)?)?;
        if name != b".note.jnox.abi" {
            continue;
        }
        if found || u32_at(sh, 4)? != 7 || u64_at(sh, 8)? != 2 || section_bytes(image, sh)? != NOTE {
            return None;
        }
        let va = u64_at(sh, 16)?;
        let file = u64_at(sh, 24)?;
        found = plan.segments().iter().any(|s| {
            let Some(delta) = va.checked_sub(s.address) else { return false };
            s.permission == Permission::Read && s.file_offset.checked_add(delta) == Some(file)
                && delta.checked_add(NOTE.len() as u64).is_some_and(|end| end <= s.bytes.len() as u64)
        });
        if !found {
            return None;
        }
    }
    found.then_some(())
}

fn validated_image(image: &[u8]) -> LoadPlan<'_> {
    let plan = native_elf::plan(image, 0x0040_0000..0x0080_0000)
        .unwrap_or_else(|_| fail(4));
    let perms = [Permission::ReadExecute, Permission::Read, Permission::ReadWrite];
    if plan.entry != 0x0040_0000 || !plan.segments().iter().map(|s| s.permission).eq(perms)
        || native_v1_note(image, &plan).is_none()
    {
        fail(5);
    }
    plan
}

pub(super) fn run() -> ! {
    let frequency: u64;
    unsafe {
        asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags));
        if frequency == 0 {
            fail(8);
        }
        CLOCK_DOMAIN = (counter(), frequency);
    }
    #[cfg(jnox_pi4)]
    {
        uart_write(b"NATIVE PI4 CNTFRQ ");
        unsigned(frequency);
        uart_write(b"\n");
    }
    head(0, 1);
    head(1, 1);
    head(3, jnox_native_el0_image.len() as u64);
    let images: [(&[u8], &str); COUNT] = [
        (&jnox_native_el0_image, env!("JNOX_PACKAGED_ELF_SHA256")),
        #[cfg(jnox_native_peer)]
        (PEER, env!("JNOX_PACKAGED_PEER_SHA256")),
        #[cfg(jnox_native_peer2)]
        (PEER2, env!("JNOX_PACKAGED_PEER2_SHA256")),
        #[cfg(all(jnox_native_amc, jnox_native_peer3))]
        (PEER3, env!("JNOX_PACKAGED_PEER3_SHA256")),
    ];
    for (pid, (_, digest)) in images.iter().enumerate() {
        uart_write(match pid { 0 => b"NATIVE EL0 ELF ", 1 => b"NATIVE EL0 PEER ELF ", 2 => b"NATIVE EL0 PEER2 ELF ", _ => b"NATIVE EL0 PEER3 ELF " });
        uart_write(digest.as_bytes());
        uart_write(b"\n");
    }
    let plans = images.map(|(image, _)| validated_image(image));
    let buffers = unsafe { &mut *(&raw mut STARTUP_TAILS) };
    #[cfg(jnox_native_threads)]
    let stack_bottom = native_el0_mmu::thread_stack(0).unwrap_or_else(|| fail(20)).0;
    #[cfg(not(jnox_native_threads))]
    let stack_bottom = STACK_BOTTOM;
    let layouts: [native_stack::Layout; COUNT] = core::array::from_fn(|pid| {
        native_stack::build(NATIVE_ARGV_EXTRAS[pid], !cfg!(jnox_native_process),
            stack_bottom, STACK_TOP, &mut buffers[pid]).unwrap_or_else(|| fail(19))
    });
    let tails: [&[u8]; COUNT] = core::array::from_fn(|pid| &buffers[pid][..layouts[pid].len]);
    // The board-specific PPI setup owns the distributor and CPU interface.
    // On Pi it must preserve firmware configuration for unrelated interrupts.
    unsafe {
        asm!("msr daifset, #0xf", "msr cntp_ctl_el0, xzr", "msr cntv_ctl_el0, xzr",
            "isb", options(nostack, preserves_flags));
        asm!("dsb sy", "isb", options(nostack, preserves_flags));
        context::configure_timer_ppi();
        asm!("dsb sy", "isb", options(nostack, preserves_flags));
        native_el0_mmu::install_processes(&plans, &tails).unwrap_or_else(|_| fail(6));
        for (pid, plan) in plans.iter().enumerate() {
            FRAMES[pid].elr = plan.entry;
            let layout = layouts[pid];
            FRAMES[pid].sp_el0 = layout.sp as u64;
            ptr::write_volatile(&raw mut jnox_native_startup_slots[pid],
                [plan.entry, layout.sp as u64, layout.len as u64, layout.argc as u64]);
            if !cfg!(any(jnox_native_amc, jnox_native_batch, jnox_native_proportional, jnox_native_extension, jnox_native_hierarchy, jnox_native_aperiodic, jnox_native_partition, jnox_native_fixed, jnox_native_weighted, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel)) {
                if pid == 0 { STATES[pid] = TaskState::Running; } else { enqueue(pid); }
            }
        }
    }
    let current: u64;
    let sctlr: u64;
    let ttbr: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", "mrs {}, sctlr_el1", "mrs {}, ttbr0_el1",
            out(reg) current, out(reg) sctlr, out(reg) ttbr, options(nostack, preserves_flags));
    }
    if current != 4 || sctlr & 1 == 0 || !native_el0_mmu::user_range(layouts[0].sp, layouts[0].len, true) {
        fail(7);
    }
    head(4, plans[0].entry);
    head(5, layouts[0].sp as u64);
    head(6, stack_bottom as u64);
    head(7, STACK_TOP as u64);
    head(12, current);
    head(13, sctlr);
    head(14, ttbr);
    head(15, layouts[0].argc as u64);
    head(1, 2);
    #[cfg(jnox_native_weighted)]
    let first = weighted::start();
    #[cfg(jnox_native_edf)]
    let first = edf::start(frequency);
    #[cfg(jnox_native_periodic)]
    let first = periodic::start(frequency);
    #[cfg(jnox_native_cbs)]
    let first = cbs::start(frequency);
    #[cfg(jnox_native_multilevel)]
    let first = multilevel::start(frequency);
    #[cfg(jnox_native_batch)]
    let first = batch::start(frequency);
    #[cfg(jnox_native_extension)]
    let first = extension::start(frequency);
    #[cfg(jnox_native_hierarchy)]
    let first = hierarchy::start(frequency);
    #[cfg(jnox_native_proportional)]
    let first = proportional::start(frequency);
    #[cfg(jnox_native_amc)]
    let first = amc::start(frequency, images.map(|(elf,_)|elf));
    #[cfg(jnox_native_aperiodic)]
    let first = aperiodic::start(frequency);
    #[cfg(jnox_native_partition)]
    let first = cyclic::start(frequency);
    #[cfg(not(any(jnox_native_amc, jnox_native_batch, jnox_native_proportional, jnox_native_extension, jnox_native_hierarchy, jnox_native_aperiodic, jnox_native_partition, jnox_native_weighted, jnox_native_edf, jnox_native_periodic, jnox_native_cbs, jnox_native_multilevel)))]
    let first = if cfg!(jnox_native_fixed) {
        start_fixed()
    } else {
        event(b"DISPATCH", 0, None);
        0
    };
    unsafe {
        if first != 0 { native_el0_mmu::switch(first).unwrap_or_else(|_| fail(9)); }
        CURRENT = first;
        #[cfg(jnox_native_batch)]
        {
            unsafe extern "C" { fn native_batch_enter(frame: *const NativeFrame) -> !; }
            native_batch_enter(&raw const FRAMES[first]);
        }
        #[cfg(any(jnox_native_amc, jnox_native_extension, jnox_native_aperiodic))]
        {
            unsafe extern "C" { fn native_server_enter(frame: *const NativeFrame) -> !; }
            native_server_enter(&raw const FRAMES[first])
        }
        #[cfg(jnox_native_threads)]
        let locator = threads::start(FRAMES[first]);
        #[cfg(not(jnox_native_threads))]
        let locator = 0;
        native_el0_enter(FRAMES[first].elr, FRAMES[first].sp_el0, INITIAL_PSTATE, locator)
    }
}
