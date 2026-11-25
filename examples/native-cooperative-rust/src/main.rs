#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use jnox_native::{exit, yield_now};

#[link_section = ".data.private_report"]
#[no_mangle]
static mut peer_total: u32 = 20;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let total = addr_of_mut!(peer_total);
    if total as usize != 0x402000 {
        return 112;
    }
    // Only this application accesses its private, aligned accumulator.
    unsafe { write_volatile(total, read_volatile(total) + 2) };
    if yield_now().is_err() || unsafe { read_volatile(total) } != 22 {
        return 113;
    }
    // The primary has begun its absolute sleep. Finish the report before exit.
    unsafe {
        write_volatile(total, read_volatile(total) + 7);
        read_volatile(total) as i32
    }
}
