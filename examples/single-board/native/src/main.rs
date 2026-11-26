#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr, time::Duration};
use jnox_native::{exit, monotonic_time, sleep_until};

// Private EL0 memory. Publish the count after the timestamp of each wake.
#[no_mangle]
static mut jnox_partition_heartbeat: [u64; 2] = [0; 2];

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(101)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut count = 0u64;
    loop {
        let Ok(start) = monotonic_time() else { return 1 };
        let deadline = start + Duration::from_millis(100);
        if sleep_until(deadline).is_err() { return 2 }
        let Ok(now) = monotonic_time() else { return 3 };
        if now < deadline { return 4 }
        count = count.wrapping_add(1);
        unsafe {
            ptr::write_volatile(&raw mut jnox_partition_heartbeat[1], now.as_nanos() as u64);
            ptr::write_volatile(&raw mut jnox_partition_heartbeat[0], count);
        }
    }
}
