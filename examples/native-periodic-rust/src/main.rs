#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use core::time::Duration;
use jnox_native::{exit, monotonic_time, sleep_until, ClockError};

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(101)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    if sleep_until(Duration::from_secs(i64::MAX as u64 + 1)) != Err(ClockError::Errno(75)) {
        return 1;
    }
    let Ok(mut target) = monotonic_time() else { return 2 };
    let mut completed = 0u32;
    for round in 1..=3 {
        let Some(next) = target.checked_add(Duration::from_millis(40)) else { return 3 };
        target = next;
        if sleep_until(target).is_err() { return 4 }
        let Ok(observed) = monotonic_time() else { return 5 };
        if observed < target { return 6 }
        // Commit this round to the application's report before waiting again.
        unsafe {
            let total = read_volatile(&completed);
            write_volatile(&mut completed, total + round);
        }
        if round == 1 && sleep_until(target).is_err() { return 7 }
    }
    if unsafe { read_volatile(&completed) } == 6 { 0 } else { 8 }
}
