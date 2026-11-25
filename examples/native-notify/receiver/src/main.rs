#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, notify};

#[no_mangle]
static mut notify_codes: [u32; 4] = [0; 4];
#[no_mangle]
static mut notify_words: [u64; 4] = [0; 4];
#[no_mangle]
static mut notify_channel_sums: [u64; 2] = [0; 2];
#[no_mangle]
static mut notify_empty_pulse: i64 = i64::MAX;
#[no_mangle]
static mut notify_event_results: [i64; 4] = [i64::MAX; 4];

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut sums = [0u64; 2];
    let readings = [300u32, 512, 257, 1025];
    for (i, reading) in readings.into_iter().enumerate() {
        let Ok((code, word)) = notify::pulse_recv(0) else { return 84 };
        unsafe {
            write_volatile(&raw mut notify_codes[i], code);
            write_volatile(&raw mut notify_words[i], word);
        }
        let channel = match code { 17 => 0, 23 => 1, _ => return 85 };
        if channel != i / 2 || word != (((i as u64 + 1) << 32) | u64::from(reading)) {
            return 86;
        }
        sums[channel] += u64::from(word as u32);
    }
    let empty = notify::pulse_recv(0).err().unwrap_or(0);
    unsafe {
        write_volatile(&raw mut notify_channel_sums, sums);
        write_volatile(&raw mut notify_empty_pulse, empty);
    }
    if empty != -11 || sums != [812, 1282] { return 87; }

    for (i, (mask, expected)) in [(1, 1), (1, -11), (12, 12), (u32::MAX, -11)]
        .into_iter().enumerate()
    {
        let observed = notify::event_take(mask, 0).map(i64::from).unwrap_or_else(|err| err);
        unsafe { write_volatile(&raw mut notify_event_results[i], observed); }
        if observed != expected { return 88; }
    }
    62
}
