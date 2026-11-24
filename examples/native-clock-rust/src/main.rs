#![no_std]
#![no_main]

use core::hint::black_box;
use core::panic::PanicInfo;
use jnox_native::{exit, monotonic_time};

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(101)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let Ok(start) = monotonic_time() else { return 1 };
    let mut hash = 0x811c9dc5u32;
    for &byte in black_box(b"hello") {
        hash = (hash ^ u32::from(byte)).wrapping_mul(0x01000193);
    }
    let Ok(end) = monotonic_time() else { return 2 };
    if end < start { return 3 }
    if hash != 0x4f9f2cab { return 4 }
    0
}