//! One-way handoff between two statically isolated Linux driver instances.
//! CPU0 runs all calls, termination and IRQ callbacks with DAIF.I masked.
use super::{claim, irq, queues, read, release, write, Phase, PHASE, STATUS};

static mut ELIGIBLE: Option<usize> = Some(0);
static mut CLAIMED: [bool; 2] = [false; 2];

// Row 0 is the original process; row 1 is its replacement. Keep the former
// through a successor reset. Only EL1 writes these non-DMA observations.
#[unsafe(no_mangle)]
static mut jnox_linux_net_restart_journal: [[u64; 48]; 2] = [[0; 48]; 2];

fn put(pid: usize, index: usize, value: u64) {
    unsafe { (&raw mut jnox_linux_net_restart_journal[pid / 2][index]).write_volatile(value) }
}

pub(in crate::native_el0) fn restart_owner() -> Option<usize> { unsafe { ELIGIBLE } }

pub(super) fn claim_once(pid: usize) -> i64 {
    if unsafe { CLAIMED[pid / 2] } { return -16; }
    let rc = claim();
    if rc == 0 {
        unsafe { CLAIMED[pid / 2] = true; }
        put(pid, 0, 1);
        put(pid, 1, pid as u64);
    }
    rc
}

fn retire(pid: usize) -> (bool, i64) {
    let (token, rx) = queues::retirement_snapshot();
    let status = read(STATUS);
    put(pid, 4, token);
    put(pid, 5, u64::from(status));
    for (i, v) in rx.into_iter().enumerate() { put(pid, 6 + i, v); }
    let before = irq::retirement_state();
    for (i, v) in before.into_iter().enumerate() { put(pid, 26 + i, v); }
    let active = token != 0 && rx[19] == token && before[0] == token && status == 15;
    let rc = release();
    put(pid, 31, rc as u64);
    let after = irq::retirement_state();
    for (i, v) in after.into_iter().enumerate() { put(pid, 32 + i, v); }
    put(pid, 37, u64::from(read(STATUS)));
    for q in 0..2 {
        write(0x30, q as u32);
        put(pid, 38 + q, u64::from(read(0x44)));
    }
    put(pid, 40, u64::from(queues::quarantined()));
    // release() uses status and both QueueReady readbacks, synchronizes the
    // GIC line and revokes its waiter before clearing ring and grant storage.
    (active, rc)
}

pub(super) fn voluntary_release(pid: usize) -> i64 {
    retire(pid).1
}

pub(in crate::native_el0) fn restart_exit(pid: usize, status: i32) -> bool {
    if restart_owner() != Some(pid) { return false; }
    // Revoke admission first. No sibling syscall can observe a partially
    // retired epoch. The queue token and IRQ recipient are revoked by reset.
    unsafe { ELIGIBLE = None; }
    put(pid, 2, 1);
    put(pid, 3, status as i64 as u64);
    if unsafe { PHASE } == Phase::Free { return false; }
    let (active, rc) = retire(pid);
    if pid == 0 && status == 86 && active && rc == 0 {
        put(pid, 41, 1);
        unsafe { ELIGIBLE = Some(2); }
        return true;
    }
    false
}
