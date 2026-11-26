//! Full-device reset protocol shared by the native driver and host controls.
pub(super) trait ResetIo {
    fn request_reset(&mut self);
    fn status(&mut self) -> u32;
    fn select(&mut self, queue: u32);
    fn ready(&mut self) -> u32;
    fn fence(&mut self);
}

pub(super) fn reset<I: ResetIo>(io: &mut I, reclaim: impl FnOnce()) -> Result<(), i32> {
    io.fence();
    io.request_reset();
    io.fence();
    if !(0..1024).any(|_| io.status() == 0) {
        return Err(-5);
    }
    io.fence();
    for queue in [0, 1] {
        io.select(queue);
        io.fence();
        if io.ready() != 0 {
            return Err(-5);
        }
        io.fence();
    }
    reclaim();
    Ok(())
}

pub(super) const COLD: u32 = 0;
pub(super) const ACTIVE: u32 = 1;
pub(super) const RETIRING: u32 = 2;
pub(super) const RETIRED: u32 = 3;
pub(super) const QUARANTINED: u32 = 4;

pub(super) fn retirement_phase(reset: Result<(), i32>, faulted: bool) -> u32 {
    if reset.is_ok() && !faulted { RETIRED } else { QUARANTINED }
}

pub(super) enum Start { Cold, Restart }

pub(super) fn start(
    request: Start,
    phase: u32,
    failed: bool,
    setup: impl FnOnce() -> Result<(), i32>,
) -> Result<(), i32> {
    match (request, phase) {
        (Start::Cold, ACTIVE) if !failed => Ok(()),
        (Start::Cold, COLD) | (Start::Restart, RETIRED) if !failed => setup(),
        _ => Err(-5),
    }
}
