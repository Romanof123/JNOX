#![no_std]
#![no_main]

use core::{panic::PanicInfo, sync::atomic::{AtomicU32, Ordering}};
use jnox_native::task_policy;

#[no_mangle]
pub static HEARTBEATS: AtomicU32 = AtomicU32::new(0);

#[no_mangle]
#[inline(never)]
pub extern "C" fn beat_entry() {
    loop {
        if HEARTBEATS.fetch_add(1, Ordering::Relaxed) == 7 {
            let _ = task_policy::exit(25);
            return;
        }
        if task_policy::complete().is_err() {
            let _ = task_policy::exit(97);
            return;
        }
    }
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    beat_entry();
    98
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    let _ = task_policy::exit(99);
    loop { core::hint::spin_loop(); }
}
