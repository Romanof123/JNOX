//! One replacement for a driver-reported UART failure on ordinary virt FIFO.
//! The three slots and their unique ASIDs are installed at cold boot. The
//! kernel is the only writer of the lease, with CPU0 IRQs masked throughout.
use super::{DISPLAY_OWNER, EXIT_STATUS, fail, ramfb, uart_rx};
use crate::fixed_policy::{Action, Fixed};

const SPARE: usize = 2;
const FAILURE: i32 = 86;

struct Lease {
    owner: Option<usize>,
    initialized: bool,
    spare_started: bool,
}

static mut LEASE: Lease = Lease {
    owner: Some(DISPLAY_OWNER), initialized: false, spare_started: false,
};

pub(super) fn authorize(pid: usize, call: u64) -> Result<(), i64> {
    let lease = unsafe { &*(&raw const LEASE) };
    if lease.owner != Some(pid) { return Err(-1); }
    if matches!(call, 73 | 74) && !lease.initialized { return Err(-19); }
    Ok(())
}

pub(super) fn init() -> Result<(), i64> {
    let lease = unsafe { &mut *(&raw mut LEASE) };
    if lease.initialized { return Ok(()); }
    // Keep the boot's terminal fw_cfg result. A replacement does not reuse an
    // outstanding DMA descriptor or issue a second framebuffer configuration.
    ramfb::init()?;
    if lease.owner == Some(SPARE) && !uart_rx::discard_pending() {
        lease.owner = None;
        return Err(-5);
    }
    uart_rx::enable();
    lease.initialized = true;
    Ok(())
}

pub(super) fn retire(pid: usize, now: u64, fixed: &mut Fixed) {
    let lease = unsafe { &mut *(&raw mut LEASE) };
    if lease.owner != Some(pid) { return; }
    // The scheduler already retired this process. All authority is revoked
    // before examining the successor. A normal exit retires the device too.
    lease.owner = None;
    lease.initialized = false;
    let drained = uart_rx::revoke();
    if pid != DISPLAY_OWNER || !fixed.dormant(SPARE) { return; }
    if unsafe { EXIT_STATUS[pid] } == FAILURE && drained {
        lease.owner = Some(SPARE);
        lease.spare_started = true;
        fixed.launch(SPARE);
        // Reselect at the original exit timestamp: no execution time elapses
        // between retirement and admission inside this masked scheduler entry.
        if fixed.event(now, Action::Poll) != Some(true) { fail(9); }
    } else {
        fixed.dismiss(SPARE);
        unsafe { EXIT_STATUS[SPARE] = -125 }; // ECANCELED: never ran
    }
}

pub(super) fn dismissed(pid: usize) -> bool {
    pid == SPARE && !unsafe { LEASE.spare_started }
}
