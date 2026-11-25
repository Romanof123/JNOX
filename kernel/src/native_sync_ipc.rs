//! Single masked CPU0 boundary between endpoint state, saved continuations
//! and fixed-priority scheduling. Endpoint deadlines use CLOCK_MONOTONIC ns.
use super::{Action, CLOCK_DOMAIN, COUNT, CURRENT, FRAMES, Fixed, NativeFrame, fail, policy};

#[cfg(jnox_native_sync_ipc)]
#[path = "sync_ipc.rs"]
mod endpoints;
#[cfg(jnox_native_sync_ipc)]
static mut ENDPOINTS: endpoints::SyncIpc<COUNT> = endpoints::SyncIpc::new(super::NATIVE_IPC_ROUTES);

#[cfg(jnox_native_mailbox_ipc)]
#[path = "mailbox_ipc.rs"]
mod mailboxes;
#[cfg(jnox_native_mailbox_ipc)]
static mut MAIL: mailboxes::MailboxIpc<COUNT> = mailboxes::MailboxIpc::new(super::NATIVE_MAILBOX_ROUTES);

pub(super) fn handles(service: u64) -> bool {
    (cfg!(jnox_native_sync_ipc) && matches!(service, 21..=23))
        || (cfg!(jnox_native_mailbox_ipc) && matches!(service, 24..=25))
}

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
    let earliest: Option<u64> = None;
    #[cfg(jnox_native_sync_ipc)]
    let earliest = earliest.into_iter().chain(unsafe { (&*(&raw const ENDPOINTS)).next_deadline() }).min();
    #[cfg(jnox_native_mailbox_ipc)]
    let earliest = earliest.into_iter().chain(unsafe { (&*(&raw const MAIL)).next_deadline() }).min();
    earliest.map(|ns| compare(ns).unwrap_or_else(|| fail(9)))
}

// A task has one saved blocking SVC, in at most one family at a time.
// Complete immediate results on the actual trap stack and deferred results in FRAMES.
fn drain(fixed: &mut Fixed, mut live: Option<(usize, &mut NativeFrame)>) {
    for id in 0..COUNT {
        let mut result: Option<(i32, u64, Option<u64>)> = None;
        #[cfg(jnox_native_sync_ipc)]
        if let Some(done) = unsafe { (&mut *(&raw mut ENDPOINTS)).take(id) } {
            result = Some((done.status, done.x1, Some(done.x2)));
        }
        #[cfg(jnox_native_mailbox_ipc)]
        if let Some(done) = unsafe { (&mut *(&raw mut MAIL)).take(id) } {
            if result.is_some() { fail(9); }
            result = Some((done.status, done.x1, None));
        }
        let Some((status, word, extra)) = result else { continue };
        let frame = match live.as_mut() {
            Some((issuer, frame)) if *issuer == id => &mut **frame,
            _ => {
                fixed.ipc_wake(id);
                unsafe { &mut *(&raw mut FRAMES[id]) }
            }
        };
        frame.x[0] = i64::from(status) as u64;
        frame.x[1] = word;
        if let Some(value) = extra { frame.x[2] = value; }
    }
    #[cfg(jnox_native_sync_ipc)]
    fixed.ipc_priorities(&unsafe { (&*(&raw const ENDPOINTS)).priorities(super::NATIVE_PRIORITIES) });
}

fn advance(now: u64, exiting: Option<usize>) {
    #[cfg(jnox_native_sync_ipc)]
    unsafe {
        let ipc = &mut *(&raw mut ENDPOINTS);
        if let Some(pid) = exiting { ipc.exit(pid, now); } else { ipc.advance(now); }
    }
    #[cfg(jnox_native_mailbox_ipc)]
    unsafe {
        let mail = &mut *(&raw mut MAIL);
        if let Some(pid) = exiting { mail.exit(pid, now); } else { mail.advance(now); }
    }
}

pub(super) fn boundary(fixed: &mut Fixed, stamp: u64, exit: bool) {
    advance(monotonic_ns(stamp), exit.then(|| fixed.current().unwrap_or_else(|| fail(9))));
    drain(fixed, None);
}

pub(super) fn service(stamp: u64, frame: &mut NativeFrame) -> Action {
    let id = unsafe { CURRENT };
    let now = monotonic_ns(stamp);
    advance(now, None);
    let op = frame.x[8];
    let deadline = match op { 21 | 24 => frame.x[2], 25 => frame.x[0], _ => u64::MAX };
    let result = if deadline != u64::MAX && compare(deadline).is_none() {
        Err(-75)
    } else {
        match op {
            #[cfg(jnox_native_sync_ipc)]
            21 => unsafe { (&mut *(&raw mut ENDPOINTS)).call(id, frame.x[0] as usize, frame.x[1], deadline, now) },
            #[cfg(jnox_native_sync_ipc)]
            22 => unsafe { (&mut *(&raw mut ENDPOINTS)).receive(id, now) },
            #[cfg(jnox_native_sync_ipc)]
            23 => unsafe { (&mut *(&raw mut ENDPOINTS)).reply(id, frame.x[0], frame.x[1], now) },
            #[cfg(jnox_native_mailbox_ipc)]
            24 => unsafe { (&mut *(&raw mut MAIL)).send(id, frame.x[0] as usize, frame.x[1], deadline, now) },
            #[cfg(jnox_native_mailbox_ipc)]
            25 => unsafe { (&mut *(&raw mut MAIL)).receive(id, deadline, now) },
            _ => unreachable!(),
        }
    };
    frame.x[0] = i64::from(result.err().unwrap_or(0)) as u64;
    frame.x[1] = 0;
    if op <= 23 { frame.x[2] = 0; }
    drain(policy(), Some((id, frame)));
    #[cfg(jnox_native_sync_ipc)]
    if unsafe { (&*(&raw const ENDPOINTS)).blocked(id) } { return Action::BlockIpc; }
    #[cfg(jnox_native_mailbox_ipc)]
    if unsafe { (&*(&raw const MAIL)).blocked(id) } { return Action::BlockIpc; }
    Action::Poll
}

#[cfg(jnox_native_mailbox_ipc)]
pub(super) fn report_mailboxes() {
    let mail = unsafe { &*(&raw const MAIL) };
    for id in 0..COUNT {
        super::uart_write(b"NATIVE MAILBOX STATE");
        for v in [id, mail.occupancy(id), mail.high_water(id)] {
            super::uart_write(b" ");
            super::unsigned(v as u64);
        }
        super::uart_write(b"\n");
    }
}
