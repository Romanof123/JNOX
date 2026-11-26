//! CPU0 loan aperture. The policy calls these operations with IRQs masked.
//! Only cold install changes the L2 links. Live ownership changes update
//! one aligned L3 descriptor in the target address space.
use super::*;

const BASE: usize = 0x0160_0000;
const SLOTS: usize = 3;
const ATTR: u64 = NORMAL | USER | NG | PXN | UXN;
const _: () = assert!(BASE % BLOCK == 0 && core::mem::size_of::<Storage<PAGE>>() == PAGE);

// The kernel's BSS identity mapping is privileged, RW, XN, Normal NC.
// Each slot's user alias has the same memory type and shareability.
#[unsafe(no_mangle)]
static mut jnox_native_loan_l3_tables: [Table; SPACE_CAPACITY] = [const { Table::EMPTY }; SPACE_CAPACITY];
#[unsafe(no_mangle)]
static mut jnox_native_loan_pages: [Storage<PAGE>; SLOTS] = [const { Storage([0; PAGE]) }; SLOTS];

pub(crate) fn loan_va(slot: usize) -> usize {
    assert!(slot < SLOTS);
    BASE + slot * PAGE
}

pub(super) fn contains_range(first: usize, last: usize) -> bool {
    first >= BASE && last < BASE + SLOTS * PAGE
}

/// Boot supplies an installed PID's private L2 before enabling translation.
pub(super) unsafe fn install_table(pid: usize, low: *mut Table) {
    unsafe { link(low, BASE, 21, &raw mut jnox_native_loan_l3_tables[pid]) };
}

fn entry(pid: usize, slot: usize) -> Option<*mut u64> {
    if !irq_masked() || slot >= SLOTS || space_id(pid).is_none() { return None; }
    // BASE begins an L2 span, hence the first three entries map the slots.
    Some(unsafe { (&raw mut jnox_native_loan_l3_tables[pid]).cast::<u64>().add(slot) })
}

pub(crate) fn loan_pte(pid: usize, slot: usize) -> Option<u64> {
    Some(unsafe { entry(pid, slot)?.read_volatile() })
}

fn backing(slot: usize) -> *mut u8 {
    assert!(slot < SLOTS);
    unsafe { (&raw mut jnox_native_loan_pages).cast::<u8>().add(slot * PAGE) }
}

/// Each transition starts or ends at an invalid PTE. Complete the ASID/VA
/// invalidation before another process may map the page or EL0 may resume.
fn replace(pid: usize, slot: usize, pte: *mut u64, descriptor: u64) {
    let asid = root_id(pid) & 0xffff_0000_0000_0000;
    let operand = asid | ((loan_va(slot) as u64) >> 12);
    unsafe {
        pte.write_volatile(descriptor);
        asm!("dsb ishst", "tlbi vae1is, {}", "dsb ish", "isb",
             in(reg) operand, options(nostack, preserves_flags));
    }
}

fn clear_tail(slot: usize, from: usize) {
    // The caller has removed all user mappings and checked the byte length.
    unsafe {
        ptr::write_bytes(backing(slot).add(from), 0, PAGE - from);
        asm!("dsb ishst", options(nostack, preserves_flags));
    }
}

fn map(pid: usize, slot: usize, writer: bool) -> Result<(), &'static str> {
    let pte = entry(pid, slot).ok_or("loan mapping context")?;
    let count = unsafe { PROCESS_COUNT };
    for owner in 0..count {
        if loan_pte(owner, slot) != Some(0) { return Err("loan slot already mapped"); }
    }
    if writer { clear_tail(slot, 0); }
    let descriptor = backing(slot) as u64 | ATTR | if writer { 0 } else { RO };
    replace(pid, slot, pte, descriptor);
    Ok(())
}

pub(crate) fn loan_map_writer(pid: usize, slot: usize) -> Result<(), &'static str> {
    map(pid, slot, true)
}

/// Immutable qualification input. The CRC differs by one bit from 0x0b759508.
#[cfg(jnox_native_safety_downstream)]
pub(crate) fn load_invalid_fixture(slot: usize) -> Result<(), i32> {
    const FRAME: [u8; 44] = [
        1, 1, 44, 0, 1, 0, 1, 1, // version, type, length, channel and data ID
        1, 0, 0, 0, 0, 0, 0, 0, // generation
        4, 0, 0, 0, 0, 0, 0, 0, // sequence
        0, 0, 0, 0, 0, 0, 0, 0, // issue
        20, 0, 0, 0, 1, 0, 0, 0, // validity, permit and reserved bytes
        0x09, 0x95, 0x75, 0x0b,
    ];
    let count = unsafe { PROCESS_COUNT };
    if count != 3 || (0..count).any(|pid| loan_pte(pid, slot) != Some(0)) {
        return Err(-5);
    }
    // No process has an alias. Publish initialized bytes and a zero tail before mapping.
    unsafe { ptr::copy_nonoverlapping(FRAME.as_ptr(), backing(slot), FRAME.len()); }
    clear_tail(slot, FRAME.len());
    Ok(())
}

pub(crate) fn loan_map_reader(pid: usize, slot: usize) -> Result<(), &'static str> {
    map(pid, slot, false)
}

fn revoke(pid: usize, slot: usize, writer_only: bool) -> Result<(), &'static str> {
    let pte = entry(pid, slot).ok_or("loan revocation context")?;
    let current = unsafe { pte.read_volatile() };
    let writer = backing(slot) as u64 | ATTR;
    if current != writer && (writer_only || current != (writer | RO)) {
        return Err("unexpected loan descriptor");
    }
    replace(pid, slot, pte, 0);
    Ok(())
}

pub(crate) fn loan_publish(pid: usize, slot: usize, len: usize) -> Result<(), &'static str> {
    if len > PAGE { return Err("loan publish length"); }
    revoke(pid, slot, true)?;
    clear_tail(slot, len);
    Ok(())
}

pub(crate) fn loan_revoke(pid: usize, slot: usize) -> Result<(), &'static str> {
    revoke(pid, slot, false)
}

/// Look up a loan VA for a kernel data copy through its identity alias.
/// Slots are physically contiguous in exactly the same order as their VAs.
pub(super) fn translation(pid: usize, addr: usize, write: bool) -> Option<usize> {
    let slot = addr.checked_sub(BASE)? / PAGE;
    let current = loan_pte(pid, slot)?;
    let pa = backing(slot) as usize;
    let writer = pa as u64 | ATTR;
    if current != writer && (write || current != (writer | RO)) { return None; }
    Some(pa + (addr & (PAGE - 1)))
}
