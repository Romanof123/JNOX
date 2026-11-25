#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr};

#[repr(C)]
pub struct Product {
    blocks: u64,
    sum: u64,
    squares: u64,
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! { jnox_native::exit(120) }

#[no_mangle]
pub extern "C" fn client_abort() -> ! { jnox_native::exit(99) }

// Each producer has its own product. The C caller acquires the publication
// mutex or joins the producer before reading it. The input is immutable after
// thread creation. Volatile accesses expose actual completed work to capture.
#[no_mangle]
#[inline(never)]
pub unsafe extern "C" fn reduce_ramps(input: *const u8, output: *mut Product, blocks: u64) {
    let mut sum = 0u64;
    let mut squares = 0u64;
    for n in 1..=blocks {
        for i in 0..256 {
            let x = u64::from(ptr::read_volatile(input.add(i)));
            sum += x;
            squares += x * x;
        }
        ptr::write_volatile(ptr::addr_of_mut!((*output).sum), sum);
        ptr::write_volatile(ptr::addr_of_mut!((*output).squares), squares);
        ptr::write_volatile(ptr::addr_of_mut!((*output).blocks), n);
    }
}
