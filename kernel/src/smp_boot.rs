//! Two-CPU, cold-reset QEMU virt boot. Only CPU0 prints bootstrap failures.
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicU32, Ordering};

use super::{park, uart_write};
#[cfg(jnox_bench = "power")]
use super::power::cpu_main;
#[cfg(jnox_bench = "smp")]
use super::smp::cpu_main;

const TOKEN: u64 = 0x471;
const REQUEST: u32 = 0x47aa;
const REPLY: u32 = 0x47bb;
const START: u32 = 0x47cc;
static HANDOFF: AtomicU32 = AtomicU32::new(0);

global_asm!(include_str!("smp_boot.S"), token = const TOKEN, secondary = sym secondary);

unsafe extern "C" {
    fn smp_enable_mmu() -> usize;
    fn smp_secondary_entry();
}

// The waiting PE has no runnable work at this point. On serialized TCG
// an explicit AArch64 YIELD also allows the other emulated PE to advance.
pub(super) fn wait_for_peer() {
    unsafe { asm!("yield", options(nomem, nostack, preserves_flags)) };
}

fn ticks() -> u64 {
    let count;
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) count, options(nostack, preserves_flags)) };
    count
}

fn fail(line: &[u8]) -> ! {
    uart_write(line);
    park()
}

pub(super) fn boot() -> ! {
    let affinity: u64;
    unsafe { asm!("mrs {}, mpidr_el1", out(reg) affinity, options(nostack, preserves_flags)) };
    if affinity & 0xff00_ffffff != 0 || unsafe { smp_enable_mmu() } == 0 {
        fail(b"SMP BOOT FAIL PRIMARY MMU\n");
    }
    let frequency: u64;
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nostack, preserves_flags)) };
    if frequency == 0 || frequency > u32::MAX as u64 {
        fail(b"SMP BOOT FAIL COUNTER\n");
    }

    HANDOFF.store(REQUEST, Ordering::Release);
    let status: u64;
    // QEMU 6.2 virt with virtualization=off supplies its internal PSCI HVC
    // conduit. Entry is a physical address, equal to its stage-1 address.
    unsafe {
        asm!("dsb sy", "hvc #0",
            inout("x0") 0xc4000003u64 => status, in("x1") 1u64,
            in("x2") smp_secondary_entry as *const (), in("x3") TOKEN,
            clobber_abi("C"), options(nostack));
    }
    if status != 0 {
        let hex = b"0123456789abcdef";
        let mut line = *b"SMP BOOT FAIL PSCI xx\n";
        let byte = status as u8;
        line[19] = hex[(byte >> 4) as usize];
        line[20] = hex[(byte & 15) as usize];
        fail(&line);
    }
    let start = ticks();
    while HANDOFF.load(Ordering::Acquire) != REPLY {
        if ticks().wrapping_sub(start) >= frequency * 3 {
            fail(b"SMP BOOT FAIL SECONDARY TIMEOUT\n");
        }
        wait_for_peer();
    }
    HANDOFF.store(START, Ordering::Release);
    cpu_main(0)
}

extern "C" fn secondary() -> ! {
    // CPU1 first touches this object through the identical Normal,
    // inner-shareable map. The CAS proves both CPUs observed the exchange.
    if HANDOFF
        .compare_exchange(REQUEST, REPLY, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        park(); // CPU0 reports its timeout.
    }
    while HANDOFF.load(Ordering::Acquire) != START {
        wait_for_peer();
    }
    cpu_main(1)
}
