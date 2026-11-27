//! AF_UNIX and IPv4 broker services using the same wire client as the C socket API.
//! Errors are negative errno values. Paths contain 1..=31 non-NUL bytes.
use core::{ffi::c_void, ptr};

pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;
pub const SOCK_SEQPACKET: i32 = 5;
pub const MSG_TRUNC: i32 = 0x20;
pub const MSG_DONTWAIT: i32 = 0x40;
pub const MSG_CMSG_CLOEXEC: i32 = 0x4000_0000;
pub const MSG_CTRUNC: i32 = 0x08;
pub const SHUT_RD: i32 = 0;
pub const SHUT_WR: i32 = 1;
pub const SHUT_RDWR: i32 = 2;

#[repr(C)]
struct Address { family: u16, path: [u8; 32] }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerName { path: [u8; 32], len: usize }
impl PeerName {
    pub fn as_bytes(&self) -> &[u8] { &self.path[..self.len] }
}

mod c {
    use super::{Address, c_void};
    extern "C" {
        pub fn jnox_socket_broker(pid: u64) -> i32;
        pub fn socket(domain: i32, kind: i32, proto: i32) -> i32;
        pub fn bind(fd: i32, addr: *const Address, len: u32) -> i32;
        pub fn connect(fd: i32, addr: *const Address, len: u32) -> i32;
        pub fn listen(fd: i32, backlog: i32) -> i32;
        pub fn accept(fd: i32, addr: *mut Address, len: *mut u32) -> i32;
        pub fn send(fd: i32, data: *const c_void, len: usize, flags: i32) -> isize;
        pub fn sendto(fd: i32, data: *const c_void, len: usize, flags: i32, addr: *const Address, addrlen: u32) -> isize;
        pub fn recv(fd: i32, data: *mut c_void, len: usize, flags: i32) -> isize;
        pub fn recvfrom(fd: i32, data: *mut c_void, len: usize, flags: i32, addr: *mut Address, addrlen: *mut u32) -> isize;
        pub fn shutdown(fd: i32, how: i32) -> i32;
        pub fn close(fd: i32) -> i32;
    }
}

fn path(bytes: &[u8]) -> Result<(Address, u32), i32> {
    if bytes.is_empty() || bytes.len() > 31 || bytes.contains(&0) { return Err(-22); }
    let mut addr = Address { family: 1, path: [0; 32] };
    addr.path[..bytes.len()].copy_from_slice(bytes);
    Ok((addr, 3 + bytes.len() as u32))
}
fn result(value: i32) -> Result<i32, i32> {
    if value < 0 { Err(-unsafe { *super::__jnox_errno_location() }) } else { Ok(value) }
}
fn count(value: isize) -> Result<usize, i32> {
    if value < 0 { Err(-unsafe { *super::__jnox_errno_location() }) } else { Ok(value as usize) }
}

pub fn set_broker(pid: u64) -> Result<(), i32> {
    result(unsafe { c::jnox_socket_broker(pid) }).map(|_| ())
}
pub fn socket(kind: i32, nonblocking: bool) -> Result<i32, i32> {
    let flags = if nonblocking { 0x800 } else { 0 };
    result(unsafe { c::socket(1, kind | flags, 0) })
}
pub fn bind(fd: i32, bytes: &[u8]) -> Result<(), i32> {
    let (addr, len) = path(bytes)?;
    result(unsafe { c::bind(fd, &addr, len) }).map(|_| ())
}
pub fn connect(fd: i32, bytes: &[u8]) -> Result<(), i32> {
    let (addr, len) = path(bytes)?;
    result(unsafe { c::connect(fd, &addr, len) }).map(|_| ())
}
pub fn listen(fd: i32, backlog: i32) -> Result<(), i32> {
    result(unsafe { c::listen(fd, backlog) }).map(|_| ())
}
pub fn accept(fd: i32) -> Result<i32, i32> {
    result(unsafe { c::accept(fd, ptr::null_mut(), ptr::null_mut()) })
}
pub fn send(fd: i32, bytes: &[u8], flags: i32) -> Result<usize, i32> {
    count(unsafe { c::send(fd, bytes.as_ptr().cast(), bytes.len(), flags) })
}
pub fn send_to(fd: i32, bytes: &[u8], flags: i32, name: &[u8]) -> Result<usize, i32> {
    let (addr, len) = path(name)?;
    count(unsafe { c::sendto(fd, bytes.as_ptr().cast(), bytes.len(), flags, &addr, len) })
}
/// With MSG_TRUNC the return value can exceed the destination length.
pub fn recv(fd: i32, bytes: &mut [u8], flags: i32) -> Result<usize, i32> {
    count(unsafe { c::recv(fd, bytes.as_mut_ptr().cast(), bytes.len(), flags) })
}
pub fn recv_from(fd: i32, bytes: &mut [u8], flags: i32) -> Result<(usize, PeerName), i32> {
    let mut addr = Address { family: 0, path: [0; 32] };
    let mut len = core::mem::size_of::<Address>() as u32;
    let n = count(unsafe { c::recvfrom(fd, bytes.as_mut_ptr().cast(), bytes.len(), flags, &mut addr, &mut len) })?;
    if addr.family != 1 { return Err(-5); }
    let path_len = match len { 2 => 0, 4..=34 => len as usize - 3, _ => return Err(-5) };
    Ok((n, PeerName { path: addr.path, len: path_len }))
}
pub fn shutdown(fd: i32, how: i32) -> Result<(), i32> {
    result(unsafe { c::shutdown(fd, how) }).map(|_| ())
}
pub fn close(fd: i32) -> Result<(), i32> {
    result(unsafe { c::close(fd) }).map(|_| ())
}

mod rights;
pub use rights::{recvmsg, sendmsg, shm_create, shm_dup_readonly, shm_map, shm_size, ReceivedMessage, ShmMapping};

/// IPv4 octets in network order and a host-order port number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Endpoint {
    pub address: [u8; 4],
    pub port: u16,
}

#[repr(C)]
struct InetAddress { family: u16, port: u16, address: u32, padding: [u8; 8] }
const INET_LEN: u32 = core::mem::size_of::<InetAddress>() as u32;

impl From<Ipv4Endpoint> for InetAddress {
    fn from(ep: Ipv4Endpoint) -> Self {
        Self { family: 2, port: ep.port.to_be(), address: u32::from_ne_bytes(ep.address), padding: [0; 8] }
    }
}

pub fn socket4(kind: i32, nonblocking: bool) -> Result<i32, i32> {
    let flags = if nonblocking { 0x800 } else { 0 };
    result(unsafe { c::socket(2, kind | flags, 0) })
}
pub fn bind4(fd: i32, endpoint: Ipv4Endpoint) -> Result<(), i32> {
    let addr = InetAddress::from(endpoint);
    result(unsafe { c::bind(fd, (&raw const addr).cast(), INET_LEN) }).map(|_| ())
}
pub fn connect4(fd: i32, endpoint: Ipv4Endpoint) -> Result<(), i32> {
    let addr = InetAddress::from(endpoint);
    result(unsafe { c::connect(fd, (&raw const addr).cast(), INET_LEN) }).map(|_| ())
}
pub fn send_to4(fd: i32, bytes: &[u8], flags: i32, endpoint: Ipv4Endpoint) -> Result<usize, i32> {
    let addr = InetAddress::from(endpoint);
    count(unsafe { c::sendto(fd, bytes.as_ptr().cast(), bytes.len(), flags, (&raw const addr).cast(), INET_LEN) })
}
/// Datagram receives supply a source; stream receives omit it. With MSG_TRUNC
/// the count may exceed the destination length for a truncated UDP datagram.
pub fn recv_from4(fd: i32, bytes: &mut [u8], flags: i32) -> Result<(usize, Option<Ipv4Endpoint>), i32> {
    let mut addr = InetAddress { family: 0, port: 0, address: 0, padding: [0; 8] };
    let mut len = INET_LEN;
    let n = count(unsafe { c::recvfrom(fd, bytes.as_mut_ptr().cast(), bytes.len(), flags,
                                      (&raw mut addr).cast(), &mut len) })?;
    if len == 0 { return Ok((n, None)); }
    if len != INET_LEN || addr.family != 2 { return Err(-5); }
    Ok((n, Some(Ipv4Endpoint { address: addr.address.to_ne_bytes(), port: u16::from_be(addr.port) })))
}
