//! CPU0 is the only writer. Rust stages private payloads with IRQ masked.
//! The assembly EL0 return gate writes R and publishes the even sequence.
use super::*;
use core::ptr::{null_mut, read_volatile, write_volatile};

pub(super) const VA: u64 = 0x0140_0000;
const WORDS: usize = 4;

#[unsafe(no_mangle)]
static mut native_amc_clock_alias: *mut u64 = null_mut();
#[unsafe(no_mangle)]
static mut native_amc_clock_audit: *mut u64 = null_mut();
#[unsafe(no_mangle)]
static mut native_amc_clock_history: [[u64; WORDS]; MAX_ROWS] = [[0; WORDS]; MAX_ROWS];

fn page(pid: usize) -> *mut u64 {
    let p = native_el0_mmu::amc_clock_alias(pid);
    if p.is_null() || p.addr() & 4095 != 0 {
        fail(27);
    }
    p
}

pub(super) fn enable(hz: u64) {
    let freq: u64;
    let access: u64;
    unsafe {
        asm!("msr cntkctl_el1, {only_count}", "isb", "mrs {control},cntkctl_el1",
             "mrs {frequency},cntfrq_el0", only_count=in(reg) 1u64,
             control=out(reg) access, frequency=out(reg) freq, options(nostack,preserves_flags));
    }
    if hz == 0 || freq != hz || access != 1 {
        fail(27);
    }
    for pid in 0..COUNT {
        page(pid);
    }
}

// DMB after invalidation is the writer's leading seqlock barrier. The final
// STLR in the vector orders all payload stores including its last R store.
unsafe fn invalidate(p: *mut u64, seq: u64) {
    unsafe {
        write_volatile(p, seq);
        asm!("dmb ishst", options(nostack, preserves_flags));
    }
}

pub(super) fn begin_generation(pid: usize, generation: u64, hz: u64) {
    let p = page(pid);
    unsafe {
        // The retired frame cannot run. The six non-sequence fields plus R are
        // runtime-owned; the remaining page bytes have never been writable at EL0.
        invalidate(p, 1);
        for (i, value) in [1, pid as u64, generation, hz, CLOCK_DOMAIN.0, 0, 0]
            .into_iter()
            .enumerate()
        {
            write_volatile(p.add(1 + i), value);
        }
    }
}

pub(super) fn revoke(pid: usize) {
    unsafe {
        invalidate(page(pid), 0);
    }
}

pub(super) fn enter(src: usize) {
    unsafe {
        native_amc_clock_alias = null_mut();
        native_amc_clock_audit = null_mut();
        if src < COUNT {
            let p = page(src);
            let s = read_volatile(p);
            if s != 0 && s & 1 == 0 {
                // MAX-1 is the last representable even publication. Zero
                // remains unavailable until a new, pristine generation starts.
                invalidate(p, if s == u64::MAX - 1 { 0 } else { s + 1 });
            }
        }
    }
}

pub(super) fn prepare(pid: usize, generation: u64, settled: u64, row: usize) {
    let p = page(pid);
    unsafe {
        if row < MAX_ROWS {
            let audit = (&raw mut native_amc_clock_history[row]).cast::<u64>();
            write_volatile(audit.add(2), settled);
            write_volatile(audit.add(3), generation);
            native_amc_clock_audit = audit;
        }
        let seq = read_volatile(p);
        if seq == 0 {
            return;
        }
        if seq & 1 == 0 || seq == u64::MAX || read_volatile(p.add(3)) != generation {
            fail(27);
        }
        write_volatile(p.add(6), settled);
        native_amc_clock_alias = p;
    }
}

pub(super) fn report(rows: usize, lost: u64) {
    let control: u64;
    unsafe {
        asm!("mrs {},cntkctl_el1",out(reg)control,options(nostack,preserves_flags));
    }
    print(
        b"CLOCKS",
        &[
            1,
            VA,
            (&raw const native_amc_clock_history).addr() as u64,
            rows as u64,
            WORDS as u64,
            lost,
            control,
        ],
    );
}
