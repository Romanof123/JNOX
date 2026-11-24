// Keep the default expressions and their call-site tokens in the ordinary module.
#[cfg(not(any(jnox_reservation_stage = "eoi", jnox_reservation_stage = "frames", jnox_reservation_stage = "frame-window")))]
macro_rules! terminal {
    ($original:expr) => { $original };
}
#[cfg(any(jnox_reservation_stage = "eoi", jnox_reservation_stage = "frames"))]
macro_rules! terminal {
    ($original:expr) => {{
        $crate::ordinary_reservations::stage_hooks::stage_probe::dump();
        $original
    }};
}
#[cfg(jnox_reservation_stage = "frame-window")]
macro_rules! terminal {
    ($original:expr) => {{
        $crate::ordinary_reservations::stage_hooks::frame_window::dump();
        $original
    }};
}
pub(super) use terminal;

#[cfg(not(any(jnox_reservation_stage = "eoi", jnox_reservation_stage = "frames", jnox_reservation_stage = "frame-window")))]
macro_rules! header {
    ($original:literal) => { $original };
}
#[cfg(any(jnox_reservation_stage = "eoi", jnox_reservation_stage = "frames"))]
macro_rules! header {
    ($original:literal) => { b"RSRV RAW 4\n" };
}
#[cfg(jnox_reservation_stage = "frame-window")]
macro_rules! header {
    ($original:literal) => { b"RSRV RAW 5\n" };
}
pub(super) use header;

#[cfg(not(jnox_reservation_stage = "frame-window"))]
macro_rules! validation_window {
    ($epoch:expr, $checks:expr) => { $checks };
}
#[cfg(jnox_reservation_stage = "frame-window")]
macro_rules! validation_window {
    ($epoch:expr, $checks:expr) => {{
        use $crate::ordinary_reservations::stage_hooks::frame_window;
        let before = frame_window::sample();
        $checks;
        let after = frame_window::sample();
        frame_window::record($epoch, before, after);
    }};
}
pub(super) use validation_window;

#[cfg(jnox_reservation_stage = "frame-window")]
pub(super) mod frame_window {
    use super::super::{asm, event, IDLE};

    // One slot-zero IRQ on one CPU. Only a completed pair is published.
    type Sample = [u64; 2];
    static mut PAIR: Option<(u64, Sample, Sample)> = None;

    #[inline(always)]
    pub(in crate::ordinary_reservations) fn sample() -> Sample {
        let tick;
        let pc;
        unsafe {
            // Also a compiler memory barrier for the live validation loads.
            asm!("isb", "2: mrs {tick}, cntpct_el0", "adr {pc}, 2b",
                tick = out(reg) tick, pc = out(reg) pc, options(nostack, preserves_flags));
        }
        [tick, pc]
    }

    pub(in crate::ordinary_reservations) fn record(epoch: u64, before: Sample, after: Sample) {
        unsafe { (&raw mut PAIR).write_volatile(Some((epoch, before, after))) }
    }

    pub(in crate::ordinary_reservations) fn dump() {
        let pair = unsafe { (&raw const PAIR).read_volatile() };
        let present = u64::from(pair.is_some());
        let (epoch, before, after) = pair.unwrap_or_default();
        for (tag, [tick, pc]) in [(39, before), (40, after)] {
            event(tag, tick, 0, IDLE, [2, present, pc, epoch]);
        }
    }
}

#[cfg(not(jnox_reservation_stage = "frames"))]
macro_rules! after_frames {
    ($original:expr) => { $original };
}
#[cfg(jnox_reservation_stage = "frames")]
macro_rules! after_frames {
    ($original:expr) => {{
        $crate::ordinary_reservations::stage_hooks::stage_probe::sample();
        $original
    }};
}
pub(super) use after_frames;

#[cfg(not(jnox_reservation_stage = "eoi"))]
macro_rules! eoi_barrier {
    ($slot:expr, $kind:expr, $barrier:expr) => { $barrier };
}
#[cfg(jnox_reservation_stage = "eoi")]
macro_rules! eoi_barrier {
    ($slot:expr, $kind:expr, $barrier:expr) => {{
        if $slot == 0 && $kind == 0 {
            $crate::ordinary_reservations::stage_hooks::stage_probe::sample();
        } else {
            $barrier;
        }
    }};
}
pub(super) use eoi_barrier;

#[cfg(any(jnox_reservation_stage = "eoi", jnox_reservation_stage = "frames"))]
pub(super) mod stage_probe {
    use super::super::{asm, event, IDLE};

    // Single core. IRQ records once, masked terminal reads once.
    static mut OBSERVATION: Option<(u64, u64)> = None;

    #[inline(always)]
    pub(in crate::ordinary_reservations) fn sample() {
        let tick: u64;
        let pc: u64;
        unsafe {
            // E owns the sampled path's final EOI completion barrier.
            #[cfg(jnox_reservation_stage = "eoi")]
            asm!(
                "dsb sy", "isb", "2: mrs {tick}, cntpct_el0", "adr {pc}, 2b",
                tick = out(reg) tick, pc = out(reg) pc, options(nostack, preserves_flags)
            );
            #[cfg(jnox_reservation_stage = "frames")]
            asm!(
                "isb", "2: mrs {tick}, cntpct_el0", "adr {pc}, 2b",
                tick = out(reg) tick, pc = out(reg) pc, options(nostack, preserves_flags)
            );
            (&raw mut OBSERVATION).write_volatile(Some((tick, pc)));
        }
    }

    pub(in crate::ordinary_reservations) fn dump() {
        let observation = unsafe { (&raw const OBSERVATION).read_volatile() };
        let (tick, pc) = observation.unwrap_or_default();
        let site = if cfg!(jnox_reservation_stage = "eoi") { 1 } else { 2 };
        let selector = if cfg!(jnox_reservation_policy = "aligned") { 1 } else { 4 };
        event(38, tick, 0, IDLE, [site, selector, u64::from(observation.is_some()), pc]);
    }
}
