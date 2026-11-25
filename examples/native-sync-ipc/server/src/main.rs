#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, ipc};

#[no_mangle]
static mut ipc_server_words: [u64; 3] = [0; 3]; // token, received word, computed summary
#[no_mangle]
static mut ipc_reply_statuses: [i64; 3] = [i64::MAX; 3]; // invalid, valid, duplicate

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let (token, word) = match ipc::receive() {
        Ok(request) => request,
        Err(_) => return 83,
    };
    let mut sum = 0u64;
    let mut min = u16::MAX;
    let mut max = 0u16;
    for i in 0..4 {
        let sample = (word >> (16 * i)) as u16;
        sum += u64::from(sample);
        min = min.min(sample);
        max = max.max(sample);
    }
    if sum > u64::from(u16::MAX) { return 84; }
    let summary = (4 << 48) | (u64::from(max) << 32) | (u64::from(min) << 16) | sum;
    unsafe { write_volatile(&raw mut ipc_server_words, [token, word, summary]); }
    let invalid = ipc::reply(!token, 0).err().unwrap_or(0);
    let valid = ipc::reply(token, summary).err().unwrap_or(0);
    let duplicate = ipc::reply(token, summary).err().unwrap_or(0);
    unsafe { write_volatile(&raw mut ipc_reply_statuses, [invalid, valid, duplicate]); }
    if invalid != -116 { return 85; }
    if valid != 0 { return 86; }
    if duplicate != -116 { return 87; }
    42
}
