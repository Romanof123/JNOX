//! Process-local mailbox capabilities. The kernel validates every raw handle.
use core::arch::asm;

pub const SEND: u64 = 1;
pub const DELEGATE: u64 = 2;

#[derive(Debug, PartialEq, Eq)]
pub struct Handle(u64);
impl Handle {
    pub fn raw(&self) -> u64 { self.0 }
    /// Wrap a raw process-local handle. This does not install or duplicate
    /// an authority; an expired or foreign value is rejected by the kernel.
    pub fn from_raw(raw: u64) -> Self { Self(raw) }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Notice {
    pub handle: Handle,
    pub tag: u64,
    pub sender: u64,
    pub target: u64,
    pub rights: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub handles: u64,
    pub notices: u64,
    pub peak_handles: u64,
    pub peak_notices: u64,
}

fn trap(op: u64, mut a: u64, mut b: u64, mut c: u64, mut d: u64, mut e: u64)
    -> Result<[u64; 5], i64>
{
    let last: u64;
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") a, inout("x1") b,
             inout("x2") c, inout("x3") d, inout("x4") e, lateout("x5") last,
             options(nostack));
    }
    if a == 0 { Ok([b, c, d, e, last]) } else { Err(a as i64) }
}

pub fn initial(index: u64) -> Result<Handle, i64> {
    trap(53, index, 0, 0, 0, 0).map(|v| Handle(v[0]))
}
pub fn copy(h: &Handle, dst: u64, rights: u64, tag: u64) -> Result<(), i64> {
    trap(54, h.raw(), dst, rights, 0, tag).map(|_| ())
}
/// A successful move retires the source. On failure, return it to the caller.
pub fn move_to(h: Handle, dst: u64, rights: u64, tag: u64) -> Result<(), (i64, Handle)> {
    trap(54, h.raw(), dst, rights, 1, tag).map(|_| ()).map_err(|code| (code, h))
}
pub fn receive() -> Result<Notice, i64> {
    let [raw, tag, sender, target, rights] = trap(55, 0, 0, 0, 0, 0)?;
    Ok(Notice { handle: Handle(raw), tag, sender, target, rights })
}
/// Deadline is absolute monotonic nanoseconds; zero tries once, u64::MAX waits.
pub fn send(h: &Handle, word: u64, deadline_ns: u64) -> Result<(), i64> {
    trap(56, h.raw(), word, deadline_ns, 0, 0).map(|_| ())
}
pub fn close(h: Handle) -> Result<(), i64> {
    trap(57, h.raw(), 0, 0, 0, 0).map(|_| ())
}
pub fn observe() -> Result<Counts, i64> {
    let [handles, notices, peak_handles, peak_notices, _] = trap(58, 0, 0, 0, 0, 0)?;
    Ok(Counts { handles, notices, peak_handles, peak_notices })
}
