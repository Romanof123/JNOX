//! Application services for the J-NOX native AArch64 ABI v1.
#![no_std]

pub mod pthread;

use core::time::Duration;

#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

extern "C" {
    fn jnox_deadline_service_wait_next() -> i32;
    fn jnox_periodic_wait_next() -> i32;
    fn sched_yield() -> i32;
    fn clock_gettime(clock: i32, result: *mut Timespec) -> i32;
    fn clock_nanosleep(clock: i32, flags: i32, request: *const Timespec, remaining: *mut Timespec) -> i32;
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

/// Sleep until an absolute deadline on the clock returned by `monotonic_time`.
pub fn sleep_until(deadline: Duration) -> Result<(), ClockError> {
    const EOVERFLOW: i32 = 75;
    const CLOCK_MONOTONIC: i32 = 1;
    const TIMER_ABSTIME: i32 = 1;
    let request = Timespec {
        seconds: i64::try_from(deadline.as_secs()).map_err(|_| ClockError::Errno(EOVERFLOW))?,
        nanoseconds: i64::from(deadline.subsec_nanos()),
    };
    // The SDK reads this initialized request and ignores remaining for absolute waits.
    let code = unsafe { clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &request, core::ptr::null_mut()) };
    if code == 0 { Ok(()) } else { Err(ClockError::Errno(code)) }
}

/// Give another ready application a turn. An error carries the native errno.
pub fn yield_now() -> Result<(), i32> {
    if unsafe { sched_yield() } == 0 {
        Ok(())
    } else {
        Err(unsafe { *__jnox_errno_location() })
    }
}

/// Wait for the next reserved deadline-service cell and return its ordinal.
pub fn deadline_service_wait_next() -> Result<u32, i32> {
    let ordinal = unsafe { jnox_deadline_service_wait_next() };
    u32::try_from(ordinal).map_err(|_| unsafe { *__jnox_errno_location() })
}

/// Complete this generation and wait for the next release under RM or DM.
pub fn wait_next_period() -> Result<(), i32> {
    if unsafe { jnox_periodic_wait_next() } == 0 {
        Ok(())
    } else {
        Err(unsafe { *__jnox_errno_location() })
    }
}

/// Terminate the calling application with its native exit status.
pub fn exit(status: i32) -> ! {
    // The SDK exit stub consumes only its integer status and never returns.
    unsafe { __jnox_exit(status) }
}