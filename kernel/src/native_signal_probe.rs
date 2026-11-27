use super::{COUNT, Fixed, fail, native_el0_mmu, process_present, public_pid};

/// Positive-PID existence and permission probe. No signal is queued by this call.
pub(super) fn call(caller: usize, fixed: &Fixed, pid: u64, signal: u64, reserved: u64) -> i64 {
    if fixed.current() != Some(caller) || native_el0_mmu::active() != Some(caller) {
        fail(9);
    }
    let Ok(pid) = i32::try_from(pid as i64) else { return -22; };
    // The selected SDK has signal numbers 1..31; zero is the existence query.
    if reserved != 0 || signal >= 32 { return -22; }
    if pid <= 0 { return -95; } // group and broadcast selectors
    let Some(target) = (0..COUNT).find(|&slot| public_pid(slot) == pid as u64) else { return -3; };
    if !process_present(fixed, target) { return -3; }

    let permitted = target == caller;
    #[cfg(jnox_native_shell)]
    let permitted = permitted || (caller == 0 && target != 0 &&
        (super::spawn::has_live_parent(target) || super::spawn::zombie(target)));
    if !permitted { return -1; }
    if signal != 0 { return -95; }
    0
}
