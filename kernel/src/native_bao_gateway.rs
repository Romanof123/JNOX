//! Profile-private EL1 transport. The two CPUs access the shared Device page
//! only through aligned words. Native syscalls serialize on one masked vCPU.
use crate::native_el0_mmu::{self as mmu, BAO_MAILBOX_IPA};
use core::{arch::asm, ptr};

const REQUEST: usize = 0;
const REPLY: usize = 64;
const ACK: usize = 192;
const RECORD: usize = 24;

static mut LAST_REPLY: u32 = 0;

fn read(offset: usize) -> u32 {
    unsafe { ptr::read_volatile((BAO_MAILBOX_IPA + offset) as *const u32) }
}

fn write(offset: usize, word: u32) {
    unsafe { ptr::write_volatile((BAO_MAILBOX_IPA + offset) as *mut u32, word) }
}

fn barrier() {
    // Default asm memory clobber also orders the compiler's memory accesses.
    unsafe { asm!("dmb sy", options(nostack, preserves_flags)) }
}

fn send(addr: usize) -> i64 {
    let mut bytes = [0; RECORD];
    if !mmu::copy_from_user(addr, &mut bytes) {
        return -14;
    }
    // Continue our own sequence across a guest restart, including one that
    // interrupted an odd publication. Zero is reserved for an empty lane.
    let stable = ((read(REQUEST) & !1).wrapping_add(2)).max(2);
    write(REQUEST, stable - 1);
    barrier();
    for (i, chunk) in bytes.chunks_exact(4).enumerate() {
        write(REQUEST + 4 + i * 4, u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    barrier();
    write(REQUEST, stable);
    0
}

fn receive(addr: usize) -> i64 {
    if !mmu::user_range(addr, RECORD, true) {
        return -14;
    }
    let seq = read(REPLY);
    if seq == 0 || seq & 1 != 0 || seq == unsafe { LAST_REPLY } {
        return -11;
    }
    barrier();
    let mut bytes = [0; RECORD];
    for (i, chunk) in bytes.chunks_exact_mut(4).enumerate() {
        chunk.copy_from_slice(&read(REPLY + 4 + i * 4).to_le_bytes());
    }
    barrier();
    if read(REPLY) != seq {
        return -11;
    }
    if !mmu::copy_to_user(addr, &bytes) {
        return -14;
    }
    unsafe { LAST_REPLY = seq };
    barrier();
    write(ACK, seq);
    0
}

pub(super) fn service(pid: usize, number: u64, addr: usize, len: u64) -> i64 {
    if pid != 0 {
        return -13;
    }
    if len != RECORD as u64 {
        return -22;
    }
    match number {
        150 => send(addr),
        151 => receive(addr),
        _ => -38,
    }
}
