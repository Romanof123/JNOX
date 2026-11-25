//! One cold, single-core virt EL1 address space. Included only by the native profile.
use crate::native_elf::{LoadPlan, Permission};
use core::{arch::asm, ptr};

const PAGE: usize = 4096;
const IMAGE_LO: usize = 0x0040_0000;
const IMAGE_HI: usize = 0x0080_0000;
pub(crate) const STACK_BOTTOM: usize = 0x0100_1000;
pub(crate) const STACK_TOP: usize = 0x0101_1000;
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

static mut ROOT: Table = Table::EMPTY;
static mut LOW: Table = Table::EMPTY;
static mut HIGH: Table = Table::EMPTY;
static mut KERNEL: [Table; HIGH_TABLES] = [const { Table::EMPTY }; HIGH_TABLES];
static mut IMAGE: [Table; 2] = [const { Table::EMPTY }; 2];
static mut STACK: Table = Table::EMPTY;
static mut UART: Table = Table::EMPTY;
static mut GIC: Table = Table::EMPTY;
static mut IMAGE_RAM: Storage<{ IMAGE_HI - IMAGE_LO }> = Storage([0; IMAGE_HI - IMAGE_LO]);
static mut STACK_RAM: Storage<{ STACK_TOP - STACK_BOTTOM }> = Storage([0; STACK_TOP - STACK_BOTTOM]);
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

/// Called once with DAIF masked, on the sole virt CPU, before user execution.
/// The parser supplies borrowed segments. The caller owns the initial stack
/// tail (including argc/argv); the initial SP is STACK_TOP - stack_tail.len().
pub(crate) unsafe fn install(plan: &LoadPlan<'_>, stack_tail: &[u8]) -> Result<(), &'static str> {
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
    if stack_tail.is_empty() || stack_tail.len() > STACK_TOP - STACK_BOTTOM
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

    let image = (&raw mut IMAGE_RAM).cast::<u8>();
    let stack = (&raw mut STACK_RAM).cast::<u8>();
    // The existing link.ld BSS clear zeroes every table, stack byte, unmapped
    // image page and segment tail. No earlier call can have modified them:
    // all possible failures above precede the first write.
    unsafe {
        link(&raw mut ROOT, 0, 30, &raw mut LOW);
        link(&raw mut ROOT, start, 30, &raw mut HIGH);
        for va in (start..end).step_by(PAGE) {
            let table = (&raw mut KERNEL).cast::<Table>().add((va - start) / BLOCK);
            if va % BLOCK == 0 {
                link(&raw mut HIGH, va, 21, table);
            }
            page(table, va, va, NORMAL | UXN | if va < rw { RO } else { PXN });
        }
        for (va, table) in [(0x0900_0000, &raw mut UART), (0x0800_0000, &raw mut GIC),
                            (0x0801_0000, &raw mut GIC)] {
            link(&raw mut LOW, va, 21, table);
            page(table, va, va, DEVICE);
        }
        for s in plan.segments() {
            let begin = s.address as usize;
            ptr::copy_nonoverlapping(s.bytes.as_ptr(), image.add(begin - IMAGE_LO), s.bytes.len());
            let attr = NORMAL | USER | PXN | match s.permission {
                Permission::ReadExecute => RO,
                Permission::Read => RO | UXN,
                Permission::ReadWrite => UXN,
            };
            for va in (begin..s.page_end as usize).step_by(PAGE) {
                let table = (&raw mut IMAGE).cast::<Table>().add((va - IMAGE_LO) / BLOCK);
                link(&raw mut LOW, va, 21, table);
                page(table, va, image.add(va - IMAGE_LO) as usize, attr);
            }
        }
        link(&raw mut LOW, STACK_BOTTOM, 21, &raw mut STACK);
        for va in (STACK_BOTTOM..STACK_TOP).step_by(PAGE) {
            page(&raw mut STACK, va, stack.add(va - STACK_BOTTOM) as usize, NORMAL | USER | PXN | UXN);
        }
        ptr::copy_nonoverlapping(stack_tail.as_ptr(), stack.add(STACK_TOP - STACK_BOTTOM - stack_tail.len()), stack_tail.len());

        // Single CPU, cold uncached mappings. MAIR[0] Normal non-cacheable,
        // MAIR[1] Device-nGnRnE; no cacheable alias or dirty D-cache line.
        // T0SZ=25 starts at L1 (39-bit VA); TTBR1 walks disabled, 32-bit PA.
        let tcr = 25u64 | (3 << 12) | (25 << 16) | (1 << 23) | (2 << 30);
        asm!("dsb sy", "msr mair_el1, {mair}", "msr tcr_el1, {tcr}",
             "msr ttbr0_el1, {root}", "isb", "tlbi vmalle1", "ic iallu",
             "dsb sy", "isb", "msr sctlr_el1, {enable}", "isb",
             mair = in(reg) 0x44u64, tcr = in(reg) tcr, root = in(reg) &raw const ROOT,
             enable = in(reg) ctl | 1, options(nostack, preserves_flags));
        INSTALLED = true;
    }
    Ok(())
}

/// Tests every page against the installed user permissions before a syscall
/// copy. Empty copies access no bytes; callers separately validate descriptors.
pub(crate) fn user_range(addr: usize, len: usize, write: bool) -> bool {
    if !unsafe { INSTALLED } {
        return false;
    }
    if len == 0 {
        return true;
    }
    let Some(last) = addr.checked_add(len - 1) else { return false };
    let (tables, base) = if addr >= IMAGE_LO && last < IMAGE_HI {
        ((&raw const IMAGE).cast::<Table>(), IMAGE_LO)
    } else if addr >= STACK_BOTTOM && last < STACK_TOP {
        (&raw const STACK, STACK_BOTTOM & !(BLOCK - 1))
    } else {
        return false;
    };
    for va in ((addr & !(PAGE - 1))..=last).step_by(PAGE) {
        let descriptor = unsafe {
            tables.add((va - base) / BLOCK).cast::<u64>().add((va >> 12) & 511).read()
        };
        if descriptor & (VALID | USER) != VALID | USER || (write && descriptor & RO != 0) {
            return false;
        }
    }
    true
}

/// Immutable page tables and a single suspended user thread make this copy
/// all-or-nothing for the caller, including unaligned spans crossing a page.
pub(crate) fn copy_to_user(addr: usize, bytes: &[u8; 16]) -> bool {
    if !user_range(addr, bytes.len(), true) {
        return false;
    }
    let (backing, base) = if addr < IMAGE_HI {
        ((&raw mut IMAGE_RAM).cast::<u8>(), IMAGE_LO)
    } else {
        ((&raw mut STACK_RAM).cast::<u8>(), STACK_BOTTOM)
    };
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), backing.add(addr - base), bytes.len()) };
    true
}
