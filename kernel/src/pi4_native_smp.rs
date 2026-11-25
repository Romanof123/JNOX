//! BCM2711 banked GICv2 interfaces and dedicated native exception stacks.
use core::{
    arch::{asm, global_asm},
    mem::{offset_of, size_of},
    ptr,
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
};
#[cfg(jnox_native_smp_migration)]
pub(crate) const CPU_COUNT: usize = super::NATIVE_SMP_ONLINE_CPUS;
#[cfg(not(jnox_native_smp_migration))]
pub(crate) const CPU_COUNT: usize = 2;
const _: () = assert!(CPU_COUNT == 2 || CPU_COUNT == 3);
#[cfg(jnox_native_smp_migration)]
global_asm!(".set PI4_NATIVE_MIGRATION_CPUS,{count}", include_str!("pi4_native_smp.S"),
            count = const CPU_COUNT);
#[cfg(not(jnox_native_smp_migration))]
global_asm!(include_str!("pi4_native_smp.S"), options(raw));

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(super) struct Frame {
    pub x: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    pub pstate: u64,
    pub tpidr: u64,
    pub tpidrro: u64,
    pub fpcr: u64,
    pub fpsr: u64,
    pub q: [u128; 32],
}
impl Frame {
    pub const ZERO: Self = Self {
        x: [0; 31],
        sp: 0,
        pc: 0,
        pstate: 0,
        tpidr: 0,
        tpidrro: 0,
        fpcr: 0,
        fpsr: 0,
        q: [0; 32],
    };
}
const _: () = assert!(
    size_of::<Frame>() == 816 && offset_of!(Frame, q) == 304 && offset_of!(Frame, sp) == 248
);
unsafe extern "C" {
    static pi4_native_smp_vectors: u8;
    static pi4_native_smp_stacks: u8;
    static pi4_native_smp_stamps: [AtomicU64; CPU_COUNT * 8];
    static pi4_native_smp_trampoline: u8;
    static __native_payload_start: u8;
    static __stack_top: u8;
    fn pi4_native_smp_enter(cpu: usize, frame: *const Frame) -> !;
}
const GICD: usize = 0xff841000;
const GICC: usize = 0xff842000;
static TARGET: [AtomicU8; CPU_COUNT] = [const { AtomicU8::new(0) }; CPU_COUNT];
#[cfg(jnox_native_smp_migration)]
static CLAIMED_TARGETS: AtomicU8 = AtomicU8::new(0);
#[cfg(jnox_native_smp_migration)]
static RELEASED_CPUS: AtomicU8 = AtomicU8::new(1);
#[cfg(not(jnox_native_smp_migration))]
static RELEASED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
fn read(addr: usize) -> u32 {
    unsafe { ptr::read_volatile(addr as *const u32) }
}
fn write(addr: usize, x: u32) {
    unsafe { ptr::write_volatile(addr as *mut u32, x) }
}
fn sync() {
    unsafe { asm!("dsb sy", "isb", options(nostack)) }
}
fn require(ok: bool) {
    if !ok {
        unsafe { asm!("msr daifset,#15", "2: wfe", "b 2b", options(noreturn)) }
    }
}
fn local() -> usize {
    let id: u64;
    let el: u64;
    unsafe { asm!("mrs {},mpidr_el1","mrs {},CurrentEL",out(reg)id,out(reg)el,options(nostack)) }
    let cpu = id & 0xff00ffffff;
    require(cpu < CPU_COUNT as u64 && el == 4);
    cpu as usize
}
pub(super) fn stack_state(cpu: usize) -> [u64; 6] {
    require(cpu < CPU_COUNT);
    let base = (&raw const pi4_native_smp_stacks as u64) + cpu as u64 * 0xa000;
    [
        base,
        base + 0x1000,
        base + 0x5000,
        base + 0x5000,
        base + 0x6000,
        base + 0xa000,
    ]
}
pub(super) fn return_stamp(cpu: usize) -> u64 {
    require(cpu < CPU_COUNT);
    unsafe { pi4_native_smp_stamps[cpu * 8].load(Ordering::Acquire) }
}
pub(super) fn idle_stamp(cpu: usize) -> u64 {
    require(cpu < CPU_COUNT);
    unsafe { pi4_native_smp_stamps[cpu * 8 + 1].load(Ordering::Acquire) }
}
pub(super) fn init_distributor() {
    require(local() == 0);
    let secure = read(GICD + 4) & (1 << 10) != 0;
    write(GICD, read(GICD) | if secure { 1 } else { 2 });
    sync();
}
pub(super) fn init_local(cpu: usize) {
    require(local() == cpu);
    let vbar = &raw const pi4_native_smp_vectors;
    unsafe {
        asm!("msr daifset,#15","msr vbar_el1,{}","msr cntp_ctl_el0,xzr","isb",in(reg)vbar,options(nostack))
    }
    let secure = read(GICD + 4) & (1 << 10) != 0;
    let lines = (1u32 << 30) | 6; // SGI1 release, SGI2 stop, PPI30.
    if !secure {
        write(GICD + 0x80, read(GICD + 0x80) | lines);
    }
    for id in [1, 2, 30] {
        unsafe { ptr::write_volatile((GICD + 0x400 + id) as *mut u8, 0x80) }
    }
    write(GICD + 0x100, 1 << 30);
    write(GICC + 4, 0xff);
    write(GICC, read(GICC) | if secure { 1 } else { 6 });
    sync();
    let mask = unsafe { ptr::read_volatile((GICD + 0x800) as *const u8) };
    #[cfg(jnox_native_smp_migration)]
    let distinct = CLAIMED_TARGETS.fetch_or(mask, Ordering::AcqRel) & mask == 0;
    #[cfg(not(jnox_native_smp_migration))]
    let distinct = mask != TARGET[1 - cpu].load(Ordering::Acquire);
    require(
        mask.count_ones() == 1
            && distinct
            && read(GICD + 0x100) & lines == lines
            && read(GICC) & (1 << 9) == 0,
    );
    TARGET[cpu].store(mask, Ordering::Release);
}
#[cfg(jnox_native_smp_migration)]
pub(super) fn target_mask(cpu: usize) -> u8 {
    require(cpu < CPU_COUNT);
    TARGET[cpu].load(Ordering::Acquire)
}

#[cfg(jnox_native_smp_migration)]
pub(super) fn release_secondary() {
    for cpu in 1..CPU_COUNT {
        release_cpu(cpu);
    }
}

#[cfg(jnox_native_smp_migration)]
pub(super) fn release_cpu(cpu: usize) {
    require(local() == 0 && (1..CPU_COUNT).contains(&cpu) && target_mask(0) != 0);
    let previous = (1u8 << cpu) - 1;
    require(RELEASED_CPUS.compare_exchange(previous, previous | (1 << cpu),
                                           Ordering::AcqRel, Ordering::Acquire).is_ok());
    publish_secondary(cpu);
}

#[cfg(not(jnox_native_smp_migration))]
pub(super) fn release_secondary() {
    require(
        local() == 0
            && TARGET[0].load(Ordering::Acquire) != 0
            && !RELEASED.swap(true, Ordering::AcqRel),
    );
    publish_secondary(1);
}

#[inline(always)]
fn publish_secondary(cpu: usize) {
    let entry = &raw const pi4_native_smp_trampoline as usize;
    require((0x80000..0x81000).contains(&entry) && entry % 4 == 0);
    let slot = (0xd8 + 8 * cpu) as *mut u64;
    let start = &raw const __native_payload_start as usize;
    let end = &raw const __stack_top as usize;
    let guards = &raw const pi4_native_smp_stacks as usize;
    let ctr: u64;
    unsafe {
        asm!("mrs {},ctr_el0",out(reg)ctr,options(nostack));
        let line = 4usize << ((ctr >> 16) & 15);
        require(ptr::read_volatile(slot) == 0);
        // Publish the image once, before a secondary can modify shared state.
        if cpu == 1 {
            for addr in (start..end).step_by(line) {
                if addr >= guards && addr - guards < CPU_COUNT * 0xa000
                    && (addr - guards) % 0x5000 < 4096 {
                    continue;
                }
                asm!("dc cvac,{}",in(reg)addr,options(nostack));
            }
        }
        asm!("dsb sy", options(nostack));
        ptr::write_volatile(slot, entry as u64);
        asm!("dc cvac,{}","dsb sy","sev",in(reg)slot,options(nostack));
    }
}
pub(super) fn send_sgi(cpu: usize, id: u8) {
    require(cpu < CPU_COUNT && cpu != local() && matches!(id, 1 | 2));
    let target = TARGET[cpu].load(Ordering::Acquire);
    require(target.count_ones() == 1);
    unsafe { asm!("dsb ishst", options(nostack)) }
    write(
        GICD + 0xf00,
        (u32::from(target) << 16) | (1 << 15) | u32::from(id),
    );
    sync();
}
pub(super) fn eoi(iar: u32) {
    require(iar & 1023 < 1020);
    sync();
    write(GICC + 0x10, iar);
    sync()
}
pub(super) fn arm(at: u64) {
    unsafe {
        asm!("msr cntp_cval_el0,{}","mov x9,#1","msr cntp_ctl_el0,x9","isb",in(reg)at,out("x9") _,options(nostack))
    }
}
pub(super) fn disable_timer() {
    unsafe { asm!("msr cntp_ctl_el0,xzr", "isb", options(nostack)) }
}
/// Called on the stopping secondary after its last owner retired and raw IAR
/// was EOI'd. CPU0 acquires the mask before reading this core's final records.
#[cfg(jnox_native_smp_migration)]
pub(super) fn acknowledge_park(cpu: usize, parked: &AtomicU8) -> ! {
    require(cpu != 0 && local() == cpu);
    unsafe { asm!("msr daifset,#15", options(nostack)) }
    disable_timer();
    sync();
    parked.fetch_or(1 << cpu, Ordering::Release);
    unsafe { asm!("dsb sy", "sev", "2: wfe", "b 2b", options(noreturn)) }
}

pub(super) unsafe fn enter(cpu: usize, frame: *const Frame) -> ! {
    require(local() == cpu);
    unsafe { pi4_native_smp_enter(cpu, frame) }
}
