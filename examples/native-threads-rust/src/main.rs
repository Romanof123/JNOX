#![no_std]
#![no_main]

use core::ffi::c_void;
use core::panic::PanicInfo;
use core::ptr::{null, null_mut};
use core::sync::atomic::{AtomicU32, Ordering};
use jnox_native::pthread::{pthread_create, pthread_join};
use jnox_native::{exit, yield_now};

#[no_mangle]
static energy_samples: [AtomicU32; 256] = [const { AtomicU32::new(0) }; 256];
#[no_mangle]
static energy_squares: [AtomicU32; 256] = [const { AtomicU32::new(0) }; 256];
#[no_mangle]
static energy_total: AtomicU32 = AtomicU32::new(0);
#[no_mangle]
static energy_main_sum: AtomicU32 = AtomicU32::new(0);

fn result_address() -> *mut c_void {
    core::ptr::addr_of!(energy_total).cast_mut().cast()
}

extern "C" fn square_samples(arg: *mut c_void) -> *mut c_void {
    if arg != result_address() { return null_mut(); }
    let mut total = 0;
    for (i, sample) in energy_samples.iter().enumerate() {
        let x = sample.load(Ordering::Acquire);
        let square = x * x;
        energy_squares[i].store(square, Ordering::Release);
        total += square;
        if (i + 1) % 64 == 0 && yield_now().is_err() { return null_mut(); }
    }
    energy_total.store(total, Ordering::Release);
    arg
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { exit(111) }

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    for (i, sample) in energy_samples.iter().enumerate() {
        sample.store(i as u32, Ordering::Release);
    }
    let mut handle = 0;
    // The output object is live for the call. The worker receives the stable
    // address of a static atomic and accesses shared data only through atomics.
    if unsafe { pthread_create(&mut handle, null(), Some(square_samples), result_address()) } != 0 {
        return 112;
    }
    let sum = energy_samples.iter().map(|s| s.load(Ordering::Acquire)).sum();
    energy_main_sum.store(sum, Ordering::Release);
    let mut result = null_mut();
    // The SDK writes this live, aligned local on successful join.
    if unsafe { pthread_join(handle, &mut result) } != 0 || result != result_address() {
        return 113;
    }
    for (i, square) in energy_squares.iter().enumerate() {
        if square.load(Ordering::Acquire) != (i * i) as u32 { return 114; }
    }
    if sum != 32640 || energy_total.load(Ordering::Acquire) != 5559680 { return 115; }
    29
}
