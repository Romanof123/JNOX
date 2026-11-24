//! One producer, one consumer, and an independently released diagnostic channel.
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

const TEMPERATURE: [u64; 3] = [18, 23, 21];
const VOLTAGE: [u64; 3] = [3300, 3296, 3304];
static MAILBOX: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
#[repr(C)]
struct ResultRow {
    epoch: u64,
    id: u64,
    input: u64,
    result: u64,
    mailbox: u64,
}
#[unsafe(no_mangle)]
static mut extension_normal_results: [ResultRow; 9] = unsafe { core::mem::zeroed() };

// Only the owner of each private accumulator accesses it on this single core.
static mut PRODUCED: usize = 0;
static mut CONSUMED_SUM: u64 = 0;
static mut RESERVED_INDEX: usize = 0;
static mut RESERVED_SUM: u64 = 0;

pub(super) fn producer_ready() -> bool {
    // The scheduler reads between service windows on CPU 0.
    unsafe { PRODUCED < TEMPERATURE.len() && !occupied() }
}

pub(super) fn occupied() -> bool {
    MAILBOX.load(Ordering::Acquire) != 0
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn normal_job_tick(tag: u64, epoch: u64) {
    let id = tag - super::cookie(0);
    unsafe {
        let (input, result) = match id {
            2 => {
                super::require(!occupied());
                let sample = TEMPERATURE[PRODUCED];
                PRODUCED += 1;
                MAILBOX.store(sample, Ordering::Release);
                (sample, PRODUCED as u64)
            }
            1 => {
                let sample = MAILBOX.swap(0, Ordering::AcqRel);
                super::require(sample != 0);
                CONSUMED_SUM += sample;
                (sample, CONSUMED_SUM)
            }
            0 => {
                let sample = VOLTAGE[RESERVED_INDEX];
                RESERVED_INDEX += 1;
                RESERVED_SUM += sample;
                (sample, RESERVED_SUM)
            }
            _ => super::fail(),
        };
        ptr::write_volatile(&raw mut extension_normal_results[epoch as usize - 1], ResultRow {
            epoch, id, input, result, mailbox: MAILBOX.load(Ordering::Acquire),
        });
    }
}

pub(super) fn complete() -> bool {
    unsafe { PRODUCED == 3 && CONSUMED_SUM == 62 && RESERVED_INDEX == 3 && RESERVED_SUM == 9900 && !occupied() }
}
