//! Single-core, opt-in startup diagnostics. Boot code owns B. The diagnostic mode is
//! a named read-only file in the QEMU 6.2 virt firmware configuration device.
use super::*;
use core::sync::atomic::{AtomicBool, Ordering};

static PROBE_DONE: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
static mut jnox_cold_b: u64 = 0;
// QEMU 6.2 virt maps the fw_cfg data port at base and selector at base+8.
fn fw_select(key: u16) {
    unsafe {
        ptr::write_volatile(0x0902_0008 as *mut u16, key.to_be());
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}

fn fw_bytes<const N: usize>() -> [u8; N] {
    core::array::from_fn(|_| unsafe { ptr::read_volatile(0x0902_0000 as *const u8) })
}

fn configured_mode() -> Option<u32> {
    const NAME: &[u8] = b"opt/jnox/cold-mode\0";
    fw_select(0);
    if fw_bytes::<4>() != *b"QEMU" { return None }
    fw_select(0x19);
    let entries = u32::from_be_bytes(fw_bytes());
    if entries > 256 { return None }

    let mut selected = None;
    for _ in 0..entries {
        let size = u32::from_be_bytes(fw_bytes());
        let key = u16::from_be_bytes(fw_bytes());
        let _reserved = fw_bytes::<2>();
        let name = fw_bytes::<56>();
        if name.starts_with(NAME) {
            if size != 1 || !(0x20..0x4000).contains(&key) || selected.is_some() {
                return None;
            }
            selected = Some(key);
        }
    }
    fw_select(selected?);
    match fw_bytes::<1>()[0] {
        value @ b'1'..=b'3' => Some(u32::from(value - b'0')),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase { Setup, ProbeArmed, ProbeDone, FirstArmed, Running }

struct Boot {
    mode: u32,
    probe: u64,
    cutoff: u64,
    epoch: u64,
    phase: Phase,
}
static mut BOOT: Boot = Boot { mode: 0, probe: 0, cutoff: 0, epoch: 0, phase: Phase::Setup };

fn data() -> &'static mut Boot {
    // Access only before first unmask or in an IRQ-masked section.
    unsafe { &mut *(&raw mut BOOT) }
}

fn mask() -> u64 {
    let value: u64;
    unsafe { asm!("mrs {},daif", out(reg)value, options(nostack, preserves_flags)) };
    value
}

fn timer() -> (u64, u64) {
    let (compare, ctl): (u64, u64);
    unsafe { asm!("isb", "mrs {},cntp_cval_el0", "mrs {},cntp_ctl_el0",
        out(reg)compare, out(reg)ctl, options(nostack, preserves_flags)) };
    (compare, ctl)
}

fn lines() -> u64 {
    let bit = 1u32 << TIMER_IRQ;
    u64::from(gic_read(GICD + 0x200) & bit != 0)
        | (u64::from(gic_read(GICD + 0x300) & bit != 0) << 1)
}

fn stop(reason: u64) -> ! {
    unsafe { asm!("msr daifset,#2", "msr cntp_ctl_el0,xzr", "isb", options(nostack)) };
    app::write_output(false);
    log(60, now(), reason, app::read_output() as u64);
    dump()
}

pub(super) fn boot(frequency: u64) {
    let base = unsafe { ptr::read_volatile(&raw const jnox_cold_b) };
    let mode = configured_mode().unwrap_or(u32::MAX);
    log(48, base, frequency, mode as u64);
    if !(1..=3).contains(&mode) || frequency == 0 || frequency % 1000 != 0 || frequency % 400 != 0 {
        stop(1)
    }
    let ms = frequency / 1000;
    let target = |offset: u64| ms.checked_mul(offset).and_then(|v| base.checked_add(v))
        .unwrap_or_else(|| stop(2));
    let start = now();
    *data() = Boot { mode, probe: target(200), cutoff: target(220),
        epoch: target(250), phase: Phase::Setup };
    log(49, start, now(), data().epoch);
}

pub(super) fn initial_off() {
    let start = now();
    let observed = app::read_output();
    log(64, start, now(), observed as u64);
    if observed { stop(13) }
}

fn program(target: u64, probe: bool) {
    let start = now();
    if probe {
        unsafe { asm!("msr cntp_cval_el0,{}", "msr cntp_ctl_el0,{}", "isb",
            in(reg)target, in(reg)1u64, options(nostack)) };
    } else {
        arm(target, 0);
    }
    let (compare, ctl) = timer();
    let end = now();
    log(51, start, end, target);
    log(52, end, compare, ctl);
    if compare != target || ctl & 7 != 1 || end >= target {
        stop(3)
    }
}

fn observe_idle(at: u64, flags: u64) {
    let enabled = (gic_read(GICD + 0x100) >> TIMER_IRQ) & 1;
    let prio = unsafe { ptr::read_volatile((GICD + 0x400 + TIMER_IRQ as usize) as *const u8) };
    let pmr = gic_read(GICC + 4) & 255;
    log(53, at, flags, lines());
    log(54, now(), u64::from(enabled) | (u64::from(prio) << 8), pmr as u64);
    let distributor = gic_read(GICD);
    let interface = gic_read(GICC);
    log(62, now(), u64::from(distributor), u64::from(interface));
    if distributor & 1 == 0 || interface & 1 == 0 || flags & 0x80 != 0 || enabled != 1 || u32::from(prio) >= pmr || app::read_output() {
        stop(4)
    }
}

pub(super) fn idle_start() {
    let entered = now();
    let daif = mask();
    locked(|| {
        observe_idle(entered, daif);
        if data().mode == 3 {
            let probe = data().probe;
            data().phase = Phase::ProbeArmed;
            program(probe, true);
        }
    });
    let after_arm = now();
    let flags = mask();
    locked(|| log(61, after_arm, flags, 1));
    let (mode, probe, cutoff) = locked(|| (data().mode, data().probe, data().cutoff));
    if mode != 1 {
        // Poll the independent cutoff even when the PPI never arrives. Both
        // matched modes wait on idle's stack with IRQ enabled between samples.
        loop {
            let tick = now();
            if tick >= cutoff || app::read_output() {
                stop(5)
            }
            let ready = if mode == 2 { tick >= probe }
                else { PROBE_DONE.load(Ordering::Acquire) };
            if ready { break }
            core::hint::spin_loop();
        }
    }
    locked(|| {
        let s = state();
        log(43, now(), s.frequency, s.step);
        let begin = now();
        s.epoch = if data().mode == 1 { begin + s.step } else { data().epoch };
        data().epoch = s.epoch;
        log(50, begin, now(), s.epoch);
        log(1, s.epoch, s.frequency, LAST);
        if lines() != 0 || app::read_output() { stop(6) }
        data().phase = Phase::FirstArmed;
        program(s.epoch, false);
        let ready = now();
        if data().mode != 1 && ready >= data().cutoff { stop(7) }
        log(58, ready, s.epoch, 0);
    });
    let after = now();
    let flags = mask();
    locked(|| log(61, after, flags, 0));
}

pub(super) fn on_irq(frame: *mut Frame, entry: u64, handler: u64, iar: u32) -> bool {
    if data().phase == Phase::Running { return false }
    let id = iar & 0x3ff;
    let (compare, ctl) = timer();
    log(55, entry, handler, unsafe { (*frame).elr });
    log(63, now(), compare, ctl);
    log(56, now(), u64::from(iar), requested_mask(frame));
    if id != TIMER_IRQ || requested_mask(frame) & 0x80 != 0 || ctl & 7 != 5 {
        stop(8)
    }
    match data().phase {
        Phase::ProbeArmed => {
            if entry < data().probe || compare != data().probe || now() >= data().cutoff {
                stop(9)
            }
            unsafe { asm!("msr cntp_ctl_el0,xzr", "isb", "dsb sy", options(nostack)) };
            gic_write(GICC + 0x10, iar);
            unsafe { asm!("dsb sy", "isb", options(nostack)) };
            let post = timer().1;
            let irq_state = lines();
            let done = now();
            log(57, done, post, irq_state);
            if post & 3 != 0 || irq_state != 0 || done >= data().cutoff || app::read_output() {
                stop(10)
            }
            data().phase = Phase::ProbeDone;
            PROBE_DONE.store(true, Ordering::Release);
            true
        }
        Phase::FirstArmed => {
            if entry < data().epoch || compare != data().epoch || !app::begin_operational() {
                stop(11)
            }
            data().phase = Phase::Running;
            log(59, now(), data().epoch, 0);
            false
        }
        _ => stop(12),
    }
}
