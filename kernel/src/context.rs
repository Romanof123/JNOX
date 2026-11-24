//! Shared AArch64 exception frame and QEMU virt timer interrupt setup.
use core::{arch::asm, ptr};

pub(super) const STACK_BYTES: usize = 16 * 1024;
pub(super) const TIMER_IRQ: u32 = 30; // Nonsecure EL1 physical timer PPI
pub(super) const GICD: usize = 0x0800_0000;
pub(super) const GICC: usize = 0x0801_0000;

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
    // SAFETY: Callers use aligned readable GICv2 MMIO registers with MMU off.
    unsafe { ptr::read_volatile(address as *const u32) }
}

pub(super) fn gic_write(address: usize, value: u32) {
    // SAFETY: Callers use aligned writable GICv2 MMIO registers, single CPU.
    unsafe { ptr::write_volatile(address as *mut u32, value) }
}

pub(super) fn init_timer_irq() {
    // SAFETY: Boot masked exceptions. VBAR is 2-KiB aligned. Disable CNTP until the bench arms it.
    unsafe {
        asm!("msr vbar_el1, {}", "msr cntp_ctl_el0, xzr", "isb",
            in(reg) &raw const sched_vectors, options(nostack, preserves_flags));
    }
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
    unsafe { asm!("dsb sy", "isb", options(nostack, preserves_flags)) };
}
