//! Two immutable user address spaces on one virt CPU. Included only by the process scheduler.
use crate::native_elf::{LoadPlan, Permission};
use core::{arch::asm, ptr};

const PAGE: usize = 4096;
const IMAGE_LO: usize = 0x0040_0000;
const IMAGE_HI: usize = 0x0080_0000;
pub(crate) const STACK_BOTTOM: usize = 0x0100_1000;
pub(crate) const STACK_TOP: usize = 0x0101_1000;
pub(crate) const CONTROL_VA: usize = 0x0120_0000;
pub(crate) const END_FLAG_VA: usize = CONTROL_VA;
const LINK_START: usize = 0x4020_0000;
// Eight L3 tables cover up to 16 MiB of actual linked kernel (including this
// module's BSS); an image beyond the pool is rejected before installing TTBR0.
const HIGH_TABLES: usize = 8;
const BLOCK: usize = PAGE * 512;

#[repr(C, align(4096))]
struct Table([u64; 512]);
impl Table {
    const EMPTY: Self = Self([0; 512]);
}
#[repr(C, align(4096))]
struct Storage<const N: usize>([u8; N]);


const IMAGE_PAGES: usize = 64;
const NG: u64 = 1 << 11;
const PA_MASK: u64 = 0xffff_f000; // IPS=32

#[repr(C)]
struct Space {
    root: Table,
    low: Table,
    image: [Table; 2],
    stack: Table,
    image_ram: Storage<{ IMAGE_PAGES * PAGE }>,
    stack_ram: Storage<{ STACK_TOP - STACK_BOTTOM }>,
}

impl Space {
    const EMPTY: Self = Self {
        root: Table::EMPTY,
        low: Table::EMPTY,
        image: [const { Table::EMPTY }; 2],
        stack: Table::EMPTY,
        image_ram: Storage([0; IMAGE_PAGES * PAGE]),
        stack_ram: Storage([0; STACK_TOP - STACK_BOTTOM]),
    };
}

static mut SPACES: [Space; 2] = [const { Space::EMPTY }; 2];
static mut HIGH: Table = Table::EMPTY;
static mut KERNEL: [Table; HIGH_TABLES] = [const { Table::EMPTY }; HIGH_TABLES];
static mut UART: Table = Table::EMPTY;
static mut GIC: Table = Table::EMPTY;
static mut CONTROL: Table = Table::EMPTY;
static mut CONTROL_RAM: Storage<PAGE> = Storage([0; PAGE]);
static mut INSTALLED: bool = false;
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
    if stack_tail.is_empty() || stack_tail.len() > STACK_TOP - STACK_BOTTOM
        || stack_tail.len() % 16 != 0 {
        return Err("native stack tail");
    }
    let mut pages = 0;
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
        pages += (s.page_end - s.address) / PAGE as u64;
        if pages > IMAGE_PAGES as u64 { return Err("native image page capacity"); }
        valid_entry |= s.permission == Permission::ReadExecute && (s.address..file_end).contains(&plan.entry);
        previous_end = s.page_end;
    }
    if !valid_entry || plan.entry % 4 != 0 {
        return Err("native executable entry");
    }

    Ok(())
}

// Same linker and cold-boot ABI as the one-process native profile. Each
// validated segment gets consecutive physical pages, regardless of its VA.
unsafe fn populate(pid: usize, plan: &LoadPlan<'_>, stack_tail: &[u8]) {
    unsafe {
        let space = &mut *(&raw mut SPACES[pid]);
        link(&raw mut space.root, 0, 30, &raw mut space.low);
        link(&raw mut space.root, LINK_START, 30, &raw mut HIGH);
        link(&raw mut space.low, 0x0900_0000, 21, &raw mut UART);
        link(&raw mut space.low, 0x0800_0000, 21, &raw mut GIC);
        link(&raw mut space.low, CONTROL_VA, 21, &raw mut CONTROL);
        let image = space.image_ram.0.as_mut_ptr();
        let mut used = 0;
        for s in plan.segments() {
            let begin = s.address as usize;
            ptr::copy_nonoverlapping(s.bytes.as_ptr(), image.add(used), s.bytes.len());
            let attr = NORMAL | USER | NG | PXN | match s.permission {
                Permission::ReadExecute => RO,
                Permission::Read => RO | UXN,
                Permission::ReadWrite => UXN,
            };
            for va in (begin..s.page_end as usize).step_by(PAGE) {
                let table = &raw mut space.image[(va - IMAGE_LO) / BLOCK];
                link(&raw mut space.low, va, 21, table);
                page(table, va, image.add(used) as usize, attr);
                used += PAGE;
            }
        }
        let stack = space.stack_ram.0.as_mut_ptr();
        link(&raw mut space.low, STACK_BOTTOM, 21, &raw mut space.stack);
        for va in (STACK_BOTTOM..STACK_TOP).step_by(PAGE) {
            page(&raw mut space.stack, va, stack.add(va - STACK_BOTTOM) as usize, NORMAL | USER | NG | PXN | UXN);
        }
        ptr::copy_nonoverlapping(stack_tail.as_ptr(), stack.add(STACK_TOP - STACK_BOTTOM - stack_tail.len()), stack_tail.len());
    }
}

// The 8-bit ASID is in bits 55:48. TCR.A1=0 selects TTBR0; TCR.AS=0.
// Neither ASID is recycled. All global translations are identical in both roots.
fn root_id(pid: usize) -> u64 {
    let root = unsafe { &raw const SPACES[pid].root };
    (root as u64) | (((pid + 1) as u64) << 48)
}

/// Cold EL1h only, one core, both images and tails borrowed from kernel memory.
/// The initial SP for each process is STACK_TOP minus its supplied tail length.
pub(crate) unsafe fn install(plans: [&LoadPlan<'_>; 2], stack_tails: [&[u8]; 2], first: usize) -> Result<(), &'static str> {
    let (el, masks, spsel, ctl, sp, pc): (u64, u64, u64, u64, usize, usize);
    unsafe {
        asm!("mrs {}, CurrentEL", "mrs {}, daif", "mrs {}, SPSel", "mrs {}, sctlr_el1",
             "mov {}, sp", "adr {}, .", out(reg) el, out(reg) masks, out(reg) spsel,
             out(reg) ctl, out(reg) sp, out(reg) pc, options(nostack, preserves_flags));
    }
    // MMU, D-cache, I-cache and big-endian EL0/EL1 must all be off.
    if el != 4 || masks & 0x3c0 != 0x3c0 || spsel != 1
        || ctl & (0x1005 | (3 << 24)) != 0 || unsafe { INSTALLED } {
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
    if first >= 2 { return Err("native initial pid"); }
    validate(plans[0], stack_tails[0])?;
    validate(plans[1], stack_tails[1])?;
    // Cold startup cleared all BSS. Every input rejection precedes these
    // writes, including the second process's size and address checks.
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
        // Same physical page for both tasks. Only its first u64 is published;
        // both adjacent virtual pages remain unmapped. Privileged code writes
        // through its identity alias; user mappings are read-only and XN.
        page(&raw mut CONTROL, CONTROL_VA, (&raw const CONTROL_RAM) as usize,
             NORMAL | USER | NG | RO | UXN | PXN);
        populate(0, plans[0], stack_tails[0]);
        populate(1, plans[1], stack_tails[1]);
        // Single CPU, cold uncached mappings. MAIR[0] Normal non-cacheable,
        // MAIR[1] Device-nGnRnE; no cacheable alias or dirty D-cache line.
        // T0SZ=25 starts at L1 (39-bit VA); TTBR1 walks disabled, 32-bit PA.
        let tcr = 25u64 | (3 << 12) | (25 << 16) | (1 << 23) | (2 << 30);
        asm!("dsb sy", "msr mair_el1, {mair}", "msr tcr_el1, {tcr}",
             "msr ttbr0_el1, {root}", "isb", "tlbi vmalle1", "ic iallu",
             "dsb sy", "isb", "msr sctlr_el1, {enable}", "isb",
             mair = in(reg) 0x44u64, tcr = in(reg) tcr, root = in(reg) root_id(first),
             enable = in(reg) ctl | 1, options(nostack, preserves_flags));
        INSTALLED = true;
    }
    Ok(())
}

pub(crate) fn space_id(pid: usize) -> Option<u64> {
    if pid >= 2 || !unsafe { INSTALLED } {
        return None;
    }
    Some(root_id(pid))
}

pub(crate) fn active() -> Option<usize> {
    if !unsafe { INSTALLED } {
        return None;
    }
    let ttbr: u64;
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) ttbr, options(nostack, preserves_flags)) };
    (0..2).find(|&pid| root_id(pid) == ttbr)
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
             (&raw const (*space).image_ram) as usize, IMAGE_PAGES * PAGE)
        } else if (STACK_BOTTOM..STACK_TOP).contains(&addr) {
            (&raw const (*space).stack, (&raw const (*space).stack_ram) as usize,
             STACK_TOP - STACK_BOTTOM)
        } else if (CONTROL_VA..CONTROL_VA + PAGE).contains(&addr) {
            (&raw const CONTROL, (&raw const CONTROL_RAM) as usize, PAGE)
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

/// Physical base of the mapped page, including for the inactive PID. This
/// observer never dereferences the requested VA or grants a copy capability.
pub(crate) fn backing_page(pid: usize, va: usize) -> Option<usize> {
    translation(pid, va & !(PAGE - 1), false)
}

fn can_copy(pid: usize, addr: usize, len: usize, write: bool) -> bool {
    if !irq_masked() || active() != Some(pid) {
        return false;
    }
    if len == 0 {
        return true;
    }
    let Some(end) = addr.checked_add(len - 1) else { return false };
    // Both endpoints must lie in a single admitted aperture. This also bounds
    // validation of adversarial integer lengths to at most 1024 user pages.
    if !((addr >= IMAGE_LO && end < IMAGE_HI) || (addr >= STACK_BOTTOM && end < STACK_TOP)
        || (addr >= CONTROL_VA && end < CONTROL_VA + PAGE)) {
        return false;
    }
    ((addr & !(PAGE - 1))..=end).step_by(PAGE).all(|va| translation(pid, va, write).is_some())
}

// On this single CPU an IRQ-masked copy cannot be switched, and process page
// tables are immutable. After preflight the current VA is safe for the full
// range even across discontiguous physical pages. Callers supply kernel slices.
pub(crate) fn copy_from_user(pid: usize, va: usize, dst: &mut [u8]) -> bool {
    if !can_copy(pid, va, dst.len(), false) {
        return false;
    }
    if !dst.is_empty() {
        unsafe { ptr::copy(va as *const u8, dst.as_mut_ptr(), dst.len()) };
    }
    true
}

pub(crate) fn copy_to_user(pid: usize, va: usize, src: &[u8]) -> bool {
    if !can_copy(pid, va, src.len(), true) {
        return false;
    }
    if !src.is_empty() {
        unsafe { ptr::copy(src.as_ptr(), va as *mut u8, src.len()) };
    }
    true
}

// Publish completion once through the privileged identity mapping. The caller
// owns the experiment horizon. EL0 can observe but cannot set or reset it.
pub(crate) fn set_end() -> bool {
    if !irq_masked() || active().is_none() {
        return false;
    }
    unsafe {
        let word = (&raw mut CONTROL_RAM).cast::<u64>();
        if word.read_volatile() != 0 { return false; }
        asm!("stlr {value}, [{word}]", "dsb ish", value = in(reg) 1u64,
             word = in(reg) word, options(nostack, preserves_flags));
    }
    true
}
