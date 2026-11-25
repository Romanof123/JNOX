//! Synchronous native-v1 register messages. Errors preserve the signed x0 status.
use core::arch::asm;

fn trap(op: u64, mut a: u64, mut b: u64, mut c: u64) -> Result<(u64, u64), i64> {
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") a, inout("x1") b,
             inout("x2") c, options(nostack));
    }
    if a == 0 { Ok((b, c)) } else { Err(a as i64) }
}

/// Wait for a reply until an absolute monotonic deadline in ns. u64::MAX waits indefinitely.
pub fn call(server: u64, word: u64, deadline_ns: u64) -> Result<u64, i64> {
    trap(21, server, word, deadline_ns).map(|(reply, _)| reply)
}

/// Receive a one-use reply token and request word on the current process's endpoint.
pub fn receive() -> Result<(u64, u64), i64> {
    trap(22, 0, 0, 0)
}

pub fn reply(token: u64, word: u64) -> Result<(), i64> {
    trap(23, token, word, 0).map(|_| ())
}
