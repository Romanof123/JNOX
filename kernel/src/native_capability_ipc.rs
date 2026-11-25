//! CPU0 fixed-profile adapter. Calls, completion and teardown run IRQ-masked.
use super::super::{COUNT, NATIVE_CAP_GRANTS, NATIVE_CAP_ROUTES, NativeFrame, fail, uart_write, unsigned};

#[path = "capability_ipc.rs"]
mod core_policy;
use core_policy::CapabilityIpc;

static mut CAPS: CapabilityIpc<COUNT> = CapabilityIpc::new(NATIVE_CAP_ROUTES);

pub(in super::super) fn boot() {
    let state = unsafe { &mut *(&raw mut CAPS) };
    for &(owner, target, rights) in NATIVE_CAP_GRANTS {
        state.grant(owner, target, rights).unwrap_or_else(|_| fail(9));
    }
    report(0);
}

pub(super) fn exit(pid: usize) {
    unsafe { (&mut *(&raw mut CAPS)).exit(pid); }
}

pub(super) fn resolve_send(pid: usize, handle: u64) -> Result<usize, i32> {
    unsafe { (&mut *(&raw mut CAPS)).resolve_send(pid, handle) }
}

pub(super) fn service(pid: usize, frame: &NativeFrame, output: &mut [u64; 5]) -> Result<(), i32> {
    let state = unsafe { &mut *(&raw mut CAPS) };
    let x = &frame.x;
    match x[8] {
        53 => output[0] = state.initial(pid, x[0] as usize)?,
        54 => {
            let rights = u8::try_from(x[2]).map_err(|_| -22)?;
            let moved = match x[3] { 0 => false, 1 => true, _ => return Err(-22) };
            state.transfer(pid, x[0], x[1] as usize, rights, moved, x[4])?;
        }
        55 => {
            let n = state.receive(pid)?;
            *output = [n.handle, n.tag, n.sender as u64, n.target as u64, u64::from(n.rights)];
        }
        57 => state.close(pid, x[0])?,
        58 => {
            let s = state.stats(pid).ok_or(-22)?;
            *output = [s.handles as u64, s.notices as u64, s.peak_handles as u64, s.peak_notices as u64, 0];
        }
        _ => return Err(-38),
    }
    Ok(())
}

pub(in super::super) fn report(phase: u64) {
    let state = unsafe { &*(&raw const CAPS) };
    for pid in 0..COUNT {
        let s = state.stats(pid).unwrap_or_else(|| fail(9));
        uart_write(b"NATIVE CAP STATE");
        for word in [phase, pid as u64, s.alive as u64, s.handles as u64, s.notices as u64,
                     s.peak_handles as u64, s.peak_notices as u64, state.refs(pid) as u64] {
            uart_write(b" ");
            unsigned(word);
        }
        uart_write(b"\n");
    }
}
