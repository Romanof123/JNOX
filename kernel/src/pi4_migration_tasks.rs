//! Ordinary EL1 inputs for the Pi 4 frame migration case. Only the running
//! task accesses its stack-local state. The dispatcher owns lease publication,
//! the bounded checkpoint journal, the one terminal commit and the SGIs.
use crate::pi4_migration::{self as runtime, Activation};
use crate::pi4_smp_bsp as bsp;
use core::{arch::asm, hint, ptr};

#[cfg(not(feature = "pi4-stream-v1"))]
pub(crate) const END: u64 = 65_536;
#[cfg(not(feature = "pi4-stream-v1"))]
pub(crate) const BLOCK: u64 = 16;
#[cfg(not(feature = "pi4-stream-v1"))]
pub(crate) const PRODUCT: u64 = 2_147_516_416;
#[cfg(feature = "pi4-stream-v1")]
pub(crate) const END: u64 = 4 * 1024 * 1024;
#[cfg(feature = "pi4-stream-v1")]
pub(crate) const BLOCK: u64 = 1024;
#[cfg(feature = "pi4-stream-v1")]
pub(crate) const PRODUCT: u64 = 0x78d2_2ad1;
const STRIDE: u64 = 16;

#[repr(C)]
struct Progress {
    index: u64,
    sum: u64,
}

// The observer must locate the arithmetic loop in the actual linked symbol.
// The function's entry and epilogue are outside the work interval.
#[cfg(not(feature = "pi4-stream-v1"))]
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

#[cfg(feature = "pi4-stream-v1")]
const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

#[cfg(feature = "pi4-stream-v1")]
#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn pi4_stream_crc32_loop_v1(state: &mut Progress, limit: u64) {
    static TABLE: [u32; 256] = crc_table();
    let mut cursor: *mut Progress = state;
    while state_index(cursor) < limit {
        // Keep X21 bound to the state used by the byte update across preemption.
        unsafe {
            asm!("", inout("x21") cursor, options(nostack, preserves_flags));
            let i = ptr::read_volatile(&raw const (*cursor).index);
            let byte = (i ^ (i >> 8) ^ (i >> 16)) as u8;
            let crc = !(ptr::read_volatile(&raw const (*cursor).sum) as u32);
            let next = (crc >> 8) ^ TABLE[((crc as u8) ^ byte) as usize];
            ptr::write_volatile(&raw mut (*cursor).sum, u64::from(!next));
            ptr::write_volatile(&raw mut (*cursor).index, i + 1);
        }
    }
}

#[cfg_attr(feature = "pi4-stream-v1", unsafe(export_name = "pi4_stream_crc32_m_v1"))]
#[cfg_attr(not(feature = "pi4-stream-v1"), unsafe(no_mangle))]
pub(crate) extern "C" fn pi4_migration_m() -> ! {
    let mut state = Progress { index: 0, sum: 0 };
    let mut checkpoints = runtime::Checkpoints::default();
    while state.index < END {
        let before = runtime::generation(Activation::M);
        let limit = state.index + BLOCK;
        #[cfg(feature = "pi4-stream-v1")]
        pi4_stream_crc32_loop_v1(&mut state, limit);
        #[cfg(not(feature = "pi4-stream-v1"))]
        pi4_migration_input_loop(&mut state, limit);
        runtime::checkpoint(&mut checkpoints, before, Activation::M, state.index, state.sum, (&raw const state).addr());
    }
    runtime::finish(Activation::M, state.sum, checkpoints)
}

fn pinned(activation: Activation, previous: u64, end: u64) -> (u64, runtime::Checkpoints) {
    let mut checkpoints = runtime::Checkpoints::default();
    let mut state = Progress { index: previous, sum: 0 };
    while state.index < end {
        let before = runtime::generation(activation);
        let limit = state.index + STRIDE;
        while state.index < limit {
            state.index += 1;
            state.sum += hint::black_box(state.index);
        }
        runtime::checkpoint(&mut checkpoints, before, activation, state.index, state.sum, (&raw const state).addr());
    }
    (state.sum, checkpoints)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn pi4_migration_p0() -> ! {
    let (sum, checkpoints) = pinned(Activation::P0, 0, 32);
    runtime::finish(Activation::P0, sum, checkpoints)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn pi4_migration_p1_open() -> ! {
    let (sum, checkpoints) = pinned(Activation::P1Open, 0, 16);
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
    runtime::finish(Activation::P1Open, sum, checkpoints)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn pi4_migration_p1() -> ! {
    let (sum, checkpoints) = pinned(Activation::P1, 16, 32);
    runtime::finish(Activation::P1, sum, checkpoints)
}
