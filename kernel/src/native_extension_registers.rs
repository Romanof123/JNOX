use transaction::Transaction;

// Keep the disabled profile's direct calls and its source-location records.
macro_rules! extension_arm {
    ($runtime:expr, $target:expr) => {{
        #[cfg(jnox_native_extension_register_observations)]
        registers::arm($runtime, $target);
        #[cfg(not(jnox_native_extension_register_observations))]
        arm_compare($target);
    }};
}

macro_rules! before_irq_event {
    ($iar:expr, $frame:expr) => {{
        before_event();
        #[cfg(jnox_native_extension_register_observations)]
        registers::entry(state(), $iar, $frame.is_null());
    }};
}

#[cfg(jnox_native_extension_register_observations)]
mod registers {
    use super::*;

    // Masked CPU0 is the only writer. The vector fills scratch before either
    // stack switching or frame construction; IRQs stay masked until finalized.
    #[unsafe(no_mangle)]
    static mut jnox_extension_reg_generation: u64 = 0;
    #[unsafe(no_mangle)]
    static mut jnox_extension_reg_scratch: [u64; 5] = [0; 5];
    #[unsafe(no_mangle)]
    static mut jnox_extension_reg_arms: [[u64; 9]; CAP] = [[0; 9]; CAP];
    #[unsafe(no_mangle)]
    static mut jnox_extension_reg_entries: [[u64; 11]; CAP] = [[0; 11]; CAP];
    static mut ENTRY_COUNT: u64 = 0;

    fn ordinal(r: &Runtime) -> u64 {
        (r.used as u64).saturating_add(r.dropped)
    }

    // Keep both architectural readbacks immediately after the final ISB.
    // out(reg) prevents the before stamp from aliasing any still-live input.
    #[inline(never)]
    #[unsafe(no_mangle)]
    fn jnox_extension_register_arm(target: Option<u64>) -> [u64; 4] {
        let (before, cval, ctl, after): (u64, u64, u64, u64);
        unsafe {
            if let Some(target) = target {
                asm!("isb", "mrs {before}, cntpct_el0",
                     "msr cntp_cval_el0, {target}", "msr cntp_ctl_el0, {enable}", "isb",
                     "mrs {cval}, cntp_cval_el0", "mrs {ctl}, cntp_ctl_el0",
                     "isb", "mrs {after}, cntpct_el0",
                     target = in(reg) target, enable = in(reg) 1u64,
                     before = out(reg) before, cval = out(reg) cval,
                     ctl = out(reg) ctl, after = out(reg) after, options(nostack, preserves_flags));
            } else {
                asm!("isb", "mrs {before}, cntpct_el0", "msr cntp_ctl_el0, xzr", "isb",
                     "mrs {cval}, cntp_cval_el0", "mrs {ctl}, cntp_ctl_el0",
                     "isb", "mrs {after}, cntpct_el0",
                     before = out(reg) before, cval = out(reg) cval,
                     ctl = out(reg) ctl, after = out(reg) after, options(nostack, preserves_flags));
            }
        }
        [before, cval, ctl, after]
    }

    pub(super) fn arm(r: &Runtime, target: Option<u64>) {
        let [before, cval, ctl, after] = jnox_extension_register_arm(target);
        let kind = match target {
            None => 16,
            Some(at) => {
                let (deadline, reason) = if r.tx.cell.is_none() { (r.epoch, 1) }
                    else if r.tx.evaluating { (r.tx.cutoff, 2) } else { (r.end, 4) };
                let wake = r.sleeps.iter().flatten().any(|&t| t == at);
                (if at == deadline { reason } else { 0 }) | (u64::from(wake) << 3)
            }
        };
        unsafe {
            let generation = (&raw const jnox_extension_reg_generation).read_volatile().saturating_add(1);
            (&raw mut jnox_extension_reg_generation).write_volatile(generation);
            if generation <= CAP as u64 {
                let event = if generation == 1 { NONE } else { ordinal(r) };
                jnox_extension_reg_arms[generation as usize - 1] =
                    [generation, event, r.tx.cell.unwrap_or(NONE), kind, target.unwrap_or(0),
                     before, cval, ctl, after];
            }
        }
    }

    pub(super) fn entry(r: &Runtime, iar: u32, from_el1: bool) {
        unsafe {
            let [prefix, spsr, elr, sp, generation] = (&raw const jnox_extension_reg_scratch).read_volatile();
            ENTRY_COUNT = ENTRY_COUNT.saturating_add(1);
            if ENTRY_COUNT <= CAP as u64 {
                let base = (&raw const EVALUATOR).addr() as u64;
                let class = if !from_el1 { 1 }
                    else if sp > base && sp <= base + context::STACK_BYTES as u64 { 2 } else { 3 };
                jnox_extension_reg_entries[ENTRY_COUNT as usize - 1] =
                    [ENTRY_COUNT, ordinal(r), generation, r.tx.cell.unwrap_or(NONE), class,
                     u64::from(iar), r.compare, prefix, spsr, elr, sp];
            }
        }
    }

    fn line(tag: &[u8], words: &[u64]) {
        uart_write(b"NATIVE REG ");
        uart_write(tag);
        for &word in words {
            uart_write(b" ");
            unsigned(word);
        }
        uart_write(b"\n");
    }

    pub(super) fn emit() {
        unsafe {
            let arms = jnox_extension_reg_generation;
            let entries = ENTRY_COUNT;
            let a = arms.min(CAP as u64);
            let e = entries.min(CAP as u64);
            line(b"CONFIG", &[1, CAP as u64]);
            for i in 0..a as usize {
                line(b"ARM", &*(&raw const jnox_extension_reg_arms[i]));
            }
            for i in 0..e as usize {
                line(b"ENTRY", &*(&raw const jnox_extension_reg_entries[i]));
            }
            line(b"END", &[arms, a, arms-a, entries, e, entries-e]);
        }
    }
}
