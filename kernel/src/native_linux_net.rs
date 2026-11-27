//! Control-plane access to the designated modern QEMU virtio-net transport.
//! All entries run with interrupts masked on the native scheduler's sole CPU.
use core::{arch::asm, ptr};

#[cfg(jnox_linux_packet_queues)]
#[path = "native_linux_net_queues.rs"]
mod queues;

#[cfg(jnox_linux_source_net_rx_irq)]
#[path = "native_linux_net_irq.rs"]
pub(super) mod irq;

#[cfg(jnox_linux_source_net_restart)]
#[path = "native_linux_net_restart.rs"]
mod restart;
#[cfg(jnox_linux_source_net_restart)]
pub(super) use restart::{restart_exit, restart_owner};

fn permitted(pid: usize) -> bool {
    #[cfg(jnox_linux_source_net_restart)]
    { restart_owner() == Some(pid) }
    #[cfg(not(jnox_linux_source_net_restart))]
    { pid == 0 }
}

const BASE: usize = 0x0a00_3e00;
const FEATURES: usize = 0x10;
const FEATURES_SEL: usize = 0x14;
const DRIVER_FEATURES: usize = 0x20;
const DRIVER_FEATURES_SEL: usize = 0x24;
const STATUS: usize = 0x70;
const CONFIG_GEN: usize = 0xfc;
const CONFIG: usize = 0x100;
const MAC: u32 = 1 << 5;
const MTU: u32 = 1 << 3;
const VERSION_1_HIGH: u32 = 1;
const ACK_DRIVER: u32 = 3;
const FEATURES_OK: u32 = 8;

#[derive(Copy, Clone, PartialEq)]
enum Phase {
    Free,
    Claimed,
    Attempted,
    #[cfg(jnox_linux_packet_queues)]
    Negotiated,
}

static mut PHASE: Phase = Phase::Free;

fn fence() {
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
}

fn read(reg: usize) -> u32 {
    unsafe { ptr::read_volatile((BASE + reg) as *const u32) }
}

fn write(reg: usize, value: u32) {
    unsafe { ptr::write_volatile((BASE + reg) as *mut u32, value) };
    fence();
}

#[cfg(jnox_linux_packet_queues)]
fn reset() -> bool { queues::reset() }

#[cfg(not(jnox_linux_packet_queues))]
fn reset() -> bool {
    write(STATUS, 0);
    (0..1024).any(|_| read(STATUS) == 0)
}

#[cfg(jnox_linux_source_net_lifecycle)]
pub(super) fn identity(pid: usize, selector: u64, arg: u64, reserved: u64) -> i64 {
    if !permitted(pid) { return -13; }
    if arg != 0 || reserved != 0 { return -22; }
    let reg = match selector { 0 => 8, 1 => 12, _ => return -22 };
    if read(0) != 0x7472_6976 || read(4) != 2 { return -19; }
    i64::from(read(reg))
}

fn claim() -> i64 {
    #[cfg(jnox_linux_packet_queues)]
    if queues::quarantined() { return -5; }
    if unsafe { PHASE } != Phase::Free { return -16; }
    if read(0) != 0x7472_6976 || read(4) != 2 || read(8) != 1 { return -19; }
    if !reset() { return -110; }
    write(STATUS, 1);
    write(STATUS, ACK_DRIVER);
    if read(STATUS) != ACK_DRIVER {
        return if reset() { -5 } else { -110 };
    }
    unsafe { PHASE = Phase::Claimed };
    0
}

fn release() -> i64 {
    let done = reset();
    unsafe { PHASE = Phase::Free };
    if done { 0 } else { -110 }
}

#[cfg(not(jnox_linux_source_net_restart))]
pub(super) fn release_on_exit() {
    if unsafe { PHASE } != Phase::Free {
        // A new claimant must also observe a completed reset before use.
        let _ = release();
    }
}

fn offered(bank: u32) -> u32 {
    write(FEATURES_SEL, bank);
    read(FEATURES)
}

fn config(offset: u64, width: u64) -> i64 {
    let feature = match (offset, width) {
        (0..=5, 1) => MAC,
        (10, 2) => MTU,
        _ => return -22,
    };
    if offered(0) & feature == 0 { return -95; }
    let addr = BASE + CONFIG + offset as usize;
    for _ in 0..8 {
        let generation = read(CONFIG_GEN);
        fence();
        let value = unsafe {
            if width == 1 {
                u32::from(ptr::read_volatile(addr as *const u8))
            } else {
                u32::from(u16::from_le(ptr::read_volatile(addr as *const u16)))
            }
        };
        fence();
        if read(CONFIG_GEN) == generation { return i64::from(value); }
    }
    -11
}

fn commit(low: u64, high: u64) -> i64 {
    if unsafe { PHASE } != Phase::Claimed { return -16; }
    let Ok(low) = u32::try_from(low) else { return -22; };
    if low & MAC == 0 || low & !(MAC | MTU) != 0 || high != u64::from(VERSION_1_HIGH) {
        return -22;
    }
    if low & !offered(0) != 0 || offered(1) & VERSION_1_HIGH == 0 { return -95; }
    if read(STATUS) != ACK_DRIVER { return -5; }
    unsafe { PHASE = Phase::Attempted };
    for (bank, word) in [(0, low), (1, VERSION_1_HIGH)] {
        write(DRIVER_FEATURES_SEL, bank);
        write(DRIVER_FEATURES, word);
    }
    write(STATUS, ACK_DRIVER | FEATURES_OK);
    if read(STATUS) != (ACK_DRIVER | FEATURES_OK) { return -5; }
    #[cfg(jnox_linux_packet_queues)]
    unsafe { PHASE = Phase::Negotiated };
    0
}

pub(super) fn service(pid: usize, number: u64, arg: u64, value: u64) -> i64 {
    if !permitted(pid) { return -13; }
    #[cfg(jnox_linux_packet_queues)]
    if number == 162 { return queues::inspect(arg, value); }
    if number == 156 {
        if arg == 1 {
            #[cfg(jnox_linux_source_net_restart)]
            { return restart::claim_once(pid); }
            #[cfg(not(jnox_linux_source_net_restart))]
            { return claim(); }
        }
        if arg != 0 { return -22; }
    }
    if unsafe { PHASE } == Phase::Free { return -19; }
    match number {
        #[cfg(jnox_linux_source_net_restart)]
        156 => restart::voluntary_release(pid),
        #[cfg(not(jnox_linux_source_net_restart))]
        156 => release(),
        157 if arg <= 1 => i64::from(offered(arg as u32)),
        158 => config(arg, value),
        159 => commit(arg, value),
        #[cfg(jnox_linux_packet_queues)]
        161 if arg == 0 && value == 0 => queues::setup(),
        #[cfg(jnox_linux_packet_queues)]
        161 => -22,
        160 if arg == 0 => i64::from(read(STATUS)),
        157 | 160 => -22,
        _ => -38,
    }
}

#[cfg(jnox_linux_net_packets)]
pub(super) fn packet_service(pid: usize, number: u64, token: u64, address: u64, len: u64) -> i64 {
    if !permitted(pid) { return -13; }
    queues::packet_service(number, token, address, len)
}
