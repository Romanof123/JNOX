#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use jnox_native::exit;

static mut RAMP: [u8; 4096] = [0; 4096];
#[no_mangle]
static mut private_result: [u64; 16] = [0; 16];
#[no_mangle]
static mut private_blocks: u32 = 0;

#[no_mangle]
#[inline(never)]
pub extern "C" fn histogram_reduce() {
    let ramp = addr_of_mut!(RAMP).cast::<u8>();
    let bins = addr_of_mut!(private_result).cast::<u64>();
    for block in 1..=131_072 {
        for i in 0..4096 {
            // The high nibble indexes one of these sixteen private bins.
            unsafe {
                let sample = read_volatile(ramp.add(i));
                let bin = bins.add((sample >> 4) as usize);
                write_volatile(bin, read_volatile(bin) + 1);
            }
        }
        unsafe { write_volatile(addr_of_mut!(private_blocks), block) };
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let ramp = addr_of_mut!(RAMP).cast::<u8>();
    for i in 0..4096 {
        unsafe { write_volatile(ramp.add(i), (17 * i + 3) as u8) };
    }
    histogram_reduce();
    let bins = addr_of_mut!(private_result).cast::<u64>();
    for i in 0..16 {
        if unsafe { read_volatile(bins.add(i)) } != 33_554_432 {
            return 112;
        }
    }
    if unsafe { read_volatile(addr_of_mut!(private_blocks)) } != 131_072 {
        return 113;
    }
    29
}
