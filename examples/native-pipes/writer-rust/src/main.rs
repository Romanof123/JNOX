#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, pipes::{self, Access, OwnedFd}};

#[no_mangle]
static mut rust_writer_crc: [u32; 8] = [0; 8];
#[no_mangle]
static mut rust_write_count: u32 = 0;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main() -> i32 {
    // The manifest provisions exactly these descriptions in slot 1.
    let gate = unsafe { OwnedFd::from_raw_fd(3) };
    let ack = unsafe { OwnedFd::from_raw_fd(4) };
    let mut start = [0u8];
    if gate.read(&mut start) != Ok(1) || start != [b'w'] { return 90; }
    let out = match pipes::open_fifo(b"/samples", Access::Write, 0) {
        Ok(fd) => fd,
        Err(_) => return 91,
    };
    if ack.write(b"a") != Ok(1) { return 92; }
    for seq in 0..8usize {
        let mut record = [0u8; 512];
        for (i, byte) in record[..508].iter_mut().enumerate() {
            *byte = (97 + seq * 31 + i * 17 + (i >> 3)) as u8;
        }
        record[0] = 1;
        record[1] = seq as u8;
        let mut crc = u32::MAX;
        for &byte in &record[..508] {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = (crc >> 1) ^ ((0u32.wrapping_sub(crc & 1)) & 0xedb88320);
            }
        }
        let crc = !crc;
        record[508..].copy_from_slice(&crc.to_le_bytes());
        if out.write(&record) != Ok(512) { return 93; }
        unsafe {
            write_volatile((&raw mut rust_writer_crc).cast::<u32>().add(seq), crc);
            write_volatile(&raw mut rust_write_count, seq as u32 + 1);
        }
    }
    if out.close().is_err() { return 94; }
    62
}
