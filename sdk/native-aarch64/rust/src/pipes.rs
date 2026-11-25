//! Native pipe descriptors. Errors preserve negative kernel errno values.
//! Each OwnedFd closes its descriptor on drop; duplication creates an
//! independent descriptor that refers to the same open description.
use core::{ffi::c_void, mem::ManuallyDrop};

pub const PIPE_BUF: usize = 512;
pub const CAPACITY: usize = 2048;
pub const O_NONBLOCK: i32 = 0x800;
pub const O_CLOEXEC: i32 = 0x80000;
pub const FD_CLOEXEC: i32 = 1;
#[repr(i32)]
#[derive(Clone, Copy)]
pub enum Access { Read = 0, Write = 1 }

mod c {
    use super::c_void;
    extern "C" {
        pub fn pipe2(fds: *mut i32, flags: i32) -> i32;
        pub fn dup(fd: i32) -> i32;
        pub fn dup2(fd: i32, target: i32) -> i32;
        pub fn fcntl(fd: i32, command: i32, ...) -> i32;
        pub fn read(fd: i32, buf: *mut c_void, len: usize) -> isize;
        pub fn write(fd: i32, buf: *const c_void, len: usize) -> isize;
        pub fn close(fd: i32) -> i32;
        pub fn jnox_pipe_name_op(op: i32, path: *const u8, len: usize, flags: i32) -> isize;
    }
}

fn status(raw: i32) -> Result<i32, i32> {
    if raw < 0 { Err(-unsafe { *super::__jnox_errno_location() }) } else { Ok(raw) }
}
fn bytes(raw: isize) -> Result<usize, i32> {
    if raw < 0 { Err(-unsafe { *super::__jnox_errno_location() }) } else { Ok(raw as usize) }
}

#[derive(Debug)]
pub struct OwnedFd(i32);
impl OwnedFd {
    pub fn as_raw_fd(&self) -> i32 { self.0 }
    /// # Safety
    /// Supply one live pipe descriptor owned by this process. Transfer its
    /// unique closing responsibility here and never separately close it.
    pub unsafe fn from_raw_fd(fd: i32) -> Self { Self(fd) }
    pub fn into_raw_fd(self) -> i32 { ManuallyDrop::new(self).0 }
    pub fn dup(&self) -> Result<Self, i32> { status(unsafe { c::dup(self.0) }).map(Self) }
    /// Atomically replace the target's descriptor while retaining its number.
    pub fn dup2(&self, target: &mut Self) -> Result<(), i32> {
        status(unsafe { c::dup2(self.0, target.0) }).map(|_| ())
    }
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, i32> {
        bytes(unsafe { c::read(self.0, buf.as_mut_ptr().cast(), buf.len()) })
    }
    pub fn write(&self, buf: &[u8]) -> Result<usize, i32> {
        bytes(unsafe { c::write(self.0, buf.as_ptr().cast(), buf.len()) })
    }
    pub fn fd_flags(&self) -> Result<i32, i32> { status(unsafe { c::fcntl(self.0, 1) }) }
    pub fn set_fd_flags(&self, flags: i32) -> Result<(), i32> {
        status(unsafe { c::fcntl(self.0, 2, flags) }).map(|_| ())
    }
    pub fn status_flags(&self) -> Result<i32, i32> { status(unsafe { c::fcntl(self.0, 3) }) }
    pub fn set_status_flags(&self, flags: i32) -> Result<(), i32> {
        status(unsafe { c::fcntl(self.0, 4, flags) }).map(|_| ())
    }
    pub fn close(self) -> Result<(), i32> {
        status(unsafe { c::close(self.into_raw_fd()) }).map(|_| ())
    }
}
impl Drop for OwnedFd {
    fn drop(&mut self) { unsafe { c::close(self.0); } }
}

pub fn pipe() -> Result<(OwnedFd, OwnedFd), i32> { pipe2(0) }
pub fn pipe2(flags: i32) -> Result<(OwnedFd, OwnedFd), i32> {
    let mut ends = [-1; 2];
    status(unsafe { c::pipe2(ends.as_mut_ptr(), flags) })?;
    Ok((OwnedFd(ends[0]), OwnedFd(ends[1])))
}

fn named(op: i32, path: &[u8], flags: i32) -> Result<i32, i32> {
    if path.is_empty() || path.len() > 31 || path.contains(&0) { return Err(-22); }
    let raw = unsafe { c::jnox_pipe_name_op(op, path.as_ptr(), path.len(), flags) };
    if raw < 0 { Err(raw as i32) } else { Ok(raw as i32) }
}
pub fn mkfifo(path: &[u8]) -> Result<(), i32> { named(43, path, 0).map(|_| ()) }
pub fn unlink(path: &[u8]) -> Result<(), i32> { named(44, path, 0).map(|_| ()) }
pub fn open_fifo(path: &[u8], access: Access, flags: i32) -> Result<OwnedFd, i32> {
    if flags & !(O_NONBLOCK | O_CLOEXEC) != 0 { return Err(-22); }
    named(45, path, access as i32 | flags).map(OwnedFd)
}
