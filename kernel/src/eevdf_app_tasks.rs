//! Finite ordinary EEVDF workloads. Each task owns its counters on its stack.
use super::eevdf_app::{drain_requested, finish, responded, wait};
use core::ptr::read_volatile;

static TELEMETRY: [u64; 16] = [11, 7, 23, 5, 19, 3, 29, 13, 2, 31, 17, 37, 41, 43, 47, 53];

const BLOCK: u64 = 64;
pub(crate) const BATCH_LIMIT: u64 = BLOCK * (1 << 32);

#[inline(always)]
fn reading(index: u64) -> u64 {
    // Keep each task's input access in the linked workload even for a fixed ring.
    unsafe { read_volatile(&TELEMETRY[(index & 15) as usize]) }
}

#[inline(never)]
fn batch(id: u64) -> ! {
    let mut count = 0;
    let mut sum = 0;
    let mut weighted_sum = 0;
    while count < BATCH_LIMIT && !drain_requested() {
        let end = count + BLOCK;
        while count < end {
            let sample = reading(3 * count + 5 * id);
            sum += sample;
            weighted_sum += sample * (1 + count % 7);
            count += 1;
        }
    }
    // count == BATCH_LIMIT is always a rejected, exhausted workload.
    finish(id, count, sum, weighted_sum)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn batch_a() -> ! {
    batch(0)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn batch_b() -> ! {
    batch(1)
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn responder() -> ! {
    let mut count = 0;
    let mut sum = 0;
    let mut weighted_sum = 0;
    for sequence in 1..=8 {
        let first = wait(sequence);
        let mut result = 0;
        for offset in 0..16 {
            result += reading(sequence + offset) * (offset + 1);
            count += 1;
        }
        sum += result;
        weighted_sum += sequence * result;
        responded(sequence, first);
    }
    finish(2, count, sum, weighted_sum)
}
