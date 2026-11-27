//! One userspace process namespace in the fixed AArch64 package. All changes
//! run on the same CPU with interrupts masked at the EL0 exception boundary.
use super::{COUNT, Fixed, NativeFrame, fail, native_el0_mmu, process_present, public_pid};

// Zero represents the initial group led by that slot's public PID. Process
// slots have one lifetime per boot. Dormant/dismissed/reaped slots cannot
// keep a group alive.
static mut GROUPS: [u64; COUNT] = [0; COUNT];

fn group(slot: usize) -> u64 {
    match unsafe { GROUPS[slot] } {
        0 => public_pid(slot),
        pgid => pgid,
    }
}

#[cfg(jnox_native_shell)]
pub(super) fn inherit(parent: usize, child: usize) {
    unsafe { GROUPS[child] = group(parent) };
}

fn set(slot: usize, fixed: &Fixed, pid: i32, pgid: i32) -> i64 {
    if pid < 0 || pgid < 0 {
        return -22;
    }
    let own = public_pid(slot);
    if pid != 0 && pid as u64 != own {
        #[cfg(jnox_native_shell)]
        if slot == 0 && (1..COUNT).contains(&(pid as usize))
            && (super::spawn::has_live_parent(pid as usize) || super::spawn::zombie(pid as usize))
        {
            // A shell child becomes public only after its executable is
            // installed. There is no pre-exec window for its parent.
            return -13;
        }
        return -3;
    }
    let desired = if pgid == 0 { own } else { pgid as u64 };
    if desired != own && !(0..COUNT).any(|id| process_present(fixed, id) && group(id) == desired) {
        return -1;
    }
    unsafe { GROUPS[slot] = desired };
    0
}

pub(super) fn service(fixed: &Fixed, slot: usize, frame: &mut NativeFrame) -> i64 {
    if fixed.current() != Some(slot) || native_el0_mmu::active() != Some(slot) {
        fail(9);
    }
    let (op, pid, pgid) = (frame.x[0], frame.x[1], frame.x[2]);
    frame.x[1] = 0;
    frame.x[2] = 0;
    match op {
        0 if pid == 0 && pgid == 0 => {
            frame.x[1] = group(slot);
            0
        }
        1 => match (i32::try_from(pid as i64), i32::try_from(pgid as i64)) {
            (Ok(target), Ok(group)) => set(slot, fixed, target, group),
            _ => -22,
        },
        _ => -22,
    }
}
