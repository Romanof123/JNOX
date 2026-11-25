#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile, time::Duration};
use jnox_native::{exit, mailbox, monotonic_time, sleep_until};

#[no_mangle]
static mut mailbox_received: [u64; 5] = [0; 5];
#[no_mangle]
static mut mailbox_empty_status: i64 = i64::MAX;
#[no_mangle]
static mut mailbox_reading_sum: u64 = 0;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let Ok(now) = monotonic_time() else { return 87 };
    let Some(wake) = now.checked_add(Duration::from_millis(400)) else { return 88 };
    if sleep_until(wake).is_err() { return 89; }

    let expected = [300u32, 512, 257, 1025, 88];
    let mut sum = 0;
    for (i, reading) in expected.into_iter().enumerate() {
        let Ok(word) = mailbox::receive(0) else { return 90 };
        unsafe { write_volatile(&raw mut mailbox_received[i], word); }
        if word != (((i as u64 + 1) << 32) | u64::from(reading)) { return 91; }
        sum += u64::from(word as u32);
    }
    unsafe { write_volatile(&raw mut mailbox_reading_sum, sum); }
    let status = mailbox::receive(0).err().unwrap_or(0);
    unsafe { write_volatile(&raw mut mailbox_empty_status, status); }
    if status == -11 && sum == 2182 { 52 } else { 92 }
}
