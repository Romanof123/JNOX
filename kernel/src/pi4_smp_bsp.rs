//! Opt-in BCM2711 two-core BSP. CPU0 initializes the distributor once.
//! Banked register snapshots become visible only after the per-core release.
use super::{
    context::{Frame, GICC, GICD, gic_read, gic_write},
    park, uart_write,
};
#[cfg(not(jnox_pi4_migration))]
use super::pi4_smp_workload as work;
#[cfg(jnox_pi4_migration)]
use super::pi4_migration as work;
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
    sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
};

global_asm!(include_str!("pi4_smp_boot.S"),secondary=sym secondary);
#[cfg(jnox_pi4_migration)]
global_asm!(".set JNOX_SMP,1", ".set JNOX_PI4_MIG,1", ".set JNOX_STAMP_X9,1", include_str!("context.S"),
    frame_size=const size_of::<Frame>(),elr=const offset_of!(Frame,elr),
    fpcr=const offset_of!(Frame,fpcr),tpidr_el1=const offset_of!(Frame,tpidr_el1),
    tpidr_el0=const offset_of!(Frame,tpidr_el0),simd=const offset_of!(Frame,simd),
    irq=sym interrupt,svc=sym service,fail=sym exception_fault);
#[cfg(not(jnox_pi4_migration))]
global_asm!(include_str!("context.S"),
    frame_size=const size_of::<Frame>(),elr=const offset_of!(Frame,elr),
    fpcr=const offset_of!(Frame,fpcr),tpidr_el1=const offset_of!(Frame,tpidr_el1),
    tpidr_el0=const offset_of!(Frame,tpidr_el0),simd=const offset_of!(Frame,simd),
    irq=sym interrupt,svc=sym service,fail=sym exception_fault);
unsafe extern "C" {
    static sched_vectors: u8;
    static pi4_secondary_entry: u8;
    static mut pi4_boot_guard: u64;
    static pi4_entry_el: [u64; 2];
    static mut __stack_top: u8;
}
const GUARD: u64 = 0x5049345f5354414b;
static mut BOOT: [[u64; 8]; 2] = [[0; 8]; 2];
static mut GIC: [[u64; 8]; 2] = [[0; 8]; 2];
static TARGET: [AtomicU8; 2] = [const { AtomicU8::new(0) }; 2];
static RELEASED: AtomicBool = AtomicBool::new(false);
static ERROR: AtomicU32 = AtomicU32::new(0);
static ERROR_IAR: AtomicU32 = AtomicU32::new(u32::MAX);
const SLOT: usize = 0xe0;
fn local() -> usize {
    (identity().0 & 0xff) as usize
}
pub(super) fn ticks() -> u64 {
    let v;
    unsafe { asm!("isb","mrs {},cntpct_el0",out(reg)v,options(nostack)) };
    v
}
pub(super) fn frequency() -> u64 {
    let v;
    unsafe { asm!("mrs {},cntfrq_el0",out(reg)v,options(nostack)) };
    v
}
pub(super) fn identity() -> (u64, u64) {
    let m;
    let el;
    unsafe { asm!("mrs {},mpidr_el1","mrs {},CurrentEL",out(reg)m,out(reg)el,options(nostack)) };
    (m, el)
}
pub(super) fn failure() -> u32 {
    ERROR.load(Ordering::Acquire)
}
pub(super) fn failure_iar() -> u32 {
    ERROR_IAR.load(Ordering::Acquire)
}
pub(super) fn target(cpu: usize) -> u8 {
    TARGET.get(cpu).map_or(0, |v| v.load(Ordering::Acquire))
}
pub(super) fn boot_state(cpu: usize) -> [u64; 8] {
    if target(cpu) == 0 {
        return [0; 8];
    }
    unsafe { BOOT[cpu] }
}
pub(super) fn gic_state(cpu: usize) -> [u64; 8] {
    if target(cpu) == 0 {
        return [0; 8];
    }
    unsafe { GIC[cpu] }
}
fn dsb() {
    unsafe { asm!("dsb sy", "isb", options(nostack)) }
}
pub(super) fn fail(code: u32) -> ! {
    unsafe { asm!("msr daifset,#15", options(nostack)) }
    ERROR.store(code, Ordering::Release);
    if local() == 0 {
        uart_write(b"PI4 SMP BSP FAIL ");
        let digits = b"0123456789abcdef";
        for i in (0..8).rev() {
            uart_write(&[digits[((code >> (i * 4)) & 15) as usize]])
        }
        uart_write(b"\n");
    }
    park()
}
extern "C" fn exception_fault() -> ! {
    fail(0x30)
}
fn guard_address(cpu: usize) -> *mut u64 {
    if cpu == 0 {
        (&raw const __stack_top)
            .wrapping_sub(16384)
            .cast_mut()
            .cast()
    } else {
        &raw mut pi4_boot_guard
    }
}
fn guards_ok() -> bool {
    unsafe {
        ptr::read_volatile(guard_address(0)) == GUARD
            && ptr::read_volatile(guard_address(1)) == GUARD
    }
}
fn byte(addr: usize) -> u8 {
    unsafe { ptr::read_volatile(addr as *const u8) }
}
fn set_byte(addr: usize, v: u8) {
    unsafe { ptr::write_volatile(addr as *mut u8, v) }
}
fn init(core: usize) {
    let (mpidr, el) = identity();
    if mpidr & 0xff00ffffff != core as u64 || el != 4 || frequency() == 0 {
        fail(1)
    }
    #[cfg(jnox_pi4_migration)]
    super::pi4_migration_local::install(core);
    let vbar = &raw const sched_vectors;
    let (ttbr, tcr, mair, ctl, ectlr): (u64, u64, u64, u64, u64);
    unsafe {
        asm!("msr vbar_el1,{}","msr cntp_ctl_el0,xzr","isb",in(reg)vbar,options(nostack));
        asm!("mrs {},ttbr0_el1","mrs {},tcr_el1","mrs {},mair_el1","mrs {},sctlr_el1","mrs {},S3_1_C15_C2_1",
            out(reg)ttbr,out(reg)tcr,out(reg)mair,out(reg)ctl,out(reg)ectlr,options(nostack));
        BOOT[core] = [mpidr, pi4_entry_el[core], el, ttbr, tcr, mair, ctl, ectlr];
    }
    let typer = gic_read(GICD + 4);
    let secure_ext = typer & (1 << 10) != 0;
    // Physical nonsecure view aliases EnableGrp1 to bit 0. raspi4b has
    // a combined group view and the GICC needs AckCtl for ordinary IAR.
    if secure_ext && (ectlr & 64) == 0 {
        fail(2)
    }
    let dist = if secure_ext { 1 } else { 2 };
    if core == 0 {
        gic_write(GICD, gic_read(GICD) | dist);
        dsb()
    }
    if gic_read(GICD) & dist == 0 || gic_read(GICC) & (1 << 9) != 0 {
        fail(3)
    }
    if !secure_ext {
        gic_write(GICD + 0x80, gic_read(GICD + 0x80) | 2)
    }
    let group = gic_read(GICD + 0x80);
    if !secure_ext && group & 2 == 0 {
        fail(4)
    }
    // SGI enable bits are architecturally permanently enabled on GICv2.
    set_byte(GICD + 0x401, 0x80);
    gic_write(GICC + 4, 0xff);
    gic_write(GICC, gic_read(GICC) | if secure_ext { 1 } else { 6 });
    dsb();
    let tgt = byte(GICD + 0x800);
    let cpu_ctl = gic_read(GICC);
    let pmr = gic_read(GICC + 4);
    let pri = byte(GICD + 0x401);
    let enabled = gic_read(GICD + 0x100);
    if tgt.count_ones() != 1
        || core == 1 && tgt == target(0)
        || cpu_ctl & (if secure_ext { 1 } else { 6 }) != (if secure_ext { 1 } else { 6 })
        || enabled & 2 == 0
        || pri != 0x80
        || pmr <= pri as u32
    {
        fail(5)
    }
    unsafe {
        GIC[core] = [
            typer as u64,
            tgt as u64,
            group as u64,
            gic_read(GICD) as u64,
            cpu_ctl as u64,
            pmr as u64,
            pri as u64,
            enabled as u64,
        ]
    }
    TARGET[core].store(tgt, Ordering::Release);
}
pub(super) fn release_cpu1() -> bool {
    let entry = &raw const pi4_secondary_entry as usize;
    if local() != 0
        || target(0) == 0
        || entry % 4 != 0
        || !(0x80000..0x100000).contains(&entry)
        || RELEASED.swap(true, Ordering::AcqRel)
    {
        return false;
    }
    let slot = SLOT as *mut u64;
    if unsafe { ptr::read_volatile(slot) } != 0 {
        return false;
    }
    unsafe {
        asm!("dsb ishst", options(nostack));
        ptr::write_volatile(slot, entry as u64);
        asm!("dc cvac,{}","dsb sy","sev",in(reg)slot,options(nostack))
    }
    true
}
#[cfg(not(jnox_pi4_migration))]
pub(super) fn send_sgi1() -> u32 {
    let dest = target(1);
    if local() != 0 || !guards_ok() || dest.count_ones() != 1 || dest == target(0) {
        return 0;
    }
    // NSATT=1 selects Group 1 on a secure/combined interface and is
    // architecturally ignored for a nonsecure distributor write.
    let value = ((dest as u32) << 16) | (1 << 15) | 1;
    unsafe { asm!("dsb ishst", options(nostack)) };
    gic_write(GICD + 0xf00, value);
    dsb();
    value
}
#[cfg(jnox_pi4_migration)]
pub(super) fn signal(destination: usize) -> u32 {
    let mask = target(destination);
    if destination > 1 || destination == local() || mask.count_ones() != 1 || !guards_ok() {
        fail(0x23)
    }
    let value = (u32::from(mask) << 16) | (1 << 15) | 1;
    unsafe { asm!("dsb ishst", options(nostack)) };
    gic_write(GICD + 0xf00, value);
    dsb();
    value
}
extern "C" fn interrupt(frame: *mut Frame) -> *mut Frame {
    let core = local();
    let iar = gic_read(GICC + 0xc);
    #[cfg(jnox_pi4_migration)]
    let source = 1usize.checked_sub(core).unwrap_or_else(|| fail(0x24));
    #[cfg(jnox_pi4_migration)]
    let invalid = iar & 1023 != 1 || target(source) == 0
        || (iar >> 10) & 7 != target(source).trailing_zeros();
    #[cfg(not(jnox_pi4_migration))]
    let invalid = core != 1 || iar & 1023 != 1 || target(0) == 0
        || (iar >> 10) & 7 != target(0).trailing_zeros();
    if invalid
        || !guards_ok()
    {
        ERROR_IAR.store(iar, Ordering::Release);
        if (iar & 1023) < 1020 {
            gic_write(GICC + 0x10, iar);
            dsb()
        }
        fail(0x20)
    }
    let f = work::irq(core, frame, iar);
    gic_write(GICC + 0x10, iar);
    dsb();
    work::eoi_done(core, iar);
    f
}
extern "C" fn service(frame: *mut Frame) -> *mut Frame {
    let esr: u64;
    unsafe { asm!("mrs {},esr_el1",out(reg)esr,options(nostack)) }
    if esr >> 26 != 0x15 || !guards_ok() {
        fail(0x21)
    }
    work::svc(local(), frame, esr as u16)
}
pub(super) fn primary() -> ! {
    unsafe {
        ptr::write_volatile(guard_address(0), GUARD);
        ptr::write_volatile(guard_address(1), GUARD)
    }
    init(0);
    work::start(0)
}
extern "C" fn secondary() -> ! {
    if !guards_ok() {
        fail(0x22)
    }
    init(1);
    work::start(1)
}
