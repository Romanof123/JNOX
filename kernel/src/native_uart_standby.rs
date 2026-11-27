//! A single boot-provisioned successor for a retired display/input owner.
//! All slots and their unique ASIDs are installed at cold boot. The
//! kernel is the only writer of the lease, with CPU0 IRQs masked throughout.
use super::{DISPLAY_OWNER, EXIT_STATUS, fail, ramfb};
#[cfg(jnox_native_uart_rx_irq)]
use super::uart_rx;
use crate::fixed_policy::{Action, Fixed};

const SPARE: usize = 2;
#[cfg(jnox_native_uart_rx_standby)]
const FAILURE: i32 = 86;

// Written from masked EL1. [exit tick, retired PID, outcome, final owner,
// selected PID, spare init tick]. Outcomes: launch, dismiss, quarantine, clean spare exit.
#[cfg(jnox_native_display_standby)]
#[unsafe(no_mangle)]
static mut jnox_display_lease: [[u64; 6]; super::COUNT] = [[0; 6]; super::COUNT];

#[cfg(jnox_native_display_standby)]
#[unsafe(no_mangle)]
static mut jnox_display_spare_restore: [u64; 3] = [0; 3];

#[cfg(jnox_native_display_standby)]
pub(super) fn restored(pid: usize, pc: u64, sp: u64) {
    if pid != SPARE { return; }
    let slot = &raw mut jnox_display_spare_restore;
    unsafe {
        if (*slot)[0] == 0 {
            slot.write_volatile([super::counter(), pc, sp]);
        }
    }
}

fn discard_pending() -> bool {
    #[cfg(jnox_native_uart_rx_irq)]
    { uart_rx::discard_pending() }
    #[cfg(not(jnox_native_uart_rx_irq))]
    {
        // PL011 RX FIFO is at most 16 bytes. Stop admitting a successor
        // if incoming input prevents us from observing an empty FIFO.
        for _ in 0..=16 {
            if crate::uart_try_read() == Err(-11) { return true; }
        }
        false
    }
}

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
    if matches!(call, 73 | 74 | 197) && !lease.initialized { return Err(-19); }
    Ok(())
}

pub(super) fn init() -> Result<(), i64> {
    let lease = unsafe { &mut *(&raw mut LEASE) };
    if lease.initialized { return Ok(()); }
    // Keep the boot's terminal fw_cfg result. A replacement does not reuse an
    // outstanding DMA descriptor or issue a second framebuffer configuration.
    ramfb::init()?;
    if lease.owner == Some(SPARE) && !discard_pending() {
        lease.owner = None;
        return Err(-5);
    }
    #[cfg(jnox_native_uart_rx_irq)]
    uart_rx::enable();
    #[cfg(jnox_native_display_standby)]
    if lease.owner == Some(SPARE) {
        unsafe { (&raw mut jnox_display_lease[DISPLAY_OWNER][5]).write_volatile(super::counter()) };
    }
    lease.initialized = true;
    Ok(())
}

pub(super) fn retire(pid: usize, _reason: u64, now: u64, fixed: &mut Fixed) {
    let lease = unsafe { &mut *(&raw mut LEASE) };
    if lease.owner != Some(pid) { return; }
    // The scheduler already retired this process. All authority is revoked
    // before examining the successor. A normal exit retires the device too.
    lease.owner = None;
    lease.initialized = false;
    #[cfg(jnox_native_uart_rx_irq)]
    let drained = uart_rx::revoke();
    #[cfg(not(jnox_native_uart_rx_irq))]
    let drained = discard_pending();
    #[cfg(jnox_native_pointer_tablet)]
    let drained = super::tablet::retire(pid) && drained;
    #[cfg(jnox_native_display_standby)]
    let failed = _reason == super::ASSERTION_EVENT;
    #[cfg(jnox_native_uart_rx_standby)]
    let failed = unsafe { EXIT_STATUS[pid] } == FAILURE;
    let outcome = if pid == DISPLAY_OWNER {
        if !fixed.dormant(SPARE) { fail(9); }
        if failed && drained {
            fixed.launch(SPARE);
            lease.owner = Some(SPARE);
            lease.spare_started = true;
            1
        } else {
            // Boot registered the dormant slot as a live socket client. Its
            // dismissal must deliver the same authenticated death as an exit.
            #[cfg(jnox_native_display_standby)]
            super::ipc::boundary(fixed, now, Some(SPARE));
            fixed.dismiss(SPARE);
            unsafe { EXIT_STATUS[SPARE] = -125 };
            if failed { 3 } else { 2 }
        }
    } else if failed {
        3
    } else {
        4
    };
    if fixed.event(now, Action::Poll) != Some(true) { fail(9); }
    #[cfg(jnox_native_display_standby)]
    unsafe {
        (&raw mut jnox_display_lease[pid]).write_volatile([
            now, pid as u64, outcome, lease.owner.map_or(u64::MAX, |id| id as u64),
            fixed.current().map_or(u64::MAX, |id| id as u64), 0,
        ]);
    }
    #[cfg(not(jnox_native_display_standby))]
    let _ = outcome;
}

pub(super) fn dismissed(pid: usize) -> bool {
    pid == SPARE && !unsafe { LEASE.spare_started }
}
