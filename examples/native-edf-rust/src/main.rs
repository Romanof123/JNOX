#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use jnox_native::exit;

#[no_mangle]
static mut private_result: [u32; 16] = [0; 16];
#[no_mangle]
static mut private_progress: u32 = 0;

#[no_mangle]
#[inline(never)]
pub extern "C" fn histogram_reduce() {
    let bins = addr_of_mut!(private_result).cast::<u32>();
    for block in 1..=128 {
        for i in 0..4096 {
            let sample = (17 * i + 3) as u8;
            let index = (sample >> 4) as usize;
            // The high nibble selects one of the sixteen process-private bins.
            unsafe {
                let bin = bins.add(index);
                write_volatile(bin, read_volatile(bin) + 1);
            }
        }
        unsafe { write_volatile(addr_of_mut!(private_progress), block) };
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(110)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    histogram_reduce();
    let bins = addr_of_mut!(private_result).cast::<u32>();
    for i in 0..16 {
        if unsafe { read_volatile(bins.add(i)) } != 32768 {
            return 111;
        }
    }
    29
}
