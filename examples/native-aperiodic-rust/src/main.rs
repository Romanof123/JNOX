#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use jnox_native::{exit, monotonic_time};

const BLOCKS_PER_BATCH: u64 = 4096;
static mut RAMP: [u8; 256] = [0; 256];
#[no_mangle]
static mut private_result: [u64; 9] = [0; 9];
#[no_mangle]
static mut private_blocks: u64 = 0;
#[no_mangle]
static mut private_batches: u64 = 0;
#[no_mangle]
static mut start_ns: u64 = 0;
#[no_mangle]
static mut end_ns: u64 = 0;

fn now_ns() -> Option<u64> {
    monotonic_time().ok()?.as_nanos().try_into().ok()
}

#[no_mangle]
#[inline(never)]
unsafe fn density_block(bins: &mut [u64; 9]) {
    for i in 0..256 {
        let byte = read_volatile(addr_of!(RAMP[i]));
        bins[byte.count_ones() as usize] += 1;
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub unsafe extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let Some(start) = now_ns() else { return 112 };
    write_volatile(addr_of_mut!(start_ns), start);
    for i in 0..256 {
        write_volatile(addr_of_mut!(RAMP[i]), i as u8);
    }
    let mut bins = [0u64; 9];
    let mut completed = 0u64;
    loop {
        // The most frequent bin gains 70 samples per complete ramp.
        if completed > u64::MAX / 70 - BLOCKS_PER_BATCH { return 113; }
        for _ in 0..BLOCKS_PER_BATCH {
            density_block(&mut bins);
            completed += 1;
            write_volatile(addr_of_mut!(private_result), bins);
            write_volatile(addr_of_mut!(private_blocks), completed);
        }
        write_volatile(addr_of_mut!(private_batches), completed / BLOCKS_PER_BATCH);
        let Some(now) = now_ns() else { return 114 };
        let Some(elapsed) = now.checked_sub(start) else { return 115 };
        if elapsed >= 900_000_000 {
            write_volatile(addr_of_mut!(end_ns), now);
            for (bin, multiplier) in bins.iter().zip([1,8,28,56,70,56,28,8,1]) {
                if *bin != completed * multiplier { return 116; }
            }
            return 42;
        }
    }
}
