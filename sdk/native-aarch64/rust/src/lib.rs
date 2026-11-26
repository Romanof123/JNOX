//! Application services for the J-NOX native AArch64 ABI v1.
#![no_std]

pub mod pthread;
#[cfg(target_arch = "aarch64")]
pub mod task_policy;

use core::time::Duration;

#[repr(C)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

extern "C" {
    fn jnox_deadline_service_wait_next() -> i32;
    fn jnox_periodic_wait_next() -> i32;
    fn jnox_clock_sample() -> RawClockPair;
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

#[repr(C)]
struct RawClockPair {
    monotonic_ns: u64,
    process_cpu_ns: u64,
    backend: u32,
    error: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockPairBackend {
    Page,
    Syscall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockPair {
    pub monotonic_ns: u64,
    pub process_cpu_ns: u64,
    pub backend: ClockPairBackend,
}

/// Read both clocks at one counter instant on a negotiated native time page.
/// The syscall fallback reads process CPU time first and monotonic time second.
pub fn clock_sample() -> Result<ClockPair, ClockError> {
    let pair = unsafe { jnox_clock_sample() };
    let backend = match pair.backend {
        1 => ClockPairBackend::Page,
        2 => ClockPairBackend::Syscall,
        _ => return Err(ClockError::Errno(pair.error as i32)),
    };
    Ok(ClockPair { monotonic_ns: pair.monotonic_ns, process_cpu_ns: pair.process_cpu_ns, backend })
}

const CLOCK_MONOTONIC: i32 = 1;
const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;

/// Read elapsed monotonic time from the native system clock.
pub fn monotonic_time() -> Result<Duration, ClockError> {
    read_clock(CLOCK_MONOTONIC)
}

/// Read CPU time charged to the current native AMC process generation.
pub fn process_cpu_time() -> Result<Duration, ClockError> {
    read_clock(CLOCK_PROCESS_CPUTIME_ID)
}

fn read_clock(clock: i32) -> Result<Duration, ClockError> {
    let mut value = Timespec { tv_sec: -1, tv_nsec: -1 };
    // The SDK writes a pair of signed LP64 fields to this aligned local.
    if unsafe { clock_gettime(clock, &mut value) } != 0 {
        // The C SDK returns a valid errno location for this native application.
        return Err(ClockError::Errno(unsafe { *__jnox_errno_location() }));
    }
    if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
        return Err(ClockError::InvalidTimespec);
    }
    Ok(Duration::new(value.tv_sec as u64, value.tv_nsec as u32))
}

/// Sleep until an absolute deadline on the clock returned by `monotonic_time`.
pub fn sleep_until(deadline: Duration) -> Result<(), ClockError> {
    const EOVERFLOW: i32 = 75;
    const TIMER_ABSTIME: i32 = 1;
    let request = Timespec {
        tv_sec: i64::try_from(deadline.as_secs()).map_err(|_| ClockError::Errno(EOVERFLOW))?,
        tv_nsec: i64::from(deadline.subsec_nanos()),
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
#[cfg(all(feature = "sync-ipc", target_arch = "aarch64"))]
pub mod ipc;

#[cfg(all(feature = "mailbox", target_arch = "aarch64"))]
pub mod mailbox;

#[cfg(all(feature = "notify", target_arch = "aarch64"))]
pub mod notify;

#[cfg(all(feature = "waitset", target_arch = "aarch64"))]
pub mod waitset;

#[cfg(all(feature = "loan", target_arch = "aarch64"))]
pub mod loan;

#[cfg(all(feature = "pipes", target_arch = "aarch64"))]
pub mod pipes;

#[cfg(all(feature = "local-socket", target_arch = "aarch64"))]
pub mod local_socket;

#[cfg(all(feature = "capabilities", target_arch = "aarch64"))]
pub mod capabilities;
#[cfg(all(feature = "pubsub", target_arch = "aarch64"))]
pub mod pubsub;
#[cfg(all(feature = "display", target_arch = "aarch64"))]
pub mod display;

#[cfg(all(feature = "raw-ethernet", target_arch = "aarch64"))]
pub mod raw_ethernet;
