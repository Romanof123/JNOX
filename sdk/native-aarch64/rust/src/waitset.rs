//! Readiness observations leave the registered mailbox, pulse or event untouched.
use core::arch::asm;

#[derive(Clone, Copy)]
#[repr(u64)]
pub enum Source { Mailbox = 1, Pulse = 2, Event = 3 }

fn trap(op: u64, mut a: u64, mut b: u64, mut c: u64) -> Result<(u64, u64), i64> {
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") a, inout("x1") b,
             inout("x2") c, options(nostack));
    }
    if a == 0 { Ok((b, c)) } else { Err(a as i64) }
}

pub fn attach(owner: u64, source: Source, mask: u32) -> Result<u64, i64> {
    trap(30, owner, source as u64, u64::from(mask)).map(|(handle, _)| handle)
}
pub fn detach(handle: u64) -> Result<(), i64> {
    trap(31, handle, 0, 0).map(|_| ())
}
/// Zero polls; u64::MAX waits indefinitely; otherwise absolute monotonic ns.
pub fn wait(deadline_ns: u64) -> Result<(u64, u64), i64> {
    trap(32, deadline_ns, 0, 0)
}
