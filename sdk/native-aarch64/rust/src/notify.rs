//! Four-entry pulse FIFO and selective event latch. Receive deadlines are
//! absolute monotonic ns; zero tries once and u64::MAX waits indefinitely.
use core::arch::asm;

fn trap(op: u64, mut a: u64, mut b: u64, mut c: u64) -> Result<(u64, u64), i64> {
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") a, inout("x1") b,
             inout("x2") c, options(nostack));
    }
    if a == 0 { Ok((b, c)) } else { Err(a as i64) }
}

pub fn pulse_send(owner: u64, code: u32, word: u64) -> Result<(), i64> {
    trap(26, owner, u64::from(code), word).map(|_| ())
}

pub fn pulse_recv(deadline_ns: u64) -> Result<(u32, u64), i64> {
    trap(27, deadline_ns, 0, 0).map(|(code, word)| (code as u32, word))
}

pub fn event_set(owner: u64, mask: u32) -> Result<(), i64> {
    trap(28, owner, u64::from(mask), 0).map(|_| ())
}

pub fn event_take(mask: u32, deadline_ns: u64) -> Result<u32, i64> {
    trap(29, u64::from(mask), deadline_ns, 0).map(|(bits, _)| bits as u32)
}
