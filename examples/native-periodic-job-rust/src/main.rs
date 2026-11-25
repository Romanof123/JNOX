#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use jnox_native::{exit, wait_next_period};

#[no_mangle]
static mut report_result: [[u32; 16]; 3] = [[0; 16]; 3];
#[no_mangle]
static mut report_bytes: [u32; 3] = [0; 3];
#[no_mangle]
static mut report_sequence: [u32; 3] = [0; 3];
#[no_mangle]
static mut report_count: u32 = 0;

#[no_mangle]
#[inline(never)]
pub extern "C" fn histogram_batch(g: usize) -> bool {
    if g >= 3 {
        return false;
    }
    // g indexes one private row and each high nibble selects a valid bin.
    let bins = unsafe { addr_of_mut!(report_result).cast::<u32>().add(g * 16) };
    for i in 0..256 * (32 + g) {
        let sample = (17 * i + g) as u8;
        unsafe {
            let bin = bins.add((sample >> 4) as usize);
            write_volatile(bin, read_volatile(bin) + 1);
        }
    }
    (0..16).all(|i| unsafe { read_volatile(bins.add(i)) } == (16 * (32 + g)) as u32)
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(110)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    for g in 0..3 {
        if !histogram_batch(g) {
            return 111;
        }
        unsafe {
            write_volatile(addr_of_mut!(report_bytes[g]), (256 * (32 + g)) as u32);
            write_volatile(addr_of_mut!(report_sequence[g]), (g + 1) as u32);
            write_volatile(addr_of_mut!(report_count), (g + 1) as u32);
        }
        if g < 2 && wait_next_period().is_err() {
            return 112;
        }
    }
    29
}
