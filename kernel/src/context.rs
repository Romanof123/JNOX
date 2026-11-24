//! Shared AArch64 exception frame and GICv2 physical timer interrupt setup.
use core::{arch::asm, ptr};

pub(super) const STACK_BYTES: usize = 16 * 1024;
pub(super) const TIMER_IRQ: u32 = 30; // Nonsecure EL1 physical timer PPI
#[cfg(not(jnox_pi4))]
pub(super) const GICD: usize = 0x0800_0000;
#[cfg(not(jnox_pi4))]
pub(super) const GICC: usize = 0x0801_0000;

// BCM2711 low-peripheral mode (Raspberry Pi armstubs/armstub8.S).
#[cfg(jnox_pi4)]
pub(super) const GICD: usize = 0xff84_1000;
#[cfg(jnox_pi4)]
pub(super) const GICC: usize = 0xff84_2000;

#[repr(C, align(16))]
pub(super) struct Stack(pub(super) [u8; STACK_BYTES]);

// Each task owns one stack. Its saved SP points to a complete Frame beneath
// its interrupted SP. The CPU-wide translation and vector configuration is
// shared. The benchmark never uses exclusive monitors as task-local state.
#[repr(C, align(16))]
pub(super) struct Frame {
    pub(super) gpr: [u64; 31],
    pub(super) elr: u64,
    pub(super) spsr: u64,
    pub(super) fpcr: u64,
    pub(super) fpsr: u64,
    pub(super) tpidr_el1: u64,
    pub(super) sp_el0: u64,
    pub(super) tpidr_el0: u64,
    pub(super) tpidrro_el0: u64,
    pub(super) reserved: u64,
    pub(super) simd: [[u64; 2]; 32],
}

unsafe extern "C" {
    static sched_vectors: u8;
    pub(super) fn sched_start_frame(frame: *mut Frame) -> !;
}

pub(super) fn gic_read(address: usize) -> u32 {
    // SAFETY: Callers use aligned readable GICv2 MMIO via the physical or
    // identity-mapped Device address for this board.
    unsafe { ptr::read_volatile(address as *const u32) }
}

pub(super) fn gic_write(address: usize, value: u32) {
    // SAFETY: The boot CPU owns setup and the enabled timer interrupt here.
    unsafe { ptr::write_volatile(address as *mut u32, value) }
}

pub(super) fn init_timer_irq() {
    // SAFETY: Boot masked exceptions. VBAR is 2-KiB aligned. Disable CNTP until the bench arms it.
    unsafe {
        asm!("msr vbar_el1, {}", "msr cntp_ctl_el0, xzr", "isb",
            in(reg) &raw const sched_vectors, options(nostack, preserves_flags));
    }
    configure_timer_ppi();
    unsafe { asm!("dsb sy", "isb", options(nostack, preserves_flags)) };
}

#[cfg(not(jnox_pi4))]
fn configure_timer_ppi() {
    // With secure=off the GIC exposes both groups. Use Group 0 as IRQ (FIQEn
    // clear), with priority 0x80 allowed by PMR. PPIs are banked per CPU.
    gic_write(GICD, 0);
    gic_write(GICC, 0);
    gic_write(GICD + 0x80, gic_read(GICD + 0x80) & !(1 << TIMER_IRQ));
    unsafe { ptr::write_volatile((GICD + 0x400 + TIMER_IRQ as usize) as *mut u8, 0x80) };
    gic_write(GICD + 0x280, 1 << TIMER_IRQ); // ICPENDR0
    gic_write(GICD + 0x100, 1 << TIMER_IRQ); // ISENABLER0
    gic_write(GICC + 4, 0xff); // PMR
    gic_write(GICD, 1);
    gic_write(GICC, 1);
}

#[cfg(jnox_pi4)]
fn configure_timer_ppi() {
    let ppi = 1 << TIMER_IRQ;
    // CPU0 owns this banked PPI. CNTP is deasserted and DAIF masked; leave
    // every other enable, pending bit, target and priority unchanged.
    gic_write(GICD + 0x180, ppi); // ICENABLER0, write-one-to-clear
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) };

    let security_extensions = gic_read(GICD + 4) & (1 << 10) != 0;
    let (dist_enable, cpu_enable) = if security_extensions {
        // Physical GIC-400: the secure firmware stub assigns PPI30 to
        // Group 1. IGROUPR is unavailable to our nonsecure EL1; bit 0 of
        // each NS CTLR aliases EnableGrp1. Use ordinary IAR and EOIR.
        (1, 1)
    } else {
        // raspi4b models a GICv2 with two groups and no Security Extensions.
        // Both groups are visible here. Select Group 1 for PPI30, enable
        // Group 1 at bit 1, and permit ordinary IAR to acknowledge it.
        gic_write(GICD + 0x80, gic_read(GICD + 0x80) | ppi);
        (2, 2 | 4)
    };
    // The stub uses combined EOI/deactivation. Preserve the interface mode
    // and reject a foreign split-EOI handoff before unmasking interrupts.
    if gic_read(GICC) & (1 << 9) != 0 {
        super::sched::fail();
    }
    unsafe { ptr::write_volatile((GICD + 0x400 + TIMER_IRQ as usize) as *mut u8, 0x80) };
    gic_write(GICD + 0x280, ppi); // ICPENDR0
    gic_write(GICD + 0x100, ppi); // ISENABLER0
    if gic_read(GICD + 0x100) & ppi == 0 {
        super::sched::fail(); // The secure stub must delegate this PPI.
    }
    gic_write(GICC + 4, 0xff); // PMR permits the timer's priority
    gic_write(GICD, gic_read(GICD) | dist_enable);
    gic_write(GICC, gic_read(GICC) | cpu_enable);
}
