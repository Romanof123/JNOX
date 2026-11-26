//! Nonblocking raw Ethernet access for the packaged native NIC owner.
use core::arch::asm;

pub const MAX_FRAME: usize = 1514;
const MIN_FRAME: usize = 14;
const EIO: i64 = -5;

fn call(op: u64, address: usize, len: usize) -> Result<usize, i64> {
    let status: i64;
    // The kernel copies synchronously and clears x1. Permit memory effects
    // so the compiler preserves transmit bytes and reloads received bytes.
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inlateout("x0") address => status,
             inlateout("x1") len => _, options(nostack));
    }
    if status < 0 { Err(status) } else { Ok(status as usize) }
}

/// Receive one complete Ethernet frame, or return -EAGAIN if the queue is empty.
pub fn receive(buffer: &mut [u8; MAX_FRAME]) -> Result<&[u8], i64> {
    let len = call(38, buffer.as_mut_ptr() as usize, buffer.len())?;
    if !(MIN_FRAME..=MAX_FRAME).contains(&len) { return Err(EIO); }
    Ok(&buffer[..len])
}

/// Submit one complete frame. On -EAGAIN the caller can retry the same bytes.
/// The kernel checks owner authority and the 14..=1514 byte length.
pub fn transmit(frame: &[u8]) -> Result<(), i64> {
    let len = call(39, frame.as_ptr() as usize, frame.len())?;
    if len != frame.len() || !(MIN_FRAME..=MAX_FRAME).contains(&len) { return Err(EIO); }
    Ok(())
}

/// Claim the replacement epoch as the designated standby.
/// Returns -EAGAIN while the original owner lives, -EACCES for other slots,
/// and -EALREADY if this standby already owns the device.
pub fn claim() -> Result<(), i64> {
    if call(76, 0, 0)? == 0 { Ok(()) } else { Err(EIO) }
}
