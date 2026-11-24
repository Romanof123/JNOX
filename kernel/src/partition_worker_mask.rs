//! Private, one-shot EL1 fixture for partition A. The worker owns the record.
use super::{counter, fail, gic_read, GICD, TIMER_IRQ, Uart};
use core::{arch::asm, fmt::Write, ptr, sync::atomic::{AtomicU64, Ordering::Relaxed}};

static DUE: AtomicU64 = AtomicU64::new(0);
static TARGET: AtomicU64 = AtomicU64::new(0);

#[cfg_attr(all(feature = "ordinary-partition-p", jnox_partition_mask_long), unsafe(export_name = "ordinary_mask_p_long"))]
#[cfg_attr(all(feature = "ordinary-partition-p", not(jnox_partition_mask_long)), unsafe(export_name = "ordinary_mask_p_short"))]
#[cfg_attr(all(feature = "ordinary-partition-q", jnox_partition_mask_long), unsafe(export_name = "ordinary_mask_q_long"))]
#[cfg_attr(all(feature = "ordinary-partition-q", not(jnox_partition_mask_long)), unsafe(export_name = "ordinary_mask_q_short"))]
#[inline(never)]
pub(super) fn boot() {
    let _ = writeln!(Uart, "OP MASKBOOT table={} mode={}",
        if super::Q_TABLE { "Q" } else { "P" },
        if cfg!(jnox_partition_mask_long) { "long" } else { "short" });
}

pub(super) fn configure(epoch: u64, quantum: u64, frequency: u64) {
    let close = if super::Q_TABLE { 4 } else { 2 };
    let due = quantum.checked_mul(close).and_then(|v| epoch.checked_add(v)).unwrap_or_else(|| fail());
    let delay = if cfg!(jnox_partition_mask_long) {
        if quantum % 4 != 0 { fail() }
        quantum.checked_add(quantum / 4).unwrap_or_else(|| fail())
    } else {
        frequency / 1000 + 1
    };
    DUE.store(due, Relaxed);
    TARGET.store(due.checked_add(delay).unwrap_or_else(|| fail()), Relaxed);
}

const IRQ_MASK: u64 = 1 << 7;
#[derive(Clone, Copy, Default)]
struct Sample {
    when: u64,
    count: u64,
    value: u64,
    generation: usize,
    sp: usize,
    pc: usize,
}

#[derive(Default)]
struct Capture {
    set_before: u64,
    saved: u64,
    masked: u64,
    set_after: u64,
    samples: [Sample; 2],
    probe_before: u64,
    daif: u64,
    compare: u64,
    ctl: u64,
    pending: u32,
    probe_after: u64,
    clear_before: u64,
    clear_after: u64,
    restored: u64,
}

static mut RECORD: Capture = unsafe { core::mem::zeroed() };

#[inline(always)]
fn daif() -> u64 {
    let value;
    unsafe { asm!("mrs {}, daif", out(reg) value, options(nostack, preserves_flags)) };
    value
}

pub(super) struct WorkerProbe {
    due: u64,
    target: u64,
    samples: usize,
    crossed: bool,
    active: bool,
}

impl WorkerProbe {
    // Only A calls this private veneer. No call is exposed to task code.
    #[inline(never)]
    pub(super) fn enter() -> Self {
        let due = DUE.load(Relaxed);
        let target = TARGET.load(Relaxed);
        let saved = daif();
        if saved & IRQ_MASK != 0 { fail() }
        unsafe {
            let r = &raw mut RECORD;
            (*r).saved = saved;
            (*r).set_before = counter();
            asm!("msr daifset, #2", "isb", options(nostack, preserves_flags));
            (*r).masked = daif();
            (*r).set_after = counter();
        }
        Self { due, target, samples: 0, crossed: false, active: true }
    }

    // Inlined at the caller so the SP and PC witnesses identify ordinary_a.
    #[inline(always)]
    pub(super) fn observe(&mut self, when: u64, generation: usize, count: u64, value: u64) {
        if !self.active || when < self.due { return }
        if when >= self.target {
            self.active = false;
            self.leave();
            return
        }
        if !self.crossed {
            // The following iteration's arithmetic is wholly after due.
            self.crossed = true;
        } else if self.samples < 2 {
            let sp;
            let pc;
            unsafe {
                asm!("mov {}, sp", "adr {}, .", out(reg) sp, out(reg) pc, options(nostack, preserves_flags));
                (*(&raw mut RECORD)).samples[self.samples] =
                    Sample { when, generation, count, value, sp, pc };
            }
            self.samples += 1;
            if self.samples == 2 { Self::pending_sample() }
        }
    }

    #[inline(never)]
    fn pending_sample() {
        unsafe {
            let r = &raw mut RECORD;
            (*r).probe_before = counter();
            (*r).daif = daif();
            asm!("mrs {}, cntp_cval_el0", out(reg) (*r).compare, options(nostack));
            asm!("mrs {}, cntp_ctl_el0", out(reg) (*r).ctl, options(nostack));
            (*r).pending = gic_read(GICD + 0x200); // banked GICD_ISPENDR0, no acknowledgement
            asm!("dsb sy", options(nostack));
            (*r).probe_after = counter();
        }
    }

    #[inline(never)]
    fn leave(&self) {
        unsafe {
            let r = &raw mut RECORD;
            (*r).clear_before = counter();
            // enter accepted only I=0. Do not change D, A or F.
            asm!("msr daifclr, #2", "isb", options(nostack, preserves_flags));
            let after = counter(); // May execute after A's next dispatch.
            let restored = daif();
            ptr::write_volatile(&raw mut (*r).clear_after, after);
            ptr::write_volatile(&raw mut (*r).restored, restored);
        }
    }
}

pub(super) fn dump(u: &mut Uart) {
    // Timer stopped, only this core writes the one-shot record.
    let r = unsafe { &*(&raw const RECORD) };
    let _ = writeln!(u, "OP MASK mode={} d={} target={} set0={} saved={} masked={} set1={} clear0={} clear1={} restored={}",
        if cfg!(jnox_partition_mask_long) { "long" } else { "short" },
        DUE.load(Relaxed), TARGET.load(Relaxed), r.set_before, r.saved, r.masked, r.set_after,
        r.clear_before, r.clear_after, r.restored);
    for (i,s) in r.samples.iter().enumerate() {
        let _ = writeln!(u, "OP MASKWORK i={} when={} gen={} count={} value={} sp={} pc={}",
            i, s.when, s.generation, s.count, s.value, s.sp, s.pc);
    }
    let _ = writeln!(u, "OP MASKPEND before={} daif={} c={} ctl={} pending={} ppi={} after={}",
        r.probe_before, r.daif, r.compare, r.ctl, r.pending, TIMER_IRQ, r.probe_after);
}
