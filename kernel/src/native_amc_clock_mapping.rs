/// EL0 maps only its own slot at this VA. The privileged identity alias is
/// also available while the AMC loader temporarily uses its kernel-only root.
pub(crate) const AMC_CLOCK_VA: usize = 0x0140_0000;
static mut AMC_CLOCK_TABLES: [Table; SPACE_CAPACITY] = [const { Table::EMPTY }; SPACE_CAPACITY];
#[unsafe(no_mangle)]
static mut jnox_native_amc_clock_pages: [Storage<PAGE>; SPACE_CAPACITY] =
    [const { Storage([0; PAGE]) }; SPACE_CAPACITY];

/// Return null for an uninstalled slot. The masked CPU0 publisher owns writes
/// to the ABI words, including invalidation before a generation is replaced.
pub(crate) fn amc_clock_alias(pid: usize) -> *mut u64 {
    if space_id(pid).is_none() {
        return ptr::null_mut();
    }
    unsafe { (&raw mut jnox_native_amc_clock_pages[pid]).cast::<u64>() }
}

#[inline(always)]
unsafe fn map_amc_clock_page(low: *mut Table, pid: usize) {
    unsafe {
        let table = &raw mut AMC_CLOCK_TABLES[pid];
        let backing = (&raw mut jnox_native_amc_clock_pages[pid]).addr();
        link(low, AMC_CLOCK_VA, 21, table);
        page(table, AMC_CLOCK_VA, backing, NORMAL | USER | RO | NG | PXN | UXN);
    }
}
