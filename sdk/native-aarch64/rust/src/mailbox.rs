//! Copied words. Deadline is absolute monotonic ns; zero tries immediately
//! and u64::MAX waits indefinitely. Errors retain the signed kernel status.
use core::arch::asm;

pub fn send(owner: u64, word: u64, deadline_ns: u64) -> Result<(), i64> {
    let status: i64;
    unsafe {
        asm!("svc #0x4a01", in("x8") 24u64, inout("x0") owner => status,
             inout("x1") word => _, inout("x2") deadline_ns => _, options(nostack));
    }
    if status == 0 { Ok(()) } else { Err(status) }
}

pub fn receive(deadline_ns: u64) -> Result<u64, i64> {
    let status: i64;
    let word: u64;
    unsafe {
        asm!("svc #0x4a01", in("x8") 25u64, inout("x0") deadline_ns => status,
             lateout("x1") word, lateout("x2") _, options(nostack));
    }
    if status == 0 { Ok(word) } else { Err(status) }
}
