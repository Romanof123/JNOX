#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use core::slice;
use jnox_native::{exit, yield_now};

#[no_mangle]
static mut sensor_argc: u32 = 0;
#[no_mangle]
static mut sensor_lengths: [u32; 16] = [0; 16];
#[no_mangle]
static mut sensor_label: [u8; 1024] = [0; 1024];
#[no_mangle]
static mut sensor_count: u32 = 0;
#[no_mangle]
static mut sensor_sum: u64 = 0;
#[no_mangle]
static mut sensor_yields: u32 = 0;
static mut RAMP: [u8; 256] = [0; 256];

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    exit(120)
}

// The native startup ABI supplies mapped, NUL-terminated argument strings.
unsafe fn argument<'a>(ptr: *const u8) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return None;
    }
    for len in 0..1024 {
        if read_volatile(ptr.add(len)) == 0 {
            return Some(slice::from_raw_parts(ptr, len));
        }
    }
    None
}

fn endpoint(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }
    let mut n = 0;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        n = n * 10 + u32::from(byte - b'0');
        if n > 256 {
            return None;
        }
    }
    Some(n)
}

#[no_mangle]
pub unsafe extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if !(1..=16).contains(&argc) || argv.is_null() {
        return 121;
    }
    let count = argc as usize;
    if !(*argv.add(count)).is_null() || !(*argv.add(count + 1)).is_null() {
        return 122;
    }
    write_volatile(addr_of_mut!(sensor_argc), argc as u32);
    let mut args: [&[u8]; 16] = [&[]; 16];
    for (i, field) in args[..count].iter_mut().enumerate() {
        let Some(bytes) = argument(*argv.add(i)) else { return 123 };
        *field = bytes;
        write_volatile(addr_of_mut!(sensor_lengths[i]), bytes.len() as u32);
    }
    if args[0] != b"/bin/app" {
        return 124;
    }
    if count == 1 {
        return 0;
    }
    if count != 5 || args[1] != b"sum" {
        return 125;
    }
    let (Some(low), Some(high)) = (endpoint(args[2]), endpoint(args[3])) else { return 126 };
    if low >= high {
        return 127;
    }
    let label = args[4];
    for (i, &byte) in label.iter().enumerate() {
        write_volatile(addr_of_mut!(sensor_label[i]), byte);
    }
    for i in 0..256 {
        write_volatile(addr_of_mut!(RAMP[i]), i as u8);
    }
    let mut total = 0u64;
    for i in low..high {
        total += u64::from(read_volatile(addr_of!(RAMP[i as usize])));
        write_volatile(addr_of_mut!(sensor_sum), total);
        let done = i + 1 - low;
        write_volatile(addr_of_mut!(sensor_count), done);
        if done % 4 == 0 {
            let mut saved = 0;
            write_volatile(addr_of_mut!(saved), low + done);
            if yield_now().is_err() || read_volatile(addr_of!(saved)) != i + 1 {
                return 128;
            }
            for j in 0..=label.len() {
                if read_volatile((*argv.add(4)).add(j)) != read_volatile(addr_of!(sensor_label[j])) {
                    return 129;
                }
            }
            write_volatile(addr_of_mut!(sensor_yields), done / 4);
        }
    }
    if total == u64::from(high - low) * u64::from(low + high - 1) / 2 { 43 } else { 130 }
}
