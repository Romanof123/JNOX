//! Nonblocking, revocable page loans. Grant values are metadata and do not
//! establish a Rust lifetime for the process's page-table mapping.
use core::{arch::asm, ptr};

pub const PAGE_BYTES: usize = 4096;
pub struct Writer { pub handle: u64, address: usize }
pub struct Reader { pub handle: u64, address: usize, pub length: usize }

impl Writer {
    /// # Safety
    /// The original grant must still be held and writable throughout this
    /// access, including across preemption. Exclude peer-death revocation.
    /// A stale handle or a recycled VA does not establish that condition.
    pub unsafe fn write_byte(&self, offset: usize, byte: u8) {
        assert!(offset < PAGE_BYTES);
        unsafe { ptr::write_volatile((self.address + offset) as *mut u8, byte); }
    }
}
impl Reader {
    /// # Safety
    /// The caller must retain this live read grant throughout the access and
    /// exclude concurrent release, revocation and address reuse. The mapped
    /// page includes a zeroed tail after length. No Rust page reference is made.
    pub unsafe fn read_page_byte(&self, offset: usize) -> u8 {
        assert!(offset < PAGE_BYTES);
        unsafe { ptr::read_volatile((self.address + offset) as *const u8) }
    }
    pub fn address(&self) -> usize { self.address }
}

fn trap(op: u64, mut a: u64, mut b: u64) -> Result<(u64, usize, usize), i64> {
    let address: usize;
    let length: usize;
    unsafe {
        asm!("svc #0x4a01", in("x8") op, inout("x0") a, inout("x1") b,
             lateout("x2") address, lateout("x3") length, options(nostack));
    }
    if a == 0 { Ok((b, address, length)) } else { Err(a as i64) }
}
pub fn acquire() -> Result<Writer, i64> {
    trap(33, 0, 0).map(|(handle, address, _)| Writer { handle, address })
}
pub fn publish(handle: u64, length: usize) -> Result<(), i64> {
    trap(34, handle, length as u64).map(|_| ())
}
pub fn receive() -> Result<Reader, i64> {
    trap(35, 0, 0).map(|(handle, address, length)| Reader { handle, address, length })
}
pub fn release(handle: u64) -> Result<(), i64> {
    trap(36, handle, 0).map(|_| ())
}
