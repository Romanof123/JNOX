#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, mailbox, monotonic_time, notify, waitset::{self, Source}};

#[no_mangle]
static mut waitset_handles: [u64; 5] = [0; 5]; // first event, mailbox, filler event, pulse, replacement
#[no_mangle]
static mut waitset_observations: [[u64; 2]; 5] = [[0; 2]; 5];
#[no_mangle]
static mut waitset_errors: [i64; 5] = [0; 5]; // foreign attach, stale detach, empty replacement, final poll, timeout
#[no_mangle]
static mut waitset_products: [u64; 4] = [0; 4]; // mailbox word, pulse code, pulse word, reading sum

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

fn expect(index: usize, deadline: u64, handle: u64, ready: u64) -> bool {
    let Ok((actual, value)) = waitset::wait(deadline) else { return false };
    unsafe { write_volatile(&raw mut waitset_observations[index], [actual, value]); }
    (actual, value) == (handle, ready)
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let foreign = waitset::attach(1, Source::Mailbox, 0).err().unwrap_or(0);
    unsafe { write_volatile(&raw mut waitset_errors[0], foreign); }
    if foreign >= 0 { return 86; }
    let Ok(first) = waitset::attach(0, Source::Event, 1) else { return 87 };
    if !expect(0, u64::MAX, first, 1) || notify::event_take(1, 0) != Ok(1) { return 88; }

    let Ok(mail) = waitset::attach(0, Source::Mailbox, 0) else { return 89 };
    if !expect(1, 0, mail, 1) || !expect(2, 0, mail, 1) { return 90; }
    let Ok(word) = mailbox::receive(0) else { return 91 };
    if word != 0x000000010000012c || waitset::wait(0) != Err(-11) { return 92; }

    // Fill the table so only the detached pulse slot can take the new event.
    let Ok(filler) = waitset::attach(0, Source::Event, 16) else { return 93 };
    let Ok(old) = waitset::attach(0, Source::Pulse, 0) else { return 94 };
    if !expect(3, 0, old, 1) || waitset::detach(old).is_err() { return 95; }
    let Ok(new) = waitset::attach(0, Source::Event, 8) else { return 96 };
    unsafe { write_volatile(&raw mut waitset_handles, [first, mail, filler, old, new]); }
    let stale = waitset::detach(old).err().unwrap_or(0);
    let empty = waitset::wait(0).err().unwrap_or(0);
    unsafe {
        write_volatile(&raw mut waitset_errors[1], stale);
        write_volatile(&raw mut waitset_errors[2], empty);
    }
    if old == new || stale >= 0 || empty != -11 { return 97; }
    if !expect(4, u64::MAX, new, 8) || notify::event_take(8, 0) != Ok(8) { return 98; }
    if notify::event_take(4, 0) != Ok(4) { return 99; }
    let Ok((code, pulse)) = notify::pulse_recv(0) else { return 100 };
    let sum = u64::from(word as u32) + u64::from(pulse as u32);
    unsafe { write_volatile(&raw mut waitset_products, [word, u64::from(code), pulse, sum]); }
    if code != 17 || pulse != 0x0000000200000200 || sum != 812 { return 101; }
    let final_poll = waitset::wait(0).err().unwrap_or(0);
    let Ok(now) = monotonic_time() else { return 102 };
    let Ok(ns) = u64::try_from(now.as_nanos()) else { return 103 };
    let Some(deadline) = ns.checked_add(20_000_000) else { return 104 };
    let timeout = waitset::wait(deadline).err().unwrap_or(0);
    unsafe {
        write_volatile(&raw mut waitset_errors[3], final_poll);
        write_volatile(&raw mut waitset_errors[4], timeout);
    }
    if final_poll == -11 && timeout == -110 { 71 } else { 105 }
}
