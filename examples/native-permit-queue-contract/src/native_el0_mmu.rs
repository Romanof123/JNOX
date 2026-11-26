//! Single-process backing for the three loan pages. Each page has at most one
//! modeled owner. Writer acquisition zeroes a page, publication revokes it and
//! clears the tail, and only its mapped role can copy bytes. The tokens below
//! describe these permissions inside the component; no hardware PTE is changed.

const PAGE: usize = 4096;
const SLOTS: usize = 3;
const BASE: usize = 0x0160_0000;
const WRITER: u64 = 1;
const READER: u64 = 2;
static mut PAGES: [[u8; PAGE]; SLOTS] = [[0; PAGE]; SLOTS];
static mut ACCESS: [[u64; SLOTS]; super::COUNT] = [[0; SLOTS]; super::COUNT];

pub(super) fn loan_va(slot: usize) -> usize {
    assert!(slot < SLOTS);
    BASE + slot * PAGE
}

pub(super) fn loan_pte(pid: usize, slot: usize) -> Option<u64> {
    (pid < super::COUNT && slot < SLOTS).then(|| unsafe { ACCESS[pid][slot] })
}

fn map(pid: usize, slot: usize, access: u64) -> Result<(), ()> {
    if loan_pte(pid, slot).is_none()
        || (0..super::COUNT).any(|owner| loan_pte(owner, slot) != Some(0))
    {
        return Err(());
    }
    unsafe {
        ACCESS[pid][slot] = access;
    }
    Ok(())
}

pub(super) fn loan_map_writer(pid: usize, slot: usize) -> Result<(), ()> {
    map(pid, slot, WRITER)?;
    unsafe {
        PAGES[slot].fill(0);
    }
    Ok(())
}

pub(super) fn loan_map_reader(pid: usize, slot: usize) -> Result<(), ()> {
    map(pid, slot, READER)
}

pub(super) fn loan_publish(pid: usize, slot: usize, len: usize) -> Result<(), ()> {
    if len > PAGE || loan_pte(pid, slot) != Some(WRITER) {
        return Err(());
    }
    loan_revoke(pid, slot)?;
    unsafe {
        PAGES[slot][len..].fill(0);
    }
    Ok(())
}

pub(super) fn loan_revoke(pid: usize, slot: usize) -> Result<(), ()> {
    match loan_pte(pid, slot) {
        Some(WRITER | READER) => {
            unsafe {
                ACCESS[pid][slot] = 0;
            }
            Ok(())
        }
        _ => Err(()),
    }
}

fn range(address: usize, len: usize) -> Option<(usize, core::ops::Range<usize>)> {
    let offset = address.checked_sub(BASE)?;
    let slot = offset / PAGE;
    let start = offset % PAGE;
    let end = start.checked_add(len)?;
    (slot < SLOTS && end <= PAGE).then_some((slot, start..end))
}

pub(super) fn copy_from_user(address: usize, bytes: &mut [u8]) -> bool {
    let Some((slot, range)) = range(address, bytes.len()) else {
        return false;
    };
    let pid = unsafe { super::CURRENT };
    if !matches!(loan_pte(pid, slot), Some(WRITER | READER)) {
        return false;
    }
    unsafe {
        bytes.copy_from_slice(&PAGES[slot][range]);
    }
    true
}

pub(super) fn write_grant(address: usize, bytes: &[u8]) -> bool {
    let Some((slot, range)) = range(address, bytes.len()) else {
        return false;
    };
    let pid = unsafe { super::CURRENT };
    if loan_pte(pid, slot) != Some(WRITER) {
        return false;
    }
    unsafe {
        PAGES[slot][range].copy_from_slice(bytes);
    }
    true
}
