#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use jnox_native::exit;

#[no_mangle]
#[link_section = ".data.private_report"]
static mut private_result: u32 = 0;

#[no_mangle]
#[inline(never)]
pub extern "C" fn count_high_primes() {
    let result = addr_of_mut!(private_result);
    for candidate in (1_000_001u32..2_000_000).step_by(2) {
        let mut divisor = 3;
        while divisor * divisor <= candidate && candidate % divisor != 0 {
            divisor += 2;
        }
        if divisor * divisor > candidate {
            // Only this process writes the private count.
            unsafe { write_volatile(result, read_volatile(result) + 1) };
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    count_high_primes();
    let count = unsafe { read_volatile(addr_of_mut!(private_result)) };
    if count == 70_435 {
        (count % 127) as i32
    } else {
        112
    }
}
