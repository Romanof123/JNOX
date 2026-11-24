//! Single-CPU masked exception ownership. Assembly shares the fixed u64 layout.
use super::*;
#[path = "mutex_anchor_policy.rs"]
mod policy;

#[repr(C)]
struct Witness {
    phase: u64,
    generation: u64,
    ordinal: u64,
    frame: u64,
    pc: u64,
    spsr: u64,
    tls: u64,
    x0: u64,
    resource: u64,
    ticks: u64,
    start: u64,
    target: u64,
    cval: u64,
    ctl: u64,
    write_end: u64,
    read_end: u64,
    restore_entry: u64,
    before_eret: u64,
    valid: u64,
    restore_sample: u64,
}
const _: () = {
    assert!(size_of::<Witness>() == 160);
    assert!(offset_of!(Witness, generation) == 8 && offset_of!(Witness, ordinal) == 16);
    assert!(offset_of!(Witness, frame) == 24 && offset_of!(Witness, pc) == 32);
    assert!(offset_of!(Witness, spsr) == 40 && offset_of!(Witness, tls) == 48);
    assert!(offset_of!(Witness, x0) == 56 && offset_of!(Witness, ticks) == 72);
    assert!(offset_of!(Witness, start) == 80 && offset_of!(Witness, target) == 88);
    assert!(offset_of!(Witness, cval) == 96 && offset_of!(Witness, ctl) == 104);
    assert!(offset_of!(Witness, restore_entry) == 128 && offset_of!(Witness, before_eret) == 136);
    assert!(offset_of!(Witness, valid) == 144);
};
#[unsafe(export_name = "jnox_mutex_anchor")]
static mut WITNESS: Witness = unsafe { core::mem::zeroed() }; // integer fields only
const MODE: u64 = if cfg!(jnox_mutex_return) {
    3
} else if cfg!(jnox_mutex_stages) {
    2
} else {
    1
};

pub(super) fn initialize(frequency: u64) {
    unsafe {
        TIMER[0] = frequency;
    }
    let ticks = policy::exact_ticks(frequency).unwrap_or_else(|| stop(13));
    unsafe {
        WITNESS.ticks = ticks;
    }
}

pub(super) fn prepare(
    r: &Runtime,
    caller: usize,
    resource: usize,
    outgoing: *mut Frame,
    selected: *mut Frame,
    result: u64,
) {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    if daif & 0x3c0 != 0x3c0
        || caller != LOW
        || r.current != LOW
        || outgoing != selected
        || !owned_frame(LOW, selected)
        || resource != (if CEILING { Y } else { X })
        || r.locks.owner(resource) != Some(LOW)
        || r.locks.owner(X) != Some(LOW)
        || result != 0
        || r.released
        || r.armed
    {
        stop(19);
    }
    unsafe {
        let f = &*selected;
        let frame = selected.addr() as u64;
        let generation = USED as u64;
        let prior = RETURNS[0];
        if WITNESS.phase != 0
            || WITNESS.ticks == 0
            || prior >= 64
            || generation != prior
            || !(1..=ROWS as u64).contains(&generation)
            || !policy::valid_identity(frame, f.elr, f.spsr, f.tpidr_el1, f.gpr[0])
        {
            stop(19);
        }
        WITNESS.generation = generation;
        WITNESS.ordinal = prior + 1;
        WITNESS.frame = frame;
        WITNESS.pc = f.elr;
        WITNESS.spsr = f.spsr;
        WITNESS.tls = f.tpidr_el1;
        WITNESS.x0 = f.gpr[0];
        WITNESS.resource = resource as u64;
        WITNESS.valid = 1;
        WITNESS.phase = 1;
    }
}

#[cfg(not(jnox_mutex_return))]
pub(super) fn arm_in_rust() {
    unsafe {
        let w = &mut *(&raw mut WITNESS);
        w.phase = 2;
        let ticks = w.ticks;
        let start = now();
        let Some(target) = start.checked_add(ticks) else {
            w.start = start;
            w.valid |= 2;
            stop(21);
        };
        let (cval, ctl): (u64, u64);
        #[cfg(not(jnox_mutex_stages))]
        asm!("msr cntp_cval_el0, {t}", "msr cntp_ctl_el0, {en}", "isb",
             "mrs {cv}, cntp_cval_el0", "mrs {ct}, cntp_ctl_el0",
             t=in(reg)target, en=in(reg)1u64, cv=out(reg)cval, ct=out(reg)ctl,
             options(nostack, preserves_flags));
        #[cfg(jnox_mutex_stages)]
        {
            let (write_end, read_end): (u64, u64);
            asm!("msr cntp_cval_el0, {t}", "msr cntp_ctl_el0, {en}", "isb",
                 "mrs {we}, cntpct_el0", "mrs {cv}, cntp_cval_el0",
                 "mrs {ct}, cntp_ctl_el0", "isb", "mrs {re}, cntpct_el0",
                 t=in(reg)target, en=in(reg)1u64, we=out(reg)write_end,
                 cv=out(reg)cval, ct=out(reg)ctl, re=out(reg)read_end,
                 options(nostack, preserves_flags));
            w.write_end = write_end;
            w.read_end = read_end;
        }
        w.start = start;
        w.target = target;
        w.cval = cval;
        w.ctl = ctl;
        w.valid = if cfg!(jnox_mutex_stages) { 63 } else { 15 };
        if !policy::valid_readback(target, cval, ctl) {
            stop(20);
        }
        w.phase = 3;
    }
}

// The generation names exactly one E row. Its designated R row follows the
// scheduling decision at the next restore. Never search for a similar frame.
pub(super) fn correlate() -> Option<u64> {
    unsafe {
        let w = &mut *(&raw mut WITNESS);
        if w.phase != 3
            || w.generation == 0
            || w.generation > USED as u64
            || w.resource != (if CEILING { Y as u64 } else { X as u64 })
            || w.ordinal != w.generation + 1
            || w.ordinal > RETURNS[0]
            || w.ordinal > 64
        {
            return None;
        }
        let e = &*(&raw const JOURNAL[w.generation as usize - 1]);
        let i = 1 + 3 * (w.ordinal as usize - 1);
        if e[2..6] != [LOCK, w.resource, 0, 0]
            || e[6..9] != [w.frame, w.pc, w.spsr]
            || e[10..12] != [w.frame, w.pc]
            || e[14] != 0
            || e[14 + w.resource as usize] != 0
            || e[19] != 0
            || e[23..26] != [w.tls, w.tls, w.x0]
            || !policy::valid_identity(w.frame, w.pc, w.spsr, w.tls, w.x0)
            || RETURNS[i] != w.frame
            || RETURNS[i + 1] != w.pc
            || RETURNS[i + 2] < e[1]
        {
            return None;
        }
        let restore = RETURNS[i + 2];
        let required = match MODE {
            1 => 15,
            2 => 127,
            _ => 143,
        };
        if w.valid != required
            || w.start.checked_add(w.ticks) != Some(w.target)
            || !policy::valid_readback(w.target, w.cval, w.ctl)
            || (MODE < 3 && !(e[1] <= w.start && w.start <= restore))
            || (MODE == 2
                && !(w.start <= w.write_end
                    && w.write_end <= w.read_end
                    && w.read_end <= w.restore_entry
                    && w.restore_entry <= restore))
            || (MODE == 3 && !(restore <= w.start && w.start <= w.before_eret))
        {
            return None;
        }
        w.restore_sample = restore;
        w.valid |= 256;
        w.phase = 4;
        Some(w.target)
    }
}

pub(super) fn dump() {
    unsafe {
        // On a terminal path before IRQ, preserve a complete return if present.
        if WITNESS.phase == 3 {
            let _ = correlate();
        }
        let w = &*(&raw const WITNESS);
        line(
            b"T",
            &[
                TIMER[0], w.start, w.target, w.cval, w.ctl, TIMER[5], TIMER[6], TIMER[7],
            ],
        );
        let elapsed = if w.valid & 128 == 0 {
            NA
        } else {
            u64::from(w.before_eret >= w.target)
        };
        line(
            b"W",
            &[
                1,
                MODE,
                w.phase,
                w.valid,
                w.generation,
                w.ordinal,
                w.frame,
                w.pc,
                w.spsr,
                w.tls,
                w.x0,
                w.resource,
                w.ticks,
            ],
        );
        line(
            b"P",
            &[
                w.write_end,
                w.read_end,
                w.restore_entry,
                w.restore_sample,
                w.before_eret,
                elapsed,
            ],
        );
    }
}

#[cfg(jnox_mutex_return)]
#[unsafe(no_mangle)]
extern "C" fn jnox_mutex_arm_stop(reason: u64) -> ! {
    stop(reason)
}
