//! Bounded native `/tmp` files. Errors are negative JNOX errno values.
//! Each operation uses the package's mount grant and its configured EL0 service.

mod c {
    extern "C" {
        pub fn jnox_tmpfs_native_open(path: *const u8, len: usize, how: u32) -> i64;
        pub fn jnox_tmpfs_native_unlink(path: *const u8, len: usize) -> i64;
        pub fn jnox_tmpfs_native_read(fd: i32, buf: *mut u8, len: usize) -> i64;
        pub fn jnox_tmpfs_native_write(fd: i32, buf: *const u8, len: usize) -> i64;
        pub fn jnox_tmpfs_native_seek(fd: i32, whence: u32, offset: i64) -> i64;
        pub fn jnox_tmpfs_native_close(fd: i32) -> i64;
    }
}

fn result(status: i64) -> Result<u64, i32> {
    if status < 0 {
        Err(i32::try_from(status).unwrap_or(-5))
    } else {
        Ok(status as u64)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SeekFrom {
    Start(u64),
    Current(i64),
    End(i64),
}

/// One open file description with its own position. Dropping the handle
/// closes its descriptor. Use `close` when the closing result matters.
#[derive(Debug)]
pub struct File {
    fd: Option<i32>,
}

impl File {
    /// Open an existing path for reading. Paths are counted bytes.
    pub fn open(path: &[u8]) -> Result<Self, i32> {
        Self::open_with(path, 0)
    }

    /// Open an existing path for reading and writing without truncation.
    pub fn open_read_write(path: &[u8]) -> Result<Self, i32> {
        Self::open_with(path, 1)
    }

    /// Exclusively create a read/write file with owner permissions 0600.
    pub fn create_new(path: &[u8]) -> Result<Self, i32> {
        Self::open_with(path, 2)
    }

    fn open_with(path: &[u8], how: u32) -> Result<Self, i32> {
        let n = result(unsafe { c::jnox_tmpfs_native_open(path.as_ptr(), path.len(), how) })?;
        let fd = i32::try_from(n).map_err(|_| -5)?;
        Ok(Self { fd: Some(fd) })
    }

    /// One bounded read. EOF returns zero and a successful read may be short.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, i32> {
        let fd = self.fd.ok_or(-9)?;
        let n = result(unsafe { c::jnox_tmpfs_native_read(fd, buffer.as_mut_ptr(), buffer.len()) })?;
        if n > buffer.len() as u64 { return Err(-5); }
        Ok(n as usize)
    }

    /// One bounded write. A successful write may accept only a prefix.
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize, i32> {
        let fd = self.fd.ok_or(-9)?;
        let n = result(unsafe { c::jnox_tmpfs_native_write(fd, bytes.as_ptr(), bytes.len()) })?;
        if n > bytes.len() as u64 { return Err(-5); }
        Ok(n as usize)
    }

    pub fn seek(&mut self, from: SeekFrom) -> Result<u64, i32> {
        let fd = self.fd.ok_or(-9)?;
        let (whence, offset) = match from {
            SeekFrom::Start(pos) => (0, i64::try_from(pos).map_err(|_| -75)?),
            SeekFrom::Current(by) => (1, by),
            SeekFrom::End(by) => (2, by),
        };
        result(unsafe { c::jnox_tmpfs_native_seek(fd, whence, offset) })
    }

    /// Release the descriptor. A failed close keeps ownership for a retry.
    pub fn close(&mut self) -> Result<(), i32> {
        let fd = self.fd.ok_or(-9)?;
        result(unsafe { c::jnox_tmpfs_native_close(fd) })?;
        self.fd = None;
        Ok(())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if let Some(fd) = self.fd {
            unsafe { c::jnox_tmpfs_native_close(fd) };
        }
    }
}

pub fn unlink(path: &[u8]) -> Result<(), i32> {
    result(unsafe { c::jnox_tmpfs_native_unlink(path.as_ptr(), path.len()) }).map(|_| ())
}
