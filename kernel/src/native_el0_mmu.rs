//! Private ordinary-process mappings on the single boot CPU.
use crate::native_elf::{LoadPlan, Permission};
use core::{arch::asm, borrow::Borrow, ptr};

const PAGE: usize = 4096;
const IMAGE_LO: usize = 0x0040_0000;
const IMAGE_HI: usize = 0x0080_0000;
pub(crate) const STACK_BOTTOM: usize = 0x0100_1000;
#[cfg(not(jnox_native_thread_fixed))]
pub(crate) const STACK_TOP: usize = 0x0101_1000;
#[cfg(jnox_native_thread_fixed)]
pub(crate) const STACK_TOP: usize = 0x0102_1000;
#[cfg(jnox_native_threads)]
const WORKER_BOTTOM: usize = STACK_BOTTOM + 2 * PAGE;
#[cfg(jnox_native_threads)]
const WORKER_TOP: usize = STACK_BOTTOM + 8 * PAGE;
#[cfg(all(jnox_native_threads, not(jnox_native_thread_fixed)))]
const MAIN_BOTTOM: usize = WORKER_TOP + PAGE;

#[cfg(jnox_native_thread_fixed)]
const MAIN_BOTTOM: usize = STACK_BOTTOM + 23 * PAGE;

#[cfg(jnox_native_threads)]
pub(crate) fn thread_stack(slot: usize) -> Option<(usize, usize)> {
    match slot {
        0 => Some((MAIN_BOTTOM, STACK_TOP)),
        1 => Some((WORKER_BOTTOM, WORKER_TOP)),
        #[cfg(jnox_native_thread_fixed)]
        2 | 3 => {
            let offset = (slot - 1) * 7 * PAGE;
            Some((WORKER_BOTTOM + offset, WORKER_TOP + offset))
        }
        _ => None,
    }
}

#[cfg(jnox_native_threads)]
pub(crate) fn thread_errno(slot: usize) -> Option<usize> {
    match slot {
        0 => Some(STACK_BOTTOM),
        1 => Some(STACK_BOTTOM + core::mem::size_of::<i32>()),
        #[cfg(jnox_native_thread_fixed)]
        2 | 3 => Some(STACK_BOTTOM + slot * core::mem::size_of::<i32>()),
        _ => None,
    }
}

#[cfg(jnox_native_threads)]
static mut THREAD_CODE: [(usize, usize); crate::native_elf::MAX_LOADS] =
    [(0, 0); crate::native_elf::MAX_LOADS];

#[cfg(jnox_native_threads)]
pub(crate) fn valid_thread_entry(address: u64) -> bool {
    let Ok(addr) = usize::try_from(address) else { return false };
    let Some(end) = addr.checked_add(4) else { return false };
    // Immutable after boot. Keep precise ELF payload bounds rather than page-rounded bounds.
    addr % 4 == 0 && unsafe { THREAD_CODE }.iter().any(|&(lo, hi)| lo <= addr && end <= hi)
}

/// Caller owns the idle worker slot on CPU0 before publishing its new frame.
#[cfg(jnox_native_threads)]
#[cfg(not(jnox_native_thread_fixed))]
pub(crate) unsafe fn reset_worker_memory() {
    unsafe { clear_worker(1); }
}

#[cfg(jnox_native_threads)]
pub(crate) unsafe fn clear_worker(slot: usize) {
    assert!(slot > 0);
    let (lo, hi) = thread_stack(slot).expect("worker stack");
    let errno = thread_errno(slot).expect("worker errno");
    unsafe {
        let backing = (&raw mut SPACES[0].stack_ram.0).cast::<u8>();
        ptr::write_bytes(backing.add(lo - STACK_BOTTOM), 0, hi - lo);
        ptr::write_bytes(backing.add(errno - STACK_BOTTOM), 0, core::mem::size_of::<i32>());
    }
}

#[cfg(not(jnox_pi4))]
const LINK_START: usize = 0x4020_0000;
#[cfg(jnox_pi4)]
const LINK_START: usize = 0x0200_0000;
#[cfg(jnox_pi4)]
const GPIO: usize = 0xfe20_0000;
const SPACE_CAPACITY: usize = 1 + cfg!(jnox_native_peer) as usize + cfg!(jnox_native_peer2) as usize
    + cfg!(jnox_native_peer3) as usize;
// Each slot admits an 8 MiB embedded ELF and backs its 4 MiB virtual image.
// Leave another 4 MiB per slot for tables, stacks and the linked kernel.
const HIGH_TABLES: usize = 8 * SPACE_CAPACITY;
const BLOCK: usize = PAGE * 512;

#[repr(C, align(4096))]
struct Table([u64; 512]);
impl Table {
    const EMPTY: Self = Self([0; 512]);
}
#[repr(C, align(4096))]
struct Storage<const N: usize>([u8; N]);

const IMAGE_BYTES: usize = IMAGE_HI - IMAGE_LO;
const NG: u64 = 1 << 11;
const PA_MASK: u64 = 0xffff_f000; // IPS=32

#[repr(C)]
struct Space {
    root: Table,
    low: Table,
    image: [Table; 2],
    stack: Table,
    image_ram: Storage<IMAGE_BYTES>,
    stack_ram: Storage<{ STACK_TOP - STACK_BOTTOM }>,
}

impl Space {
    const EMPTY: Self = Self {
        root: Table::EMPTY,
        low: Table::EMPTY,
        image: [const { Table::EMPTY }; 2],
        stack: Table::EMPTY,
        image_ram: Storage([0; IMAGE_BYTES]),
        stack_ram: Storage([0; STACK_TOP - STACK_BOTTOM]),
    };
}

static mut SPACES: [Space; SPACE_CAPACITY] = [const { Space::EMPTY }; SPACE_CAPACITY];
static mut HIGH: Table = Table::EMPTY;
static mut KERNEL: [Table; HIGH_TABLES] = [const { Table::EMPTY }; HIGH_TABLES];
static mut UART: Table = Table::EMPTY;
static mut GIC: Table = Table::EMPTY;
#[cfg(jnox_native_shell)]
const SHELL_ROM_VA: usize = 0x0180_0000;
#[cfg(jnox_native_shell)]
const SHELL_ROM_BYTES: usize = 2 * PAGE;
#[cfg(jnox_native_shell)]
static mut SHELL_ROM_TABLE: Table = Table::EMPTY;
#[cfg(jnox_native_shell)]
#[unsafe(no_mangle)]
static jnox_native_shell_rom: Storage<SHELL_ROM_BYTES> =
    Storage(*include_bytes!(env!("JNOX_NATIVE_ROM_IMAGE")));
#[cfg(jnox_native_shell)]
static mut SHELL_STACK_INSTALLED: [bool; SPACE_CAPACITY] = [false; SPACE_CAPACITY];
#[cfg(jnox_native_net_service)]
const VIRTIO_MMIO_BASE: usize = 0x0a00_0000;
#[cfg(jnox_native_net_service)]
static mut VIRTIO_MMIO: Table = Table::EMPTY;
// The horizon word has a privileged writable identity mapping and one
// read-only EL0 alias shared by all participating processes.
#[cfg(jnox_native_weighted_notify)]
const WEIGHTED_NOTIFICATION_VA: usize = 0x0120_0000;
#[cfg(jnox_native_weighted_notify)]
static mut WEIGHTED_NOTIFICATION_TABLE: Table = Table::EMPTY;
#[cfg(jnox_native_weighted_notify)]
static mut WEIGHTED_NOTIFICATION_PAGE: Storage<PAGE> = Storage([0; PAGE]);

/// Publish through the kernel alias on CPU0 before the next exception return.
#[cfg(jnox_native_weighted_notify)]
pub(crate) fn publish_weighted_notification() {
    let word = (&raw mut WEIGHTED_NOTIFICATION_PAGE).cast::<u64>();
    // The backing is page-aligned, initially zero and never writable at EL0.
    unsafe {
        if word.read_volatile() == 0 {
            word.write_volatile(1);
            asm!("dsb ishst", options(nostack, preserves_flags));
        }
    }
}

/// EL0 maps only its own slot at this VA. The privileged identity alias is
/// also available while the AMC loader temporarily uses its kernel-only root.
#[cfg(jnox_native_amc_clock_page)]
pub(crate) const AMC_CLOCK_VA: usize = 0x0140_0000;
#[cfg(jnox_native_amc_clock_page)]
static mut AMC_CLOCK_TABLES: [Table; SPACE_CAPACITY] = [const { Table::EMPTY }; SPACE_CAPACITY];
#[cfg(jnox_native_amc_clock_page)]
#[unsafe(no_mangle)]
static mut jnox_native_amc_clock_pages: [Storage<PAGE>; SPACE_CAPACITY] =
    [const { Storage([0; PAGE]) }; SPACE_CAPACITY];

/// Return null for an uninstalled slot. The masked CPU0 publisher owns writes
/// to the ABI words, including invalidation before a generation is replaced.
#[cfg(jnox_native_amc_clock_page)]
pub(crate) fn amc_clock_alias(pid: usize) -> *mut u64 {
    if space_id(pid).is_none() {
        return ptr::null_mut();
    }
    unsafe { (&raw mut jnox_native_amc_clock_pages[pid]).cast::<u64>() }
}

static mut PROCESS_COUNT: usize = 0;
#[cfg(jnox_native_loan_ipc)]
#[path = "native_loan_mmu.rs"]
mod loan;
#[cfg(jnox_native_loan_ipc)]
pub(crate) use loan::{loan_map_reader, loan_map_writer, loan_pte, loan_publish, loan_revoke, loan_va};
#[cfg(jnox_native_amc)]
#[path = "native_amc_space.rs"]
mod amc_space;
#[cfg(jnox_native_amc)]
pub(crate) use amc_space::amc_reload;
unsafe extern "C" {
    #[cfg(not(jnox_pi4))]
    static _start: u8;
    #[cfg(jnox_pi4)]
    static __native_payload_start: u8;
    static __native_rw_start: u8;
    static __stack_top: u8;
}

const VALID: u64 = 3;
const AF: u64 = 1 << 10;
const INNER: u64 = 3 << 8;
const USER: u64 = 1 << 6;
const RO: u64 = 1 << 7;
const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;
const NORMAL: u64 = VALID | AF | INNER;
const DEVICE: u64 = VALID | AF | (1 << 2) | (2 << 8) | UXN | PXN;

// All table pointers originate in the fixed BSS tables; each index is one
// 9-bit level. On the cold path the tables are private until publication.
unsafe fn put(table: *mut Table, index: usize, descriptor: u64) {
    unsafe { table.cast::<u64>().add(index).write(descriptor) }
}

unsafe fn link(table: *mut Table, va: usize, shift: usize, next: *mut Table) {
    unsafe { put(table, (va >> shift) & 511, next as u64 | VALID) }
}

unsafe fn page(table: *mut Table, va: usize, pa: usize, attr: u64) {
    unsafe { put(table, (va >> 12) & 511, pa as u64 | attr) }
}

fn validate(plan: &LoadPlan<'_>, stack_tail: &[u8]) -> Result<(), &'static str> {
    #[cfg(jnox_native_threads)]
    let bottom = MAIN_BOTTOM;
    #[cfg(not(jnox_native_threads))]
    let bottom = STACK_BOTTOM;
    if stack_tail.is_empty() || stack_tail.len() > STACK_TOP - bottom
        || stack_tail.len() % 16 != 0 {
        return Err("native stack tail");
    }
    let mut previous_end = IMAGE_LO as u64;
    let mut valid_entry = false;
    for s in plan.segments() {
        let file_end = s.address.checked_add(s.bytes.len() as u64).ok_or("native file size")?;
        let mem_end = s.address.checked_add(s.memory_size).ok_or("native memory size")?;
        if s.address < previous_end || s.address % PAGE as u64 != 0 || s.memory_size == 0
            || file_end > mem_end || mem_end > IMAGE_HI as u64
            || mem_end.next_multiple_of(PAGE as u64) != s.page_end {
            return Err("native load range");
        }
        valid_entry |= s.permission == Permission::ReadExecute && (s.address..file_end).contains(&plan.entry);
        previous_end = s.page_end;
    }
    if !valid_entry || plan.entry % 4 != 0 {
        return Err("native executable entry");
    }

    Ok(())
}

// Same linker and cold-boot ABI as the one-process native profile. Each
// process backs the full ordinary image aperture, preserving its existing size.
unsafe fn populate(pid: usize, plan: &LoadPlan<'_>, stack_tail: &[u8]) {
    unsafe {
        let space = &mut *(&raw mut SPACES[pid]);
        link(&raw mut space.root, 0, 30, &raw mut space.low);
        #[cfg(not(jnox_pi4))]
        {
            link(&raw mut space.root, LINK_START, 30, &raw mut HIGH);
            link(&raw mut space.low, crate::UART0, 21, &raw mut UART);
            #[cfg(jnox_native_net_service)]
            link(&raw mut space.low, VIRTIO_MMIO_BASE, 21, &raw mut VIRTIO_MMIO);
            link(&raw mut space.low, crate::context::GICD, 21, &raw mut GIC);
        }
        #[cfg(jnox_pi4)]
        {
            // The relocated kernel shares root[0] with this process's EL0
            // mappings. Only its L3 tables are shared between private L2s.
            let end = &raw const __stack_top as usize;
            for va in (LINK_START..end).step_by(BLOCK) {
                let kernel = (&raw mut KERNEL).cast::<Table>().add((va - LINK_START) / BLOCK);
                link(&raw mut space.low, va, 21, kernel);
            }
            link(&raw mut space.root, GPIO, 30, &raw mut HIGH);
        }
        #[cfg(jnox_native_weighted_notify)]
        link(&raw mut space.low, WEIGHTED_NOTIFICATION_VA, 21, &raw mut WEIGHTED_NOTIFICATION_TABLE);
        #[cfg(jnox_native_loan_ipc)]
        loan::install_table(pid, &raw mut space.low);
        #[cfg(jnox_native_shell)]
        link(&raw mut space.low, SHELL_ROM_VA, 21, &raw mut SHELL_ROM_TABLE);
        #[cfg(jnox_native_amc_clock_page)]
        {
            let table = &raw mut AMC_CLOCK_TABLES[pid];
            let backing = (&raw mut jnox_native_amc_clock_pages[pid]).addr();
            link(&raw mut space.low, AMC_CLOCK_VA, 21, table);
            page(table, AMC_CLOCK_VA, backing, NORMAL | USER | RO | NG | PXN | UXN);
        }
        let image = space.image_ram.0.as_mut_ptr();
        for s in plan.segments() {
            let begin = s.address as usize;
            ptr::copy_nonoverlapping(s.bytes.as_ptr(), image.add(begin - IMAGE_LO), s.bytes.len());
            let attr = NORMAL | USER | NG | PXN | match s.permission {
                Permission::ReadExecute => RO,
                Permission::Read => RO | UXN,
                Permission::ReadWrite => UXN,
            };
            for va in (begin..s.page_end as usize).step_by(PAGE) {
                let table = &raw mut space.image[(va - IMAGE_LO) / BLOCK];
                link(&raw mut space.low, va, 21, table);
                page(table, va, image.add(va - IMAGE_LO) as usize, attr);
            }
        }
        #[cfg(jnox_native_threads)]
        for (index, segment) in plan.segments().iter().enumerate() {
            if segment.permission == Permission::ReadExecute {
                let lo = segment.address as usize;
                THREAD_CODE[index] = (lo, lo + segment.bytes.len());
            }
        }
        let stack = space.stack_ram.0.as_mut_ptr();
        link(&raw mut space.low, STACK_BOTTOM, 21, &raw mut space.stack);
        for va in (STACK_BOTTOM..STACK_TOP).step_by(PAGE) {
            #[cfg(jnox_native_threads)]
            if va != STACK_BOTTOM && !(0..4).filter_map(thread_stack).any(|(lo, hi)| (lo..hi).contains(&va)) {
                continue;
            }
            page(&raw mut space.stack, va, stack.add(va - STACK_BOTTOM) as usize, NORMAL | USER | NG | PXN | UXN);
        }
        ptr::copy_nonoverlapping(stack_tail.as_ptr(), stack.add(STACK_TOP - STACK_BOTTOM - stack_tail.len()), stack_tail.len());
    }
}

// The 8-bit ASID is in bits 55:48. TCR.A1=0 selects TTBR0; TCR.AS=0.
// No ASID is recycled. All global translations are identical across roots.
fn root_id(pid: usize) -> u64 {
    let root = unsafe { &raw const SPACES[pid].root };
    (root as u64) | (((pid + 1) as u64) << 48)
}

/// The caller supplies validated kernel-resident images and initial stack tails.
pub(crate) unsafe fn install(plan: &LoadPlan<'_>, tail: &[u8]) -> Result<(), &'static str> {
    unsafe { install_processes(core::slice::from_ref(plan), &[tail]) }
}

pub(crate) unsafe fn install_pair(plans: [&LoadPlan<'_>; 2], tails: [&[u8]; 2]) -> Result<(), &'static str> {
    unsafe { install_spaces(&plans, &tails) }
}

pub(crate) unsafe fn install_processes(plans: &[LoadPlan<'_>], stack_tails: &[&[u8]]) -> Result<(), &'static str> {
    unsafe { install_spaces(plans, stack_tails) }
}

unsafe fn install_spaces<'a>(
    plans: &[impl Borrow<LoadPlan<'a>>],
    stack_tails: &[&[u8]],
) -> Result<(), &'static str> {
    let count = plans.len();
    if !(1..=SPACE_CAPACITY).contains(&count) || stack_tails.len() != count {
        return Err("native process count");
    }
    let (el, masks, spsel, ctl, sp, pc): (u64, u64, u64, u64, usize, usize);
    unsafe {
        asm!("mrs {}, CurrentEL", "mrs {}, daif", "mrs {}, SPSel", "mrs {}, sctlr_el1",
             "mov {}, sp", "adr {}, .", out(reg) el, out(reg) masks, out(reg) spsel,
             out(reg) ctl, out(reg) sp, out(reg) pc, options(nostack, preserves_flags));
    }
    // MMU, D-cache, I-cache and big-endian EL0/EL1 must all be off.
    if el != 4 || masks & 0x3c0 != 0x3c0 || spsel != 1
        || ctl & (0x1005 | (3 << 24)) != 0 || unsafe { PROCESS_COUNT != 0 } {
        return Err("native VM requires cold masked EL1h");
    }
    #[cfg(not(jnox_pi4))]
    let start = &raw const _start as usize;
    #[cfg(jnox_pi4)]
    let start = &raw const __native_payload_start as usize;
    let rw = &raw const __native_rw_start as usize;
    let end = &raw const __stack_top as usize;
    #[cfg(jnox_pi4)]
    if end > 0x0400_0000 || count != 2 {
        return Err("Pi 4 native kernel aperture or process count");
    }
    if start != LINK_START || !(start < rw && rw < end) || rw % PAGE != 0
        || end > start + HIGH_TABLES * BLOCK || !(start..rw).contains(&pc)
        || !(rw..=end).contains(&sp) {
        return Err("native kernel identity span");
    }
    for pid in 0..count {
        validate(plans[pid].borrow(), stack_tails[pid])?;
    }
    // Cold startup cleared all BSS. Every input rejection precedes these
    // writes, including size and address checks for the last slot.
    unsafe {
        for va in (start..end).step_by(PAGE) {
            let table = (&raw mut KERNEL).cast::<Table>().add((va - start) / BLOCK);
            #[cfg(not(jnox_pi4))]
            if va % BLOCK == 0 {
                link(&raw mut HIGH, va, 21, table);
            }
            page(table, va, va, NORMAL | UXN | if va < rw { RO } else { PXN });
        }
        #[cfg(jnox_pi4)]
        {
            // Both peripheral groups live under root[3]. Only the three
            // peripheral bases and the GPIO page receive EL1 Device mappings.
            link(&raw mut HIGH, GPIO, 21, &raw mut UART);
            link(&raw mut HIGH, crate::context::GICD, 21, &raw mut GIC);
            page(&raw mut UART, GPIO, GPIO, DEVICE);
        }
        #[cfg(jnox_native_net_service)]
        for offset in (0..0x4000).step_by(PAGE) {
            let addr = VIRTIO_MMIO_BASE + offset;
            page(&raw mut VIRTIO_MMIO, addr, addr, DEVICE);
        }
        page(&raw mut UART, crate::UART0, crate::UART0, DEVICE);
        page(&raw mut GIC, crate::context::GICD, crate::context::GICD, DEVICE);
        page(&raw mut GIC, crate::context::GICC, crate::context::GICC, DEVICE);
        #[cfg(jnox_native_weighted_notify)]
        page(&raw mut WEIGHTED_NOTIFICATION_TABLE, WEIGHTED_NOTIFICATION_VA,
             (&raw const WEIGHTED_NOTIFICATION_PAGE) as usize, NORMAL | USER | RO | NG | PXN | UXN);
        #[cfg(jnox_native_shell)]
        for offset in (0..SHELL_ROM_BYTES).step_by(PAGE) {
            let pa = (&raw const jnox_native_shell_rom) as usize + offset;
            page(&raw mut SHELL_ROM_TABLE, SHELL_ROM_VA + offset, pa, NORMAL | USER | RO | NG | PXN | UXN);
        }
        for pid in 0..count {
            populate(pid, plans[pid].borrow(), stack_tails[pid]);
            #[cfg(jnox_native_amc)]
            amc_space::remember(pid, plans[pid].borrow());
        }
        #[cfg(jnox_native_amc)]
        amc_space::prepare_kernel_root();
        // Single CPU, cold uncached mappings. MAIR[0] Normal non-cacheable,
        // MAIR[1] Device-nGnRnE; no cacheable alias or dirty D-cache line.
        // T0SZ=25 starts at L1 (39-bit VA); TTBR1 walks disabled, 32-bit PA.
        let tcr = 25u64 | (3 << 12) | (25 << 16) | (1 << 23) | (2 << 30);
        asm!("dsb sy", "msr mair_el1, {mair}", "msr tcr_el1, {tcr}",
             "msr ttbr0_el1, {root}", "isb", "tlbi vmalle1", "ic iallu",
             "dsb sy", "isb", "msr sctlr_el1, {enable}", "isb",
             mair = in(reg) 0x44u64, tcr = in(reg) tcr, root = in(reg) root_id(0),
             enable = in(reg) ctl | 1, options(nostack, preserves_flags));
        PROCESS_COUNT = count;
    }
    Ok(())
}

pub(crate) fn space_id(pid: usize) -> Option<u64> {
    if pid >= unsafe { PROCESS_COUNT } {
        return None;
    }
    Some(root_id(pid))
}

pub(crate) fn active() -> Option<usize> {
    let count = unsafe { PROCESS_COUNT };
    if count == 0 { return None; }
    let ttbr: u64;
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) ttbr, options(nostack, preserves_flags)) };
    (0..count).find(|&pid| root_id(pid) == ttbr)
}

fn irq_masked() -> bool {
    let daif: u64;
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nostack, preserves_flags)) };
    daif & (1 << 7) != 0
}

/// Called on masked CPU0 while the scheduler still owns the child's dormant
/// lifecycle. Re-checking before commit allows FD redirection to be preflighted.
#[cfg(jnox_native_shell)]
pub(crate) fn validate_dormant_stack(pid: usize, sp: usize, bytes: &[u8]) -> Result<(), &'static str> {
    if pid == 0 || space_id(pid).is_none() || !irq_masked() || active() != Some(0) {
        return Err("shell child stack context");
    }
    if unsafe { SHELL_STACK_INSTALLED[pid] } { return Err("shell child stack already installed"); }
    if bytes.is_empty() || bytes.len() > 1200 || sp < STACK_BOTTOM || sp % 16 != 0
        || sp.checked_add(bytes.len()) != Some(STACK_TOP) {
        return Err("shell child stack bounds");
    }
    Ok(())
}

/// Replace the packaged placeholder stack through its privileged identity
/// alias. No user mappings or registers change here. The scheduler publishes
/// the validated SP and marks the child runnable after the redirects commit.
#[cfg(jnox_native_shell)]
pub(crate) fn install_dormant_stack(pid: usize, sp: usize, bytes: &[u8]) -> Result<(), &'static str> {
    validate_dormant_stack(pid, sp, bytes)?;
    unsafe {
        let stack = (&raw mut SPACES[pid].stack_ram).cast::<u8>();
        ptr::write_bytes(stack, 0, sp - STACK_BOTTOM);
        ptr::copy_nonoverlapping(bytes.as_ptr(), stack.add(sp - STACK_BOTTOM), bytes.len());
        asm!("dsb ishst", options(nostack, preserves_flags));
        SHELL_STACK_INSTALLED[pid] = true;
    }
    Ok(())
}

/// Scheduler owns EL1 and keeps IRQ masked until the selected frame returns.
/// Slot ASIDs are never recycled. Live loan PTE changes invalidate their
/// own VA/ASID before returning. Switching roots needs no additional TLBI.
pub(crate) unsafe fn switch(pid: usize) -> Result<(), &'static str> {
    let target = space_id(pid).ok_or("uninstalled native pid")?;
    let _current = irq_masked().then(active).flatten().ok_or("native switch context")?;
    // The permit profile can resume the same PID from the EL1 idle loop.
    // Its live loan PTE updates already perform synchronous VA/ASID TLBI.
    // Keep the return-path barriers and the caller's saved-frame restore.
    #[cfg(jnox_native_safety_channel)]
    if _current == pid {
        unsafe { asm!("dsb sy", "isb", options(nostack, preserves_flags)); }
        return Ok(());
    }
    unsafe {
        asm!("dsb sy", "msr ttbr0_el1, {}", "isb", in(reg) target, options(nostack, preserves_flags));
    }
    if active() != Some(pid) {
        return Err("native TTBR readback");
    }
    Ok(())
}

fn translation(pid: usize, addr: usize, write: bool) -> Option<usize> {
    space_id(pid)?;
    #[cfg(jnox_native_loan_ipc)]
    if loan::contains_range(addr, addr) {
        return loan::translation(pid, addr, write);
    }
    #[cfg(jnox_native_shell)]
    if (SHELL_ROM_VA..SHELL_ROM_VA + SHELL_ROM_BYTES).contains(&addr) {
        let offset = addr - SHELL_ROM_VA;
        let base = (&raw const jnox_native_shell_rom) as usize;
        let expected = (base + (offset & !(PAGE - 1))) as u64 | NORMAL | USER | RO | NG | PXN | UXN;
        let pte = unsafe { SHELL_ROM_TABLE.0[offset / PAGE] };
        return (!write && pte == expected).then_some(base + offset);
    }
    unsafe {
        let space = &raw const SPACES[pid];
        let (table, base, size) = if (IMAGE_LO..IMAGE_HI).contains(&addr) {
            (&raw const (*space).image[(addr - IMAGE_LO) / BLOCK],
             (&raw const (*space).image_ram) as usize, IMAGE_BYTES)
        } else if (STACK_BOTTOM..STACK_TOP).contains(&addr) {
            (&raw const (*space).stack, (&raw const (*space).stack_ram) as usize,
             STACK_TOP - STACK_BOTTOM)
        } else {
            return None;
        };
        let entry = table.cast::<u64>().add((addr >> 12) & 511).read();
        let required = VALID | USER | NG | AF;
        let pa = (entry & PA_MASK) as usize;
        if entry & required != required || write && entry & RO != 0
            || !(base..base + size).contains(&pa) {
            return None;
        }
        Some(pa + (addr & (PAGE - 1)))
    }
}

fn can_copy(pid: usize, addr: usize, len: usize, write: bool) -> bool {
    if !irq_masked() || space_id(pid).is_none() {
        return false;
    }
    if len == 0 {
        return true;
    }
    let Some(end) = addr.checked_add(len - 1) else { return false };
    // Both endpoints must lie in a single admitted aperture. This also bounds
    // validation of adversarial integer lengths to at most 1024 user pages.
    let admitted = (addr >= IMAGE_LO && end < IMAGE_HI) || (addr >= STACK_BOTTOM && end < STACK_TOP);
    #[cfg(jnox_native_loan_ipc)]
    let admitted = admitted || loan::contains_range(addr, end);
    #[cfg(jnox_native_shell)]
    let admitted = admitted || (addr >= SHELL_ROM_VA && end < SHELL_ROM_VA + SHELL_ROM_BYTES);
    if !admitted {
        return false;
    }
    ((addr & !(PAGE - 1))..=end).step_by(PAGE).all(|va| translation(pid, va, write).is_some())
}

pub(crate) fn user_range(addr: usize, len: usize, write: bool) -> bool {
    active().is_some_and(|pid| can_copy(pid, addr, len, write))
}

pub(crate) fn user_return_pc(addr: usize) -> bool {
    #[cfg(not(any(jnox_native_loan_ipc, jnox_native_shell)))]
    { user_range(addr, 4, false) }
    #[cfg(any(jnox_native_loan_ipc, jnox_native_shell))]
    {
        if addr % 4 != 0 || !(IMAGE_LO..IMAGE_HI).contains(&addr) || !irq_masked() {
            return false;
        }
        let Some(pid) = active() else { return false };
        unsafe {
            let table = &raw const SPACES[pid].image[(addr - IMAGE_LO) / BLOCK];
            let entry = table.cast::<u64>().add((addr >> 12) & 511).read();
            let ram = (&raw const SPACES[pid].image_ram) as usize;
            let pa = ram + ((addr - IMAGE_LO) & !(PAGE - 1));
            entry == (pa as u64 | NORMAL | USER | RO | NG | PXN)
        }
    }
}

// Each aperture is backed contiguously at its VA offset. Use its privileged
// identity alias so a blocked process can receive a result without a switch.
pub(crate) fn copy_from_process(pid: usize, addr: usize, dst: &mut [u8]) -> bool {
    if !can_copy(pid, addr, dst.len(), false) { return false; }
    if dst.is_empty() { return true; }
    let Some(pa) = translation(pid, addr, false) else { return false; };
    unsafe { ptr::copy(pa as *const u8, dst.as_mut_ptr(), dst.len()) };
    true
}

pub(crate) fn copy_to_process(pid: usize, addr: usize, src: &[u8]) -> bool {
    if !can_copy(pid, addr, src.len(), true) { return false; }
    if src.is_empty() { return true; }
    let Some(pa) = translation(pid, addr, true) else { return false; };
    unsafe { ptr::copy(src.as_ptr(), pa as *mut u8, src.len()) };
    true
}

pub(crate) fn copy_from_user(addr: usize, dst: &mut [u8]) -> bool {
    active().is_some_and(|pid| copy_from_process(pid, addr, dst))
}

pub(crate) fn copy_to_user(addr: usize, src: &[u8]) -> bool {
    active().is_some_and(|pid| copy_to_process(pid, addr, src))
}
