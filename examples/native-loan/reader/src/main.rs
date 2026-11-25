#![no_std]
#![no_main]

use core::{panic::PanicInfo, ptr::write_volatile};
use jnox_native::{exit, loan};

#[no_mangle]
static mut loan_reads: [[u64; 4]; 8] = [[0; 4]; 8]; // handle, VA, length, byte sum
#[no_mangle]
static mut loan_stale_status: i64 = 0;
#[no_mangle]
static mut loan_final_status: i64 = 0;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut previous = [(0usize, 0u64); 8];
    let mut stale_seen = false;
    for sequence in 1usize..=8 {
        let page = loop {
            match loan::receive() {
                Ok(page) => break page,
                Err(-11) => core::hint::spin_loop(),
                Err(_) => return 90,
            }
        };
        if page.length != 5 + 3 * sequence { return 91; }
        if let Some(&(_, old)) = previous[..sequence-1].iter().find(|p| p.0 == page.address()) {
            if old == page.handle { return 92; }
            let stale = loan::release(old).err().unwrap_or(0);
            unsafe { write_volatile(&raw mut loan_stale_status, stale); }
            if stale >= 0 { return 93; }
            stale_seen = true;
        }
        previous[sequence - 1] = (page.address(), page.handle);
        let mut sum = 0u64;
        for i in 0..loan::PAGE_BYTES {
            // Single reader, no release until after this loop. Producer exit
            // preserves its committed data and the consumer's active mapping.
            let byte = unsafe { page.read_page_byte(i) };
            let expected = if i < page.length { (sequence * 17 + 3 * i) as u8 } else { 0 };
            if byte != expected { return 94; }
            sum += u64::from(byte);
        }
        unsafe { write_volatile(&raw mut loan_reads[sequence - 1],
                                 [page.handle, page.address() as u64, page.length as u64, sum]); }
        if loan::release(page.handle).is_err() { return 95; }
    }
    loop {
        match loan::receive() {
            Err(-11) => core::hint::spin_loop(),
            Err(code) => {
                unsafe { write_volatile(&raw mut loan_final_status, code); }
                return if stale_seen && code == -32 { 74 } else { 96 };
            }
            Ok(_) => return 97,
        }
    }
}
