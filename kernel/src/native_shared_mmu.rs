//! Anonymous shared-memory mappings on CPU0. Each process owns one private
//! L3 table. Each mapping has its own VA and independently chosen permissions.
use super::*;

pub(crate) const OBJECTS: usize = 4;
pub(crate) const MAX_SIZE: usize = 75 * PAGE;
const MAPS: usize = 4;
const BASE: usize = 0x0200_0000;
const STRIDE: usize = BLOCK / MAPS;
const ATTR: u64 = NORMAL | USER | NG | PXN | UXN;
const _: () = assert!(BASE % BLOCK == 0 && MAX_SIZE < STRIDE && STRIDE % PAGE == 0);

#[derive(Clone, Copy)]
#[repr(C)]
struct Mapping {
    object: usize,
    size: usize,
    writable: u64,
}

impl Mapping {
    const EMPTY: Self = Self { object: 0, size: 0, writable: 0 };
}

const _: () = assert!(core::mem::size_of::<Mapping>() == 24);
#[unsafe(no_mangle)]
static mut jnox_native_shared_maps: [[Mapping; MAPS]; SPACE_CAPACITY] = [[Mapping::EMPTY; MAPS]; SPACE_CAPACITY];
#[unsafe(no_mangle)]
static mut jnox_native_shared_l3: [Table; SPACE_CAPACITY] = [const { Table::EMPTY }; SPACE_CAPACITY];
#[unsafe(no_mangle)]
static mut jnox_native_shared_pages: [Storage<MAX_SIZE>; OBJECTS] =
    [const { Storage([0; MAX_SIZE]) }; OBJECTS];

pub(super) unsafe fn install_table(pid: usize, low: *mut Table) {
    unsafe { link(low, BASE, 21, &raw mut jnox_native_shared_l3[pid]) };
}

fn backing(object: usize) -> *mut u8 {
    unsafe { (&raw mut jnox_native_shared_pages[object]).cast() }
}

pub(crate) fn clear(object: usize, size: usize) {
    // The caller holds the last-reference check across allocation. Clear all
    // bytes that can be accessed through the new page-rounded EL0 mapping.
    unsafe { ptr::write_bytes(backing(object), 0, size.next_multiple_of(PAGE)) };
}

// Bytes beyond the logical EOF in a previously writable last page may
// contain user data. Zero the entire newly exposed page tail on growth.
pub(crate) fn zero_growth(object: usize, old: usize, new: usize) {
    unsafe { ptr::write_bytes(backing(object).add(old), 0, new.next_multiple_of(PAGE) - old) };
}

pub(crate) fn referenced(object: usize) -> bool {
    unsafe { (&*(&raw const jnox_native_shared_maps)).iter().flatten().any(|m| m.size != 0 && m.object == object) }
}

fn descriptor(mapping: Mapping, offset: usize) -> u64 {
    (backing(mapping.object) as u64 + offset as u64) | ATTR | if mapping.writable != 0 { 0 } else { RO }
}

// A slot always passes through invalid descriptors before reuse. Publish
// the whole range, invalidate the old translations for this ASID, and finish
// the barrier before recycling backing or returning to a user continuation.
fn update(pid: usize, slot: usize, mapping: Mapping, present: bool) {
    let start = slot * STRIDE;
    let length = mapping.size.next_multiple_of(PAGE);
    let asid = root_id(pid) & 0xffff_0000_0000_0000;
    unsafe {
        let table = (&raw mut jnox_native_shared_l3[pid]).cast::<u64>();
        for offset in (0..length).step_by(PAGE) {
            let entry = if present { descriptor(mapping, offset) } else { 0 };
            table.add((start + offset) / PAGE).write_volatile(entry);
        }
        asm!("dsb ishst", options(nostack, preserves_flags));
        for offset in (0..length).step_by(PAGE) {
            let operand = asid | ((BASE + start + offset) >> 12) as u64;
            asm!("tlbi vae1is, {}", in(reg) operand, options(nostack, preserves_flags));
        }
        asm!("dsb ish", "isb", options(nostack, preserves_flags));
    }
}

pub(crate) fn map(pid: usize, object: usize, size: usize, writable: bool) -> Result<usize, i32> {
    if !irq_masked() || space_id(pid).is_none() { return Err(-5); }
    let maps = unsafe { &mut *(&raw mut jnox_native_shared_maps[pid]) };
    let slot = maps.iter().position(|m| m.size == 0).ok_or(-12)?;
    let mapping = Mapping { object, size, writable: u64::from(writable) };
    update(pid, slot, mapping, true);
    maps[slot] = mapping;
    Ok(BASE + slot * STRIDE)
}

pub(crate) fn unmap(pid: usize, address: usize, length: Option<usize>) -> Result<(), i32> {
    if !irq_masked() || space_id(pid).is_none() { return Err(-5); }
    let offset = address.checked_sub(BASE).ok_or(-22)?;
    if offset >= BLOCK || offset % STRIDE != 0 { return Err(-22); }
    let slot = offset / STRIDE;
    let mapping = unsafe { jnox_native_shared_maps[pid][slot] };
    if mapping.size == 0 { return Err(-22); }
    if length.is_some_and(|n| n != mapping.size) { return Err(-95); }
    update(pid, slot, mapping, false);
    unsafe { jnox_native_shared_maps[pid][slot] = Mapping::EMPTY };
    Ok(())
}

#[cfg(jnox_native_display_standby)]
pub(crate) fn mapping_count(pid: usize) -> usize {
    unsafe { (&*(&raw const jnox_native_shared_maps[pid])).iter().filter(|m| m.size != 0).count() }
}

pub(crate) fn exit(pid: usize) {
    for slot in 0..MAPS {
        let mapping = unsafe { jnox_native_shared_maps[pid][slot] };
        if mapping.size != 0 {
            update(pid, slot, mapping, false);
            unsafe { jnox_native_shared_maps[pid][slot] = Mapping::EMPTY };
        }
    }
}

pub(super) fn contains(address: usize) -> bool {
    (BASE..BASE + BLOCK).contains(&address)
}

pub(super) fn valid_span(pid: usize, first: usize, last: usize) -> bool {
    if !contains(first) { return false; }
    let slot = (first - BASE) / STRIDE;
    let mapping = unsafe { jnox_native_shared_maps[pid][slot] };
    last < BASE + slot * STRIDE + mapping.size
}

pub(super) fn translation(pid: usize, address: usize, write: bool) -> Option<usize> {
    let offset = address - BASE;
    let mapping = unsafe { jnox_native_shared_maps[pid][offset / STRIDE] };
    let within = offset % STRIDE;
    if within >= mapping.size || write && mapping.writable == 0 { return None; }
    let pte = unsafe { (&raw const jnox_native_shared_l3[pid]).cast::<u64>().add(offset / PAGE).read_volatile() };
    (pte == descriptor(mapping, within & !(PAGE - 1))).then(|| backing(mapping.object) as usize + within)
}
