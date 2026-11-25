#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use jnox_native::exit;

#[cfg(feature = "extended-input")]
const BLOCKS: u32 = 2_097_152;
#[cfg(not(feature = "extended-input"))]
const BLOCKS: u32 = 262_144;

#[no_mangle]
static mut private_result: [u32; 9] = [0; 9];
#[no_mangle]
static mut private_progress: u32 = 0;

#[no_mangle]
#[inline(never)]
pub extern "C" fn histogram_stream() {
    let bins = addr_of_mut!(private_result).cast::<u32>();
    // Every block is a permutation of all 256 bytes.
    for block in 0..BLOCKS {
        for offset in 0..256 {
            let sample = (offset + block) as u8;
            let density = sample.count_ones() as usize;
            // An eight-bit sample always selects a valid, process-private bin.
            unsafe {
                let bin = bins.add(density);
                write_volatile(bin, read_volatile(bin) + 1);
            }
        }
        unsafe { write_volatile(addr_of_mut!(private_progress), (block + 1) * 256) };
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    histogram_stream();
    let bins = addr_of_mut!(private_result).cast::<u32>();
    let multiplicities = [1, 8, 28, 56, 70, 56, 28, 8, 1];
    let mut score = 0u64;
    for (i, multiplicity) in multiplicities.iter().enumerate() {
        let count = unsafe { read_volatile(bins.add(i)) };
        if count != multiplicity * BLOCKS {
            return 112;
        }
        score += (i as u64 + 1).pow(2) * count as u64;
    }
    (score % 251) as i32
}
