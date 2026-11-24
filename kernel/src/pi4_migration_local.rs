//! Register ownership for the Pi migration profile. Register values are evidence
//! only. Every reference originates from a named, permanent Rust static.
use crate::pi4_smp_bsp as bsp;
use core::{arch::asm, sync::atomic::{AtomicU64, Ordering::Relaxed}};

#[repr(C, align(64))]
struct LocalWord(AtomicU64);

macro_rules! local_words {
    ($($name:ident),+) => {$(
        #[unsafe(no_mangle)]
        static $name: LocalWord = LocalWord(AtomicU64::new(0));
    )+};
}
local_words!(PI4_MIG_CPU0, PI4_MIG_CPU1, PI4_MIG_TLS_M, PI4_MIG_TLS_P0,
             PI4_MIG_TLS_P1_OPEN, PI4_MIG_TLS_P1_REPLACEMENT);

#[cfg(feature = "pi4-stream-v1")]
const IMAGE_ID: &str = concat!("MG4|", env!("JNOX_MIGRATION_APP"), "|sample-crc32-v1");
#[cfg(not(feature = "pi4-stream-v1"))]
const IMAGE_ID: &str = concat!("MG4|", env!("JNOX_MIGRATION_APP"), "|sum-65536");

#[unsafe(no_mangle)]
static pi4_mg4_identity: [u8; IMAGE_ID.len()] = {
    let mut id = [0; IMAGE_ID.len()];
    let mut i = 0;
    while i < id.len() {
        id[i] = IMAGE_ID.as_bytes()[i];
        i += 1;
    }
    id
};

fn cpu_record(c: usize) -> &'static LocalWord {
    if bsp::identity().0 & 0xff00ffffff != c as u64 {
        bsp::fail(0x31)
    }
    match c {
        0 => &PI4_MIG_CPU0,
        1 => &PI4_MIG_CPU1,
        _ => bsp::fail(0x31),
    }
}

// BSP enters masked and installs the record before vectors become reachable.
pub(super) fn install(c: usize) {
    let base = cpu_record(c) as *const LocalWord as u64;
    unsafe { asm!("msr tpidr_el1, {}", "isb", in(reg) base, options(nostack)) }
}

// Consume the returned atom in this masked section, before any context switch.
pub(super) fn local_lease(c: usize) -> (u64, &'static AtomicU64) {
    let record = cpu_record(c);
    let observed: u64;
    unsafe { asm!("mrs {}, tpidr_el1", out(reg) observed, options(nostack)) }
    if observed != record as *const LocalWord as u64 {
        bsp::fail(0x32)
    }
    (observed, &record.0)
}

fn activation(a: usize) -> (&'static LocalWord, u64) {
    match a {
        0 => (&PI4_MIG_TLS_M, 0x10601),
        1 => (&PI4_MIG_TLS_P0, 0x10602),
        2 => (&PI4_MIG_TLS_P1_OPEN, 0x10603),
        3 => (&PI4_MIG_TLS_P1_REPLACEMENT, 0x10604),
        _ => bsp::fail(0x33),
    }
}

pub(super) fn activation_pair(a: usize) -> (u64, u64) {
    let (tls, token) = activation(a);
    (tls as *const LocalWord as u64, token)
}

pub(super) fn initialize_tls() {
    unsafe { core::ptr::read_volatile(pi4_mg4_identity.as_ptr()); }
    for a in 0..4 {
        activation(a).0.0.store(0, Relaxed);
    }
}

fn checked_tls(a: usize) -> (&'static AtomicU64, u64, u64) {
    let (tls, expected_token) = activation(a);
    let (ptr, token): (u64, u64);
    unsafe {
        asm!("mrs {}, tpidr_el0", "mrs {}, tpidrro_el0",
             out(reg) ptr, out(reg) token, options(nostack))
    }
    if ptr != tls as *const LocalWord as u64 || token != expected_token {
        bsp::fail(0x34)
    }
    (&tls.0, ptr, token)
}

// One executing lease writes the digest. Frame publication orders handoffs.
pub(super) fn update_digest(a: usize, index: u64, sum: u64) {
    let (word, _, _) = checked_tls(a);
    let next = (word.load(Relaxed).rotate_left(7) ^ index).wrapping_add(sum);
    word.store(next, Relaxed);
}

// Sample the restored task registers on the CPU emitting the sparse row.
pub(super) fn observe_tls(a: usize) -> (u64, u64, u64) {
    let (word, ptr, token) = checked_tls(a);
    (ptr, token, word.load(Relaxed))
}

pub(super) fn digest(a: usize) -> u64 {
    checked_tls(a).0.load(Relaxed)
}
