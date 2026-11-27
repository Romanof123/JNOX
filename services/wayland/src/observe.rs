//! Receipts from actual socket operations and completed scanout, written by
//! the single compositor thread. A stopped guest exposes the same wire bytes
//! the platform just accepted, including partial writes and ancillary order.
#[repr(C)]
#[derive(Clone, Copy)]
struct Wire {
    seq: u64,
    socket: i32,
    direction: u32,
    bytes: u32,
    count: u32,
    fds: [i32; 8],
    data: [u8; 64],
}
const N: usize = 2048;
const ZERO: Wire = Wire {
    seq: 0,
    socket: 0,
    direction: 0,
    bytes: 0,
    count: 0,
    fds: [0; 8],
    data: [0; 64],
};
const _: () = assert!(core::mem::size_of::<Wire>() == 120);
#[no_mangle]
static mut jnox_wayland_wire: [Wire; N] = [ZERO; N];
#[no_mangle]
static mut jnox_wayland_wire_head: [u64; 2] = [0; 2];
#[no_mangle]
pub static mut jnox_wayland_scene: [u64; 16] = [0; 16];

pub fn wire(socket: i32, dir: u32, bytes: &[u8], fds: &[i32]) {
    for i in 0..bytes.len().div_ceil(64).max(1) {
        let data = &bytes[i * 64..(i * 64 + 64).min(bytes.len())];
        let fds = if i == 0 { fds } else { &[] };
        let mut row = ZERO;
        unsafe {
            let seq = jnox_wayland_wire_head[0] + 1;
            row.seq = seq;
            row.socket = socket;
            row.direction = dir;
            row.bytes = data.len() as u32;
            row.count = fds.len() as u32;
            row.fds[..fds.len()].copy_from_slice(fds);
            row.data[..data.len()].copy_from_slice(data);
            (&raw mut jnox_wayland_wire[(seq as usize - 1) % N]).write_volatile(row);
            (&raw mut jnox_wayland_wire_head).write_volatile([seq, seq.saturating_sub(N as u64)]);
        }
    }
}
