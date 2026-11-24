//! Ordinary EL1 inputs for the Pi 4 frame migration case. Only the running
//! task accesses its stack-local state. The dispatcher owns lease publication,
//! the bounded checkpoint journal, the one terminal commit and the SGIs.
use crate::pi4_migration as runtime;
use crate::pi4_smp_bsp as bsp;
use core::{arch::asm, hint, ptr};

const M: usize = 0;
const P0: usize = 1;
const P1: usize = 2;
const END: u64 = 65_536;
const STRIDE: u64 = 16;

#[repr(C)]
struct Progress {
    index: u64,
    sum: u64,
}

// The observer must locate the arithmetic loop in the actual linked symbol.
// The function's entry and epilogue are outside the work interval.
#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn pi4_migration_input_loop(state: &mut Progress, limit: u64) {
    let mut cursor: *mut Progress = state;
    while state_index(cursor) < limit {
        // An empty assembly block preserves the live task-local pointer in
        // callee-saved X21. Feed that very pointer to the Rust arithmetic.
        unsafe {
            asm!("", inout("x21") cursor, options(nostack, preserves_flags));
            let index = ptr::read_volatile(&raw const (*cursor).index) + 1;
            let sum = ptr::read_volatile(&raw const (*cursor).sum) + index;
            ptr::write_volatile(&raw mut (*cursor).index, index);
            ptr::write_volatile(&raw mut (*cursor).sum, sum);
        }
    }
}

#[inline(always)]
fn state_index(cursor: *const Progress) -> u64 {
    // The caller owns the task stack across a restore on either CPU.
    unsafe { ptr::read_volatile(&raw const (*cursor).index) }
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn pi4_migration_m() -> ! {
    let mut state = Progress { index: 0, sum: 0 };
    let mut checkpoints = runtime::Checkpoints::default();
    while state.index < END {
        let before = runtime::generation(M);
        let limit = state.index + STRIDE;
        pi4_migration_input_loop(&mut state, limit);
        runtime::checkpoint(&mut checkpoints, before, M, state.index, state.sum, (&raw const state).addr());
    }
    runtime::finish(M, state.sum, checkpoints)
}

fn pinned(task: usize, previous: u64, end: u64) -> (u64, runtime::Checkpoints) {
    let mut checkpoints = runtime::Checkpoints::default();
    let mut state = Progress { index: previous, sum: 0 };
    while state.index < end {
        let before = runtime::generation(task);
        let limit = state.index + STRIDE;
        while state.index < limit {
            state.index += 1;
            state.sum += hint::black_box(state.index);
        }
        runtime::checkpoint(&mut checkpoints, before, task, state.index, state.sum, (&raw const state).addr());
    }
    (state.sum, checkpoints)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn pi4_migration_p0() -> ! {
    let (sum, checkpoints) = pinned(P0, 0, 32);
    runtime::finish(P0, sum, checkpoints)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn pi4_migration_p1_open() -> ! {
    let (sum, checkpoints) = pinned(P1, 0, 16);
    // Use the same four-local-seconds bound as the directed-wake bootstrap.
    // Only the opening peer waits; M never polls for an interrupt or peer.
    let hz = bsp::frequency();
    let deadline = hz.checked_mul(4).and_then(|span| bsp::ticks().checked_add(span));
    let deadline = deadline.filter(|_| hz != 0).unwrap_or_else(|| runtime::abort(0x7511));
    while !runtime::first_milestone() {
        if bsp::ticks() >= deadline || bsp::failure() != 0 {
            runtime::abort(0x7511);
        }
        hint::spin_loop();
    }
    runtime::finish(P1, sum, checkpoints)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn pi4_migration_p1() -> ! {
    let (sum, checkpoints) = pinned(P1, 16, 32);
    runtime::finish(P1, sum, checkpoints)
}
