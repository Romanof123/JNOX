// IRQs are masked throughout each native syscall and fixed scheduling transition.
// Only the boot CPU runs these native processes.
include!(concat!(env!("OUT_DIR"), "/native_net_standby.rs"));

#[derive(Clone, Copy)]
enum Epoch {
    Original,
    Retired,
    Replacement,
    Closed,
    Failed(i32),
}

static mut EPOCH: Epoch = Epoch::Original;

pub(super) fn owns(pid: usize) -> bool {
    match unsafe { EPOCH } {
        Epoch::Original => pid == super::NATIVE_SOCKET_SERVICE,
        Epoch::Replacement => STANDBY == Some(pid),
        _ => false,
    }
}

pub(super) fn exit(pid: usize) {
    if !owns(pid) { return; }
    let original = matches!(unsafe { EPOCH }, Epoch::Original);
    // Revoke before touching the device, including on the final owner's exit.
    unsafe { EPOCH = Epoch::Closed };
    let next = match super::virtio_net::retire() {
        Ok(()) if original && STANDBY.is_some() => Epoch::Retired,
        Ok(()) => Epoch::Closed,
        Err(error) => Epoch::Failed(error),
    };
    unsafe { EPOCH = next };
}

pub(super) fn claim(pid: usize) -> Result<(), i32> {
    if STANDBY != Some(pid) { return Err(-13); }
    match unsafe { EPOCH } {
        Epoch::Original => Err(-11),
        Epoch::Replacement => Err(-114),
        Epoch::Closed => Err(-13),
        Epoch::Failed(error) => Err(error),
        Epoch::Retired => {
            unsafe { EPOCH = Epoch::Closed };
            match super::virtio_net::restart() {
                Ok(()) => {
                    unsafe { EPOCH = Epoch::Replacement };
                    Ok(())
                }
                Err(error) => {
                    unsafe { EPOCH = Epoch::Failed(error) };
                    Err(error)
                }
            }
        }
    }
}
