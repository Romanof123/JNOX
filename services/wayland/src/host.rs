//! Linux host platform for exercising the native compositor with ordinary Wayland clients.
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::mem::{align_of, size_of, size_of_val};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd};
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::os::unix::net::UnixListener;
use std::ptr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[repr(C)]
struct IoVec {
    base: *mut c_void,
    len: usize,
}

#[repr(C)]
struct Msg {
    name: *mut c_void,
    name_len: u32,
    iov: *mut IoVec,
    iov_len: usize,
    control: *mut c_void,
    control_len: usize,
    flags: i32,
}

#[repr(C)]
struct Header {
    len: usize,
    level: i32,
    kind: i32,
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn accept4(fd: i32, addr: *mut c_void, len: *mut u32, flags: i32) -> i32;
    fn recvmsg(fd: i32, msg: *mut Msg, flags: i32) -> isize;
    fn sendmsg(fd: i32, msg: *const Msg, flags: i32) -> isize;
    fn memfd_create(name: *const i8, flags: u32) -> i32;
    fn poll(fds: *mut PollFd, count: usize, timeout: i32) -> i32;
    fn read(fd: i32, data: *mut c_void, len: usize) -> isize;
    #[link_name = "close"]
    fn sys_close(fd: i32) -> i32;
}

const NONBLOCK: i32 = 0x800;
const CLOEXEC: i32 = 0x80000;
const DONTWAIT: i32 = 0x40;
const MSG_NOSIGNAL: i32 = 0x4000;
const MSG_CMSG_CLOEXEC: i32 = 0x40000000;
const MSG_CTRUNC: i32 = 8;
const SOL_SOCKET: i32 = 1;
const SCM_RIGHTS: i32 = 1;
const WIDTH: usize = 320;
const HEIGHT: usize = 240;

static FRAMEBUFFER: OnceLock<File> = OnceLock::new();

fn errno(e: io::Error) -> i32 {
    -e.raw_os_error().unwrap_or(5)
}

fn anonymous(name: &'static [u8]) -> io::Result<File> {
    let fd = unsafe { memfd_create(name.as_ptr().cast(), 1) }; // MFD_CLOEXEC
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub fn listen(_spare: bool) -> Result<i32, i32> {
    let path = std::env::args_os().nth(1).ok_or(-22)?;
    let fb = match std::env::var_os("JNOX_WAYLAND_FRAMEBUFFER") {
        Some(path) => OpenOptions::new().write(true).create_new(true).mode(0o600).open(path),
        None => anonymous(b"jnox-framebuffer\0"),
    }.map_err(errno)?;
    fb.set_len((WIDTH * HEIGHT * 4) as u64).map_err(errno)?;
    let listener = UnixListener::bind(path).map_err(errno)?;
    listener.set_nonblocking(true).map_err(errno)?;
    FRAMEBUFFER.set(fb).map_err(|_| -16)?;
    Ok(listener.into_raw_fd())
}

pub fn accept(fd: i32) -> Result<i32, i32> {
    let client = unsafe { accept4(fd, ptr::null_mut(), ptr::null_mut(), NONBLOCK | CLOEXEC) };
    if client < 0 { Err(errno(io::Error::last_os_error())) } else { Ok(client) }
}

pub fn close(fd: i32) {
    if fd >= 0 {
        unsafe { sys_close(fd); }
    }
}

const fn aligned(n: usize) -> usize {
    (n + align_of::<usize>() - 1) & !(align_of::<usize>() - 1)
}

const HEADER: usize = aligned(size_of::<Header>());
const SPACE: usize = aligned(HEADER + 8 * size_of::<i32>());

fn message(iov: &mut IoVec, control: *mut c_void, control_len: usize) -> Msg {
    Msg { name: ptr::null_mut(), name_len: 0, iov, iov_len: 1, control, control_len, flags: 0 }
}

pub fn recv(fd: i32, bytes: &mut [u8], fds: &mut [i32; 8]) -> Result<(usize, usize), i32> {
    if bytes.is_empty() {
        return Err(-22);
    }
    let mut control = [0usize; SPACE / size_of::<usize>()];
    let ptr = control.as_mut_ptr().cast::<u8>();
    let mut iov = IoVec { base: bytes.as_mut_ptr().cast(), len: bytes.len() };
    let mut msg = message(&mut iov, ptr.cast(), SPACE);
    let n = unsafe { recvmsg(fd, &mut msg, DONTWAIT | MSG_CMSG_CLOEXEC) };
    if n < 0 {
        return Err(errno(io::Error::last_os_error()));
    }

    // Linux closes excess rights that did not fit in the control buffer. All rights
    // that did fit are already installed and belong to us, even on MSG_CTRUNC.
    let mut invalid = msg.flags & MSG_CTRUNC != 0;
    let end = msg.control_len.min(SPACE);
    let mut offset = 0;
    let mut count = 0;
    while end - offset >= HEADER {
        let h = unsafe { &*ptr.add(offset).cast::<Header>() };
        if h.len < HEADER || h.len > end - offset {
            invalid = true;
            break;
        }
        let size = h.len - HEADER;
        if h.level == SOL_SOCKET && h.kind == SCM_RIGHTS {
            invalid |= size % size_of::<i32>() != 0;
            for j in 0..size / size_of::<i32>() {
                let received = unsafe { ptr.add(offset + HEADER).cast::<i32>().add(j).read() };
                if count == fds.len() {
                    close(received);
                    invalid = true;
                } else {
                    fds[count] = received;
                    count += 1;
                }
            }
        } else {
            invalid = true;
        }
        offset = (offset + aligned(h.len)).min(end);
    }
    if invalid {
        for received in &mut fds[..count] {
            close(*received);
            *received = -1;
        }
        return Err(-90);
    }
    Ok((n as usize, count))
}

pub fn send(fd: i32, bytes: &[u8], right: Option<i32>) -> Result<usize, i32> {
    let mut iov = IoVec { base: bytes.as_ptr().cast_mut().cast(), len: bytes.len() };
    let mut control = [0usize; aligned(HEADER + size_of::<i32>()) / size_of::<usize>()];
    let mut msg = message(&mut iov, ptr::null_mut(), 0);
    if let Some(right) = right {
        if bytes.is_empty() { return Err(-22); }
        let p = control.as_mut_ptr().cast::<u8>();
        unsafe {
            p.cast::<Header>().write(Header { len: HEADER + size_of::<i32>(), level: SOL_SOCKET, kind: SCM_RIGHTS });
            p.add(HEADER).cast::<i32>().write(right);
        }
        msg.control = p.cast();
        msg.control_len = size_of_val(&control);
    }
    let n = unsafe { sendmsg(fd, &msg, DONTWAIT | MSG_NOSIGNAL) };
    if n < 0 { Err(errno(io::Error::last_os_error())) } else { Ok(n as usize) }
}

pub struct Mapping {
    file: File,
    len: usize,
}

impl Mapping {
    pub fn len(&self) -> usize { self.len }
}

// A Wayland client may resize its shm file at any time. An exact positional
// read reports a short backing file as an error without risking SIGBUS.
pub fn read_shm(map: &Mapping, offset: usize, dst: &mut [u8]) -> Result<(), ()> {
    let end = offset.checked_add(dst.len()).ok_or(())?;
    if end > map.len { return Err(()); }
    map.file.read_exact_at(dst, offset as u64).map_err(|_| ())
}

pub fn map_readonly(fd: i32) -> Result<Mapping, ()> {
    if fd < 0 { return Err(()); }
    let owned = unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned().map_err(|_| ())?;
    let file = File::from(owned);
    let meta = file.metadata().map_err(|_| ())?;
    let len = usize::try_from(meta.len()).map_err(|_| ())?;
    if !meta.is_file() || len == 0 || len > isize::MAX as usize { return Err(()); }
    Ok(Mapping { file, len })
}

pub fn keymap_fd(blob: &[u8]) -> Result<i32, ()> {
    let mut backing = anonymous(b"jnox-keymap\0").map_err(|_| ())?;
    backing.write_all(blob).map_err(|_| ())?;
    let ro = File::open(format!("/proc/self/fd/{}", backing.as_raw_fd())).map_err(|_| ())?;
    Ok(ro.into_raw_fd())
}

pub fn now_ms() -> u32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u32
}

pub fn idle() { std::thread::sleep(Duration::from_millis(1)); }

pub fn input() -> Option<u8> {
    let mut p = PollFd { fd: 0, events: 1, revents: 0 };
    let mut byte = 0u8;
    if unsafe { poll(&mut p, 1, 0) } == 1 && p.revents & 1 != 0
        && unsafe { read(0, (&mut byte as *mut u8).cast(), 1) } == 1 {
        Some(byte)
    } else {
        None
    }
}

pub fn row(y: usize, pixels: &[u32; WIDTH]) -> Result<(), ()> {
    if y >= HEIGHT { return Err(()); }
    let mut bytes = [0u8; WIDTH * 4];
    for (word, dst) in pixels.iter().zip(bytes.chunks_exact_mut(4)) {
        dst.copy_from_slice(&word.to_le_bytes());
    }
    FRAMEBUFFER.get().ok_or(())?.write_all_at(&bytes, (y * WIDTH * 4) as u64).map_err(|_| ())
}
