//! Application services for the J-NOX native AArch64 ABI v1.
#![no_std]

use core::time::Duration;

#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

extern "C" {
    fn clock_gettime(clock: i32, result: *mut Timespec) -> i32;
    fn __jnox_errno_location() -> *mut i32;
    fn __jnox_exit(status: i32) -> !;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    Errno(i32),
    InvalidTimespec,
}

/// Read elapsed monotonic time from the native system clock.
pub fn monotonic_time() -> Result<Duration, ClockError> {
    const CLOCK_MONOTONIC: i32 = 1;
    let mut value = Timespec { seconds: -1, nanoseconds: -1 };
    // The SDK writes a pair of signed LP64 fields to this aligned local.
    if unsafe { clock_gettime(CLOCK_MONOTONIC, &mut value) } != 0 {
        // The C SDK returns a valid errno location for this native application.
        return Err(ClockError::Errno(unsafe { *__jnox_errno_location() }));
    }
    if value.seconds < 0 || !(0..1_000_000_000).contains(&value.nanoseconds) {
        return Err(ClockError::InvalidTimespec);
    }
    Ok(Duration::new(value.seconds as u64, value.nanoseconds as u32))
}

/// Terminate the calling application with its native exit status.
pub fn exit(status: i32) -> ! {
    // The SDK exit stub consumes only its integer status and never returns.
    unsafe { __jnox_exit(status) }
}