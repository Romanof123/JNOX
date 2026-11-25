#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use jnox_native::{exit, yield_now};

static READINGS: [u32; 3] = [18, 23, 21];
#[no_mangle]
static mut private_batches: u32 = 0;
#[no_mangle]
static mut batch_count: [u32; 4] = [0; 4];
#[no_mangle]
static mut batch_sum: [u64; 4] = [0; 4];
#[no_mangle]
static mut cumulative_sum: [u64; 4] = [0; 4];
#[no_mangle]
static mut private_result: u64 = 0;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main(argc: i32, _argv: *const *const u8) -> i32 {
    if argc != 1 { return 112; }
    let mut total = 0u64;
    for (i, expected) in [18, 41, 62].iter().enumerate() {
        let value = u64::from(unsafe { read_volatile(addr_of!(READINGS[i])) });
        total += value;
        unsafe {
            write_volatile(addr_of_mut!(batch_count[i]), 1);
            write_volatile(addr_of_mut!(batch_sum[i]), value);
            write_volatile(addr_of_mut!(cumulative_sum[i]), total);
            write_volatile(addr_of_mut!(private_result), total);
            write_volatile(addr_of_mut!(private_batches), (i + 1) as u32);
        }
        if total != *expected { return 113; }
        if i + 1 < READINGS.len() && yield_now().is_err() { return 114; }
    }
    31
}
