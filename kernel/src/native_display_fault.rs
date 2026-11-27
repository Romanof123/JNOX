//! Ordinary EL0 synchronous faults for the five-active display package on CPU0.
use super::{Action, COUNT, CURRENT, EXIT_STATUS, NativeFrame, counter, decimal, fail,
            fixed_dispatch, native_el0_mmu, policy, uart_write, unsigned};

const FAULT_EXIT: i32 = -5;
const FAULT_EVENT: u64 = 0x101;
static mut CLASSES: [Option<u8>; COUNT] = [None; COUNT];

pub(super) fn attributable(esr: u64) -> bool {
    let class = (esr >> 26) & 0x3f;
    match class {
        0x00 | 0x15 | 0x22 | 0x26 => true, // undefined, malformed SVC64, PC/SP alignment
        0x3c => esr & (1 << 25) != 0,       // A64 software breakpoint
        0x20 | 0x24 => {
            let fsc = esr & 0x3f;
            let table_walk = esr & (1 << 7) != 0;
            !table_walk && (matches!(fsc, 4..=7 | 12..=15) || (class == 0x24 && fsc == 0x21))
        }
        _ => false,
    }
}

/// Called only after checking the lower-A64 hardware vector and saved EL.
pub(super) fn retire(f: &mut NativeFrame, ttbr: u64) {
    let slot = unsafe { CURRENT };
    if policy().current() != Some(slot) || native_el0_mmu::space_id(slot) != Some(ttbr) {
        fail(9);
    }
    unsafe {
        if CLASSES[slot].is_some() {
            fail(9);
        }
        CLASSES[slot] = Some(((f.esr >> 26) & 0x3f) as u8);
        EXIT_STATUS[slot] = FAULT_EXIT;
    }
    fixed_dispatch(counter(), Action::Exit, FAULT_EVENT, f);
}

pub(super) fn report() {
    for (slot, record) in unsafe { CLASSES }.into_iter().enumerate() {
        if let Some(class) = record {
            uart_write(b"NATIVE EL0 RETIRED ");
            unsigned(slot as u64);
            uart_write(b" ");
            unsigned(u64::from(class));
            uart_write(b" ");
            decimal(i64::from(FAULT_EXIT));
            uart_write(b"\n");
        }
    }
}
