use core::arch::asm;

use super::{park, uart_write};

const UNKNOWN: u64 = u64::MAX;
static mut CONTEXT: [u64; 5] = [UNKNOWN; 5];
static mut STOPPED: bool = false;

pub(super) fn boundary_attempt(current: usize, tick: u64) {
    // At the initial dispatch no prior window exists. At later boundaries
    // keep the outgoing window until the new selection is validated.
    unsafe {
        if CONTEXT[0] == UNKNOWN {
            CONTEXT[0] = current as u64;
            CONTEXT[1] = tick
        }
    }
}
pub(super) fn window(running: usize, owner: Option<usize>, start: u64, end: u64) {
    unsafe {
        CONTEXT[0] = running as u64;
        CONTEXT[4] = owner.map_or(UNKNOWN - 1, |id| id as u64);
        CONTEXT[1] = start;
        CONTEXT[2] = end;
    }
}
pub(super) fn armed(target: u64) {
    unsafe { CONTEXT[3] = target }
}

// Exactly one record and the legacy terminal failure marker. No allocation,
// timing-loop logging, retry, recovery or alternative dispatch path.
#[unsafe(no_mangle)]
pub(super) extern "C" fn partition_diag_stop(
    code: u64,
    actual: u64,
    expected: u64,
    upper: u64,
) -> ! {
    unsafe { asm!("msr daifset,#0xf", options(nostack, preserves_flags)) };
    let context = unsafe {
        if STOPPED {
            park()
        }
        STOPPED = true;
        CONTEXT
    };
    let mut line=*b"PART DIAG1 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000 0000000000000000\n";
    let values = [
        code, actual, expected, upper, context[0], context[1], context[2], context[3], context[4],
    ];
    for (i, value) in values.into_iter().enumerate() {
        for n in 0..16 {
            line[11 + 17 * i + n] = b"0123456789abcdef"[((value >> (4 * (15 - n))) & 15) as usize];
        }
    }
    uart_write(&line);
    uart_write(b"PART FAIL\n");
    park()
}
pub(super) use self::partition_diag_stop as stop;
