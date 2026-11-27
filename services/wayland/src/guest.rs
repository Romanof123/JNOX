use core::time::Duration;
use jnox_native::{display, local_socket as sock, monotonic_time, sleep_until};
pub use sock::ShmMapping as Mapping;

pub fn listen(spare: bool) -> Result<i32, i32> {
    sock::set_broker(3)?;
    display::init().map_err(|e| e as i32)?;
    let clock_errno = |e| match e {
        jnox_native::ClockError::Errno(code) => -code,
        jnox_native::ClockError::InvalidTimespec => -22,
    };
    let deadline = if spare {
        Some(monotonic_time().map_err(clock_errno)? + Duration::from_secs(2))
    } else {
        None
    };
    loop {
        let fd = sock::socket(sock::SOCK_STREAM, true)?;
        match sock::bind(fd, b"/wayland-0") {
            Ok(()) => {
                if let Err(e) = sock::listen(fd, 2) {
                    close(fd);
                    return Err(e);
                }
                return Ok(fd);
            }
            Err(e) => {
                close(fd);
                let Some(end) = deadline.filter(|_| e == -98) else {
                    return Err(e);
                };
                let now = monotonic_time().map_err(clock_errno)?;
                if now >= end {
                    return Err(e);
                }
                sleep_until(end.min(now + Duration::from_millis(10))).map_err(clock_errno)?;
            }
        }
    }
}
pub fn accept(fd: i32) -> Result<i32, i32> {
    sock::accept(fd)
}
pub fn close(fd: i32) {
    let _ = sock::close(fd);
}
pub fn recv(fd: i32, bytes: &mut [u8], rights: &mut [i32; 8]) -> Result<(usize, usize), i32> {
    let msg = sock::recvmsg(fd, bytes, true, sock::MSG_DONTWAIT)?;
    if msg.control_truncated {
        if let Some(fd) = msg.fd {
            close(fd);
        }
        return Err(-90);
    }
    if let Some(right) = msg.fd {
        rights[0] = right;
    }
    Ok((msg.count, usize::from(msg.fd.is_some())))
}
pub fn send(fd: i32, bytes: &[u8], right: Option<i32>) -> Result<usize, i32> {
    sock::sendmsg(fd, bytes, right, sock::MSG_DONTWAIT)
}
pub fn map_readonly(fd: i32) -> Result<Mapping, ()> {
    sock::shm_map(fd, false).map_err(|_| ())
}
pub fn read_shm(map: &Mapping, offset: usize, dst: &mut [u8]) -> Result<(), ()> {
    if offset
        .checked_add(dst.len())
        .is_none_or(|end| end > map.len())
    {
        return Err(());
    }
    let source = (map.address() + offset) as *const u8;
    for (i, byte) in dst.iter_mut().enumerate() {
        *byte = unsafe { source.add(i).read_volatile() };
    }
    Ok(())
}
pub fn keymap_fd(bytes: &[u8]) -> Result<i32, ()> {
    let fd = sock::shm_create(bytes.len()).map_err(|_| ())?;
    let result = (|| {
        let map = sock::shm_map(fd, true)?;
        for (index, &byte) in bytes.iter().enumerate() {
            unsafe {
                map.write_byte(index, byte)?;
            }
        }
        drop(map);
        sock::shm_dup_readonly(fd)
    })();
    close(fd);
    result.map_err(|_| ())
}
pub fn input() -> Option<u8> {
    display::poll_key().ok()
}
pub fn row(y: usize, pixels: &[u32; 320]) -> Result<(), ()> {
    display::write_row(y, pixels).map_err(|_| ())
}
pub fn now_ms() -> u32 {
    monotonic_time().map_or(0, |t| t.as_millis() as u32)
}
pub fn idle() {
    if let Ok(t) = monotonic_time() {
        let _ = sleep_until(t + Duration::from_millis(5));
    }
}
