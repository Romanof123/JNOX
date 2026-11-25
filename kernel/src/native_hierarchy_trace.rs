//! CPU0 hierarchy decisions. IRQ masking serializes the writer and return hooks.
//! No UART output or table traversal occurs on a scheduling path.
use core::{arch::asm, ptr};
use super::super::{uart_write, unsigned};

const CAPACITY: usize = 4096;
const WIDTH: usize = 47;
const ROOT: usize = 44;
const RETURN: usize = 45;
const IDLE_LANDING: usize = 46;

/// Times are physical counter ticks. Balances are per-leaf [parent, child,
/// leaf] after settlement and before the new reservation. The single debit
/// to that selected path reconstructs post-reservation balances.
#[derive(Default)]
pub(super) struct Row {
    pub(super) kind: u64,
    pub(super) op: u64,
    pub(super) entry: u64,
    pub(super) settle_begin: u64,
    pub(super) settle_end: u64,
    pub(super) select_begin: u64,
    pub(super) select_end: u64,
    pub(super) source: u64,
    pub(super) target: u64,
    pub(super) ready: u64,
    pub(super) woke: u64,
    pub(super) exited: u64,
    pub(super) old_compare: u64,
    pub(super) next_compare: u64,
    pub(super) grant_start: u64,
    pub(super) grant_end: u64,
    pub(super) elapsed: u64,
    pub(super) overrun: u64,
    pub(super) src_pc: u64,
    pub(super) src_sp: u64,
    pub(super) src_frame: u64,
    pub(super) src_root: u64,
    pub(super) dst_pc: u64,
    pub(super) dst_sp: u64,
    pub(super) dst_frame: u64,
    pub(super) argument: u64,
    pub(super) result: u64,
    pub(super) reserved: u64,
    pub(super) debts: [u64; 3],
    pub(super) sleepers: [u64; 3],
    pub(super) balances: [[u64; 3]; 3],
}

// The leading contiguous prefix and its count are externally readable from a
// stopped machine, including a capture that never reaches terminal export.
#[unsafe(no_mangle)]
static mut native_hierarchy_rows: [[u64; WIDTH]; CAPACITY] = [[0; WIDTH]; CAPACITY];
#[unsafe(no_mangle)]
static mut native_hierarchy_used: usize = 0;
#[unsafe(no_mangle)]
static mut native_hierarchy_lost: u64 = 0;

// Existing native cyclic assembly writes the last pre-ERET / idle-transfer
// counter through this pointer, after the chosen mapping and frame are active.
#[unsafe(no_mangle)]
static mut native_cyclic_return_cell: *mut u64 = ptr::null_mut();

pub(super) fn record(r: Row) {
    unsafe {
        native_cyclic_return_cell = ptr::null_mut();
        let i = native_hierarchy_used;
        if i == CAPACITY {
            native_hierarchy_lost = native_hierarchy_lost.saturating_add(1);
            return;
        }
        let row = &mut *(&raw mut native_hierarchy_rows[i]);
        row[..29].copy_from_slice(&[
            i as u64, r.kind, r.op, r.entry, r.settle_begin, r.settle_end,
            r.select_begin, r.select_end, r.source, r.target, r.ready, r.woke,
            r.exited, r.old_compare, r.next_compare, r.grant_start, r.grant_end,
            r.elapsed, r.overrun, r.src_pc, r.src_sp, r.src_frame, r.src_root,
            r.dst_pc, r.dst_sp, r.dst_frame, r.argument, r.result, r.reserved,
        ]);
        row[29..32].copy_from_slice(&r.debts);
        row[32..35].copy_from_slice(&r.sleepers);
        for (dest, path) in row[35..44].chunks_exact_mut(3).zip(r.balances) {
            dest.copy_from_slice(&path);
        }
        if r.target != 4 { native_cyclic_return_cell = &raw mut row[RETURN]; }
        native_hierarchy_used = i + 1;
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_cyclic_return() {
    unsafe {
        let stamp = native_cyclic_return_cell;
        if stamp.is_null() { return; }
        // A syscall which retains a grant can return again without a new
        // scheduling row. Preserve the first return belonging to the decision.
        if ptr::read_volatile(stamp) != 0 {
            native_cyclic_return_cell = ptr::null_mut();
            return;
        }
        let root: u64;
        asm!("mrs {}, ttbr0_el1", out(reg) root, options(nostack, preserves_flags));
        native_hierarchy_rows[native_hierarchy_used - 1][ROOT] = root;
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_cyclic_idle_sample(stamp: u64) {
    unsafe {
        if !native_cyclic_return_cell.is_null() {
            let row = &mut *(&raw mut native_hierarchy_rows[native_hierarchy_used - 1]);
            if row[9] == 3 && row[IDLE_LANDING] == 0 { row[IDLE_LANDING] = stamp; }
        }
    }
}

pub(super) fn dump() {
    unsafe {
        native_cyclic_return_cell = ptr::null_mut();
        asm!("dsb sy", options(nostack, preserves_flags));
        for i in 0..native_hierarchy_used {
            uart_write(b"NATIVE HIERARCHY EVENT");
            for &v in &*(&raw const native_hierarchy_rows[i]) {
                uart_write(b" ");
                unsigned(v);
            }
            uart_write(b"\n");
        }
        uart_write(b"NATIVE HIERARCHY DROPPED ");
        unsigned(native_hierarchy_lost);
        uart_write(b"\n");
    }
}
