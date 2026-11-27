//! Bounded AF_UNIX stream ancillary data and shared backing pages.
use super::{count, result, MSG_CTRUNC};
use core::{ffi::c_void, mem::size_of, ptr};

#[repr(C)]
struct Iov { base: *mut u8, len: usize }
#[repr(C)]
struct Control {
    len: usize,
    level: i32,
    kind: i32,
    fd: i32,
    padding: i32,
}
impl Control {
    fn right(fd: i32) -> Self {
        Self { len: 20, level: 1, kind: 1, fd, padding: 0 }
    }
}
#[repr(C)]
struct Message {
    name: *mut c_void,
    name_len: u32,
    iov: *mut Iov,
    iov_len: usize,
    control: *mut Control,
    control_len: usize,
    flags: i32,
}
impl Message {
    fn new(iov: &mut Iov, control: Option<&mut Control>) -> Self {
        let (ptr, len) = match control {
            Some(c) => (c as *mut Control, size_of::<Control>()),
            None => (ptr::null_mut(), 0),
        };
        Self { name: ptr::null_mut(), name_len: 0, iov, iov_len: 1,
               control: ptr, control_len: len, flags: 0 }
    }
}
const _: () = assert!(size_of::<Control>() == 24 && size_of::<Message>() == 56);

mod c {
    use super::{c_void, Message};
    extern "C" {
        pub(super) fn sendmsg(fd: i32, msg: *const Message, flags: i32) -> isize;
        pub(super) fn recvmsg(fd: i32, msg: *mut Message, flags: i32) -> isize;
        pub(super) fn jnox_shm_create(size: usize) -> i32;
        pub(super) fn jnox_shm_dup_readonly(fd: i32) -> i32;
        pub(super) fn jnox_shm_size(fd: i32) -> isize;
        pub(super) fn jnox_shm_map(fd: i32, prot: i32) -> *mut c_void;
        pub(super) fn jnox_shm_unmap(address: *mut c_void) -> i32;
    }
}

/// A positive short send has already transferred the optional backing FD.
pub fn sendmsg(fd: i32, bytes: &[u8], backing: Option<i32>, flags: i32) -> Result<usize, i32> {
    let mut iov = Iov { base: bytes.as_ptr().cast_mut(), len: bytes.len() };
    let mut control = backing.map(Control::right);
    let msg = Message::new(&mut iov, control.as_mut());
    count(unsafe { c::sendmsg(fd, &msg, flags) })
}

#[derive(Debug)]
pub struct ReceivedMessage {
    pub count: usize,
    /// Receiver-local FD. The caller is responsible for closing it.
    pub fd: Option<i32>,
    pub control_truncated: bool,
}
pub fn recvmsg(fd: i32, bytes: &mut [u8], accept_fd: bool, flags: i32) -> Result<ReceivedMessage, i32> {
    let mut iov = Iov { base: bytes.as_mut_ptr(), len: bytes.len() };
    let mut control = Control::right(-1);
    let mut msg = Message::new(&mut iov, accept_fd.then_some(&mut control));
    let n = count(unsafe { c::recvmsg(fd, &mut msg, flags) })?;
    Ok(ReceivedMessage { count: n, fd: (msg.control_len != 0).then_some(control.fd),
                         control_truncated: msg.flags & MSG_CTRUNC != 0 })
}

/// Create a zero-initialized shared backing of 1..=307200 bytes. Use close(fd) to release it.
pub fn shm_create(length: usize) -> Result<i32, i32> {
    result(unsafe { c::jnox_shm_create(length) })
}
/// The new local descriptor may only map read-only, including after transfer.
pub fn shm_dup_readonly(fd: i32) -> Result<i32, i32> {
    result(unsafe { c::jnox_shm_dup_readonly(fd) })
}
pub fn shm_size(fd: i32) -> Result<usize, i32> {
    count(unsafe { c::jnox_shm_size(fd) })
}

/// Owns a process-local mapping independently of the source descriptor.
/// No Rust slice or reference to the shared backing is created.
pub struct ShmMapping { address: usize, len: usize, writable: bool }
pub fn shm_map(fd: i32, writable: bool) -> Result<ShmMapping, i32> {
    let len = shm_size(fd)?;
    let address = unsafe { c::jnox_shm_map(fd, if writable { 3 } else { 1 }) } as usize;
    if address == usize::MAX {
        return Err(-unsafe { *crate::__jnox_errno_location() });
    }
    Ok(ShmMapping { address, len, writable })
}
impl ShmMapping {
    pub fn address(&self) -> usize { self.address }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_writable(&self) -> bool { self.writable }

    /// # Safety
    /// Keep this mapping live and exclude unsynchronized writes to this byte
    /// through every alias, including mappings in other processes.
    pub unsafe fn read_byte(&self, offset: usize) -> Result<u8, i32> {
        if offset >= self.len { return Err(-22); }
        Ok(unsafe { ptr::read_volatile((self.address + offset) as *const u8) })
    }
    /// # Safety
    /// Keep this mapping live and exclude unsynchronized reads and writes to
    /// this byte through every alias, including mappings in other processes.
    pub unsafe fn write_byte(&self, offset: usize, byte: u8) -> Result<(), i32> {
        if !self.writable { return Err(-13); }
        if offset >= self.len { return Err(-22); }
        unsafe { ptr::write_volatile((self.address + offset) as *mut u8, byte) };
        Ok(())
    }
    /// Release explicitly to observe the kernel status. Drop also releases a live mapping.
    pub fn unmap(mut self) -> Result<(), i32> {
        let address = core::mem::replace(&mut self.address, 0);
        result(unsafe { c::jnox_shm_unmap(address as *mut c_void) }).map(|_| ())
    }
}
impl Drop for ShmMapping {
    fn drop(&mut self) {
        if self.address != 0 {
            unsafe { c::jnox_shm_unmap(self.address as *mut c_void) };
        }
    }
}
