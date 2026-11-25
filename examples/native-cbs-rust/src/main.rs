#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use core::time::Duration;
use jnox_native::{exit, monotonic_time};

static mut RAMP: [u8; 4096] = [0; 4096];
#[no_mangle]
static mut private_result: [u64; 16] = [0; 16];
#[no_mangle]
static mut private_blocks: u64 = 0;
#[no_mangle]
static mut private_bytes: u64 = 0;

#[no_mangle]
#[inline(never)]
pub extern "C" fn histogram_block() {
    let ramp = addr_of_mut!(RAMP).cast::<u8>();
    let bins = addr_of_mut!(private_result).cast::<u64>();
    for i in 0..4096 {
        // The high nibble is in 0..16 and all buffers are process-private.
        unsafe {
            let index = (read_volatile(ramp.add(i)) >> 4) as usize;
            let bin = bins.add(index);
            write_volatile(bin, read_volatile(bin) + 1);
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(111)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let Ok(start) = monotonic_time() else { return 112 };
    let ramp = addr_of_mut!(RAMP).cast::<u8>();
    for i in 0..4096 {
        unsafe { write_volatile(ramp.add(i), i as u8) };
    }
    for n in 1..=(1u64 << 20) {
        histogram_block();
        unsafe {
            write_volatile(addr_of_mut!(private_bytes), n * 4096);
            write_volatile(addr_of_mut!(private_blocks), n);
        }
        let Ok(now) = monotonic_time() else { return 113 };
        let Some(elapsed) = now.checked_sub(start) else { return 114 };
        if elapsed >= Duration::from_millis(650) {
            let bins = addr_of_mut!(private_result).cast::<u64>();
            for i in 0..16 {
                if unsafe { read_volatile(bins.add(i)) } != n * 256 {
                    return 115;
                }
            }
            return 29;
        }
    }
    116
}
