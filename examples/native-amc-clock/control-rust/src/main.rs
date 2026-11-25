#![no_std]
#![no_main]

use core::panic::PanicInfo;
use jnox_native::{clock_sample, ClockError, ClockPairBackend};

#[repr(C)]
pub struct Sample {
    monotonic_ns: u64,
    process_cpu_ns: u64,
    backend: u32,
    error: u32,
}

#[no_mangle]
pub extern "C" fn rust_clock_sample() -> Sample {
    match clock_sample() {
        Ok(pair) => Sample {
            monotonic_ns: pair.monotonic_ns,
            process_cpu_ns: pair.process_cpu_ns,
            backend: match pair.backend {
                ClockPairBackend::Page => 1,
                ClockPairBackend::Syscall => 2,
            },
            error: 0,
        },
        Err(error) => Sample {
            monotonic_ns: 0, process_cpu_ns: 0, backend: 0,
            error: match error {
                ClockError::Errno(code) => code as u32,
                ClockError::InvalidTimespec => 22,
            },
        },
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! { jnox_native::exit(120) }
