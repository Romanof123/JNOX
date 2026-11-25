//! Private ordinary-process mappings on the single virt CPU.
use crate::native_elf::{LoadPlan, Permission};
use core::{arch::asm, borrow::Borrow, ptr};

const PAGE: usize = 4096;
const IMAGE_LO: usize = 0x0040_0000;
const IMAGE_HI: usize = 0x0080_0000;
pub(crate) const STACK_BOTTOM: usize = 0x0100_1000;
pub(crate) const STACK_TOP: usize = 0x0101_1000;
#[cfg(jnox_native_threads)]
const WORKER_BOTTOM: usize = STACK_BOTTOM + 2 * PAGE;
#[cfg(jnox_native_threads)]
const WORKER_TOP: usize = STACK_BOTTOM + 8 * PAGE;
#[cfg(jnox_native_threads)]
const MAIN_BOTTOM: usize = WORKER_TOP + PAGE;

#[cfg(jnox_native_threads)]
pub(crate) fn thread_stack(slot: usize) -> Option<(usize, usize)> {
    match slot {
        0 => Some((MAIN_BOTTOM, STACK_TOP)),
        1 => Some((WORKER_BOTTOM, WORKER_TOP)),
        _ => None,
    }
}

#[cfg(jnox_native_threads)]
pub(crate) fn thread_errno(slot: usize) -> Option<usize> {
    match slot {
        0 => Some(STACK_BOTTOM),
        1 => Some(STACK_BOTTOM + core::mem::size_of::<i32>()),
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
pub(crate) unsafe fn reset_worker_memory() {
    unsafe {
        let backing = (&raw mut SPACES[0].stack_ram.0).cast::<u8>();
        ptr::write_bytes(backing.add(WORKER_BOTTOM - STACK_BOTTOM), 0, WORKER_TOP - WORKER_BOTTOM);
        ptr::write_bytes(backing.add(core::mem::size_of::<i32>()), 0, core::mem::size_of::<i32>());
    }
}

const LINK_START: usize = 0x4020_0000;
const SPACE_CAPACITY: usize = 1 + cfg!(jnox_native_peer) as usize + cfg!(jnox_native_peer2) as usize;
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
static mut PROCESS_COUNT: usize = 0;
unsafe extern "C" {
    static _start: u8;
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
        link(&raw mut space.root, LINK_START, 30, &raw mut HIGH);
        link(&raw mut space.low, 0x0900_0000, 21, &raw mut UART);
        link(&raw mut space.low, 0x0800_0000, 21, &raw mut GIC);
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
            if va == STACK_BOTTOM + PAGE || va == WORKER_TOP {
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
        return Err("native VM requires cold masked virt EL1h");
    }
    let start = &raw const _start as usize;
    let rw = &raw const __native_rw_start as usize;
    let end = &raw const __stack_top as usize;
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
            if va % BLOCK == 0 {
                link(&raw mut HIGH, va, 21, table);
            }
            page(table, va, va, NORMAL | UXN | if va < rw { RO } else { PXN });
        }
        page(&raw mut UART, 0x0900_0000, 0x0900_0000, DEVICE);
        page(&raw mut GIC, 0x0800_0000, 0x0800_0000, DEVICE);
        page(&raw mut GIC, 0x0801_0000, 0x0801_0000, DEVICE);
        for pid in 0..count {
            populate(pid, plans[pid].borrow(), stack_tails[pid]);
        }
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

/// Scheduler owns EL1 and keeps IRQ masked until the selected frame returns.
/// For immutable per-ASID nG user entries, a switch does not recycle any TLB
/// tag. Installation already invalidated all predecessors on this sole CPU.
pub(crate) unsafe fn switch(pid: usize) -> Result<(), &'static str> {
    let target = space_id(pid).ok_or("uninstalled native pid")?;
    if !irq_masked() || active().is_none() {
        return Err("native switch context");
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
    if !((addr >= IMAGE_LO && end < IMAGE_HI) || (addr >= STACK_BOTTOM && end < STACK_TOP)) {
        return false;
    }
    ((addr & !(PAGE - 1))..=end).step_by(PAGE).all(|va| translation(pid, va, write).is_some())
}

pub(crate) fn user_range(addr: usize, len: usize, write: bool) -> bool {
    active().is_some_and(|pid| can_copy(pid, addr, len, write))
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
