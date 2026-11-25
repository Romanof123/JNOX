//! Single masked CPU0 boundary between endpoint state, saved continuations
//! and fixed-priority scheduling. Endpoint deadlines use CLOCK_MONOTONIC ns.
use super::{Action, CLOCK_DOMAIN, COUNT, CURRENT, FRAMES, Fixed, NATIVE_IPC_ROUTES,
            NATIVE_PRIORITIES, NativeFrame, fail, policy};

#[path = "sync_ipc.rs"]
mod endpoints;
use endpoints::SyncIpc;

static mut ENDPOINTS: SyncIpc<COUNT> = SyncIpc::new(NATIVE_IPC_ROUTES);

fn monotonic_ns(stamp: u64) -> u64 {
    let (origin, hz) = unsafe { CLOCK_DOMAIN };
    let elapsed = stamp.checked_sub(origin).unwrap_or_else(|| fail(9));
    (u128::from(elapsed) * 1_000_000_000 / u128::from(hz))
        .try_into().unwrap_or(u64::MAX)
}

fn compare(ns: u64) -> Option<u64> {
    let (origin, hz) = unsafe { CLOCK_DOMAIN };
    let delta = (u128::from(ns) * u128::from(hz)).div_ceil(1_000_000_000);
    origin.checked_add(u64::try_from(delta).ok()?)
}

pub(super) fn next_compare() -> Option<u64> {
    let ipc = unsafe { &*(&raw const ENDPOINTS) };
    ipc.next_deadline().map(|ns| compare(ns).unwrap_or_else(|| fail(9)))
}

fn drain(ipc: &mut SyncIpc<COUNT>, fixed: &mut Fixed, mut live: Option<(usize, &mut NativeFrame)>) {
    for id in 0..COUNT {
        let Some(done) = ipc.take(id) else { continue };
        let frame = match live.as_mut() {
            Some((issuer, frame)) if *issuer == id => &mut **frame,
            _ => {
                fixed.ipc_wake(id);
                unsafe { &mut *(&raw mut FRAMES[id]) }
            }
        };
        frame.x[0] = i64::from(done.status) as u64;
        frame.x[1] = done.x1;
        frame.x[2] = done.x2;
    }
    fixed.ipc_priorities(&ipc.priorities(NATIVE_PRIORITIES));
}

pub(super) fn boundary(fixed: &mut Fixed, stamp: u64, exit: bool) {
    let now = monotonic_ns(stamp);
    let ipc = unsafe { &mut *(&raw mut ENDPOINTS) };
    ipc.advance(now);
    if exit {
        ipc.exit(fixed.current().unwrap_or_else(|| fail(9)), now);
    }
    drain(ipc, fixed, None);
}

pub(super) fn service(stamp: u64, frame: &mut NativeFrame) -> Action {
    let id = unsafe { CURRENT };
    let ipc = unsafe { &mut *(&raw mut ENDPOINTS) };
    let now = monotonic_ns(stamp);
    ipc.advance(now);
    let result = match frame.x[8] {
        21 => {
            let deadline = frame.x[2];
            if deadline != u64::MAX && compare(deadline).is_none() {
                Err(-75)
            } else {
                ipc.call(id, frame.x[0] as usize, frame.x[1], deadline, now)
            }
        }
        22 => ipc.receive(id, now),
        23 => ipc.reply(id, frame.x[0], frame.x[1], now),
        _ => unreachable!(),
    };
    frame.x[0] = i64::from(result.err().unwrap_or(0)) as u64;
    frame.x[1] = 0;
    frame.x[2] = 0;
    drain(ipc, policy(), Some((id, frame)));
    if ipc.blocked(id) { Action::BlockIpc } else { Action::Poll }
}
