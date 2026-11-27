//! One kernel-owned split ring per direction. Identity-mapped, privileged,
//! non-cacheable RAM on the native virt board; no IOMMU or address from EL0.
use super::{fence, read, write, Phase, ACK_DRIVER, FEATURES_OK, PHASE, STATUS};
use core::{mem::{offset_of, size_of}, ptr};

#[allow(dead_code)]
#[path = "native_virtio_reset.rs"]
mod reset_protocol;

#[cfg(jnox_linux_net_packets)]
#[path = "native_linux_net_packets.rs"]
mod packets;

#[cfg(jnox_linux_net_packets)]
pub(super) use packets::packet_service;

const SIZE: usize = 32;
const SELECT: usize = 0x30;
const MAX: usize = 0x34;
const NUM: usize = 0x38;
const READY: usize = 0x44;
const DESC: usize = 0x80;
const DRIVER: usize = 0x90;
const DEVICE: usize = 0xa0;

#[repr(C, align(4096))]
struct Ring {
    desc: [[u64; 2]; SIZE],
    // flags, index, entries, plus the optional event word.
    available: [u16; SIZE + 3],
    available_padding: u16,
    used_header: [u16; 2],
    used: [[u32; 2]; SIZE],
    used_event: u16,
}

const _: () = assert!(
    offset_of!(Ring, desc) == 0 && offset_of!(Ring, available) % 2 == 0
        && offset_of!(Ring, used_header) % 4 == 0
        && offset_of!(Ring, used) == offset_of!(Ring, used_header) + 4
        && size_of::<Ring>() == 4096
);

#[unsafe(no_mangle)]
static mut jnox_linux_net_queue_pool: [Ring; 2] = unsafe { core::mem::zeroed() };
static mut ACTIVE_TOKEN: i64 = 0;
static mut GENERATION: i64 = 0;
static mut QUARANTINED: bool = false;

struct Io;
impl reset_protocol::ResetIo for Io {
    fn request_reset(&mut self) { write(STATUS, 0); }
    fn status(&mut self) -> u32 { read(STATUS) }
    fn select(&mut self, index: u32) { write(SELECT, index); }
    fn ready(&mut self) -> u32 { read(READY) }
    fn fence(&mut self) { fence(); }
}

#[cfg(jnox_linux_source_net_restart)]
pub(super) fn retirement_snapshot() -> (u64, [u64; 20]) {
    (unsafe { ACTIVE_TOKEN as u64 }, packets::retirement_snapshot())
}

pub(super) fn quarantined() -> bool { unsafe { QUARANTINED } }

#[cfg(jnox_linux_source_net_rx_irq)]
pub(super) fn rx_progress() -> (u16, bool) { packets::rx_progress() }
#[cfg(jnox_linux_source_net_rx_irq)]
pub(super) fn rx_callbacks_enabled() -> bool { packets::rx_callbacks_enabled() }

pub(super) fn reset() -> bool {
    // Retire the token before reset. The serialized path never reuses RAM
    // until Status=0 and QueueReady=0 have been observed for both queues.
    unsafe { ACTIVE_TOKEN = 0; }
    if quarantined() { return false; }
    #[cfg(jnox_linux_source_net_rx_irq)]
    super::irq::retire();
    let reset_failed = reset_protocol::reset(&mut Io, || {}).is_err();
    #[cfg(jnox_linux_source_net_rx_irq)]
    let reset_failed = !super::irq::finish_reset() || reset_failed;
    if reset_failed {
        unsafe { QUARANTINED = true; }
        return false;
    }
    #[cfg(jnox_linux_net_packets)]
    packets::retired();
    #[cfg(jnox_linux_source_net_restart)]
    {
        // The verified full-device reset is the ownership barrier for all
        // descriptor memory, even when the old application never reaped it.
        unsafe { ptr::write_bytes(&raw mut jnox_linux_net_queue_pool, 0, 1); }
        fence();
    }
    true
}

fn set_address(reg: usize, addr: usize) {
    write(reg, addr as u32);
    write(reg + 4, (addr >> 32) as u32);
}

fn valid_max(max: u32) -> bool {
    (SIZE as u32..=32768).contains(&max)
}

fn abort_setup() -> i64 {
    // A failed transaction ends the whole device claim, including negotiation.
    let result = super::release();
    if result == 0 { -5 } else { result }
}

pub(super) fn setup() -> i64 {
    if quarantined() { return -5; }
    if unsafe { ACTIVE_TOKEN != 0 } { return -16; }
    if unsafe { PHASE } != Phase::Negotiated || read(STATUS) != (ACK_DRIVER | FEATURES_OK) {
        return -22;
    }
    let Some(next) = (unsafe { GENERATION }).checked_add(1) else { return -75 };
    for index in 0..2 {
        write(SELECT, index);
        if read(READY) != 0 { return abort_setup(); }
        if !valid_max(read(MAX)) { return -95; }
    }
    // The complete BSS pool is private to EL1. No descriptors or available
    // entries are posted. QEMU cannot fetch buffers through an empty ring.
    unsafe { ptr::write_bytes((&raw mut jnox_linux_net_queue_pool).cast::<u8>(), 0, 2 * size_of::<Ring>()) };
    fence();
    for index in 0..2 {
        let ring = unsafe { &raw mut jnox_linux_net_queue_pool[index] };
        write(SELECT, index as u32);
        write(NUM, SIZE as u32);
        unsafe {
            set_address(DESC, (&raw mut (*ring).desc).addr());
            set_address(DRIVER, (&raw mut (*ring).available).addr());
            set_address(DEVICE, (&raw mut (*ring).used_header).addr());
        }
        write(READY, 1);
    }
    for index in 0..2 {
        write(SELECT, index);
        if read(READY) != 1 || !valid_max(read(MAX)) { return abort_setup(); }
    }
    if read(STATUS) != (ACK_DRIVER | FEATURES_OK) { return abort_setup(); }
    unsafe { GENERATION = next; ACTIVE_TOKEN = next; }
    next
}

pub(super) fn inspect(token: u64, index: u64) -> i64 {
    if token == 0 || token != unsafe { ACTIVE_TOKEN as u64 } { return -116; }
    if index > 1 { return -22; }
    write(SELECT, index as u32);
    let ready = read(READY);
    let max = read(MAX);
    let expected = ACK_DRIVER | FEATURES_OK;
    #[cfg(jnox_linux_net_packets)]
    let expected = if packets::running(token) { expected | 4 } else { expected };
    if ready != 1 || !valid_max(max) || read(STATUS) != expected {
        return abort_setup();
    }
    (i64::from(max) << 32) | ((SIZE as i64) << 16) | i64::from(ready)
}
