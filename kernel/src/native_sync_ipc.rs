//! Single masked CPU0 boundary between endpoint state, saved continuations
//! and fixed-priority scheduling. Endpoint deadlines use CLOCK_MONOTONIC ns.
use super::{Action, CLOCK_DOMAIN, COUNT, CURRENT, FRAMES, Fixed, NativeFrame, fail, policy};

#[cfg(jnox_native_sync_ipc)]
#[path = "sync_ipc.rs"]
mod endpoints;
#[cfg(jnox_native_sync_ipc)]
static mut ENDPOINTS: endpoints::SyncIpc<COUNT> = {
    let state = endpoints::SyncIpc::new(super::NATIVE_IPC_ROUTES);
    #[cfg(jnox_native_socket_service)]
    let state = state.register_broker(super::NATIVE_SOCKET_SERVICE);
    #[cfg(jnox_native_pubsub_service)]
    let state = state.register_broker(super::NATIVE_PUBSUB_SERVICE);
    state
};

#[cfg(jnox_native_mailbox_ipc)]
#[path = "mailbox_ipc.rs"]
mod mailboxes;
#[cfg(jnox_native_mailbox_ipc)]
static mut MAIL: mailboxes::MailboxIpc<COUNT> = mailboxes::MailboxIpc::new(super::NATIVE_MAILBOX_ROUTES);

#[cfg(jnox_native_notify_ipc)]
#[path = "notify_ipc.rs"]
mod notifications;
#[cfg(jnox_native_notify_ipc)]
static mut NOTIFY: notifications::NotifyIpc<COUNT> = notifications::NotifyIpc::new(super::NATIVE_NOTIFY_ROUTES);

#[cfg(jnox_native_waitset)]
#[path = "waitset_ipc.rs"]
mod waitsets;
#[cfg(jnox_native_waitset)]
static mut SETS: waitsets::WaitsetIpc<COUNT> = waitsets::WaitsetIpc::new();

#[cfg(jnox_native_waitset)]
fn levels(id: usize) -> waitsets::Levels {
    let mut levels = waitsets::Levels::default();
    #[cfg(jnox_native_mailbox_ipc)]
    { levels.mailbox = unsafe { (&*(&raw const MAIL)).occupancy(id) != 0 }; }
    #[cfg(jnox_native_notify_ipc)]
    unsafe {
        let state = &*(&raw const NOTIFY);
        levels.pulse = state.occupancy(id) != 0;
        levels.events = state.pending_mask(id);
    }
    levels
}

#[cfg(jnox_native_waitset)]
fn refresh(now: u64) {
    let sets = unsafe { &mut *(&raw mut SETS) };
    for id in 0..COUNT {
        sets.refresh(id, now, levels(id));
    }
}

#[cfg(jnox_native_waitset)]
fn attach(id: usize, owner: usize, source: u64, mask: u64) -> Result<u64, i32> {
    if owner != id { return Err(-13); }
    match source {
        1 if !cfg!(jnox_native_mailbox_ipc) => return Err(-95),
        2 | 3 if !cfg!(jnox_native_notify_ipc) => return Err(-95),
        _ => {}
    }
    unsafe { (&mut *(&raw mut SETS)).attach(id, owner, source, mask) }
}

pub(super) fn handles(service: u64) -> bool {
    (cfg!(jnox_native_sync_ipc) && matches!(service, 21..=23))
        || (cfg!(jnox_native_socket_service) && service == 37)
        || (cfg!(jnox_native_pubsub_service) && service == 52)
        || (cfg!(jnox_native_mailbox_ipc) && matches!(service, 24..=25))
        || (cfg!(jnox_native_notify_ipc) && matches!(service, 26..=29))
        || (cfg!(jnox_native_waitset) && matches!(service, 30..=32))
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
    #[cfg(jnox_native_notify_ipc)]
    let earliest = earliest.into_iter().chain(unsafe { (&*(&raw const NOTIFY)).next_deadline() }).min();
    #[cfg(jnox_native_waitset)]
    let earliest = earliest.into_iter().chain(unsafe { (&*(&raw const SETS)).next_deadline() }).min();
    earliest.map(|ns| compare(ns).unwrap_or_else(|| fail(9)))
}

// A task has one saved blocking SVC, in at most one family at a time.
// Complete immediate results on the actual trap stack and deferred results in FRAMES.
fn drain(fixed: &mut Fixed, mut live: Option<(usize, &mut NativeFrame)>) {
    for id in 0..COUNT {
        let mut result: Option<(i32, u64, Option<u64>, Option<usize>)> = None;
        #[cfg(jnox_native_sync_ipc)]
        if let Some((done, caller)) = unsafe { (&mut *(&raw mut ENDPOINTS)).take_with_caller(id) } {
            result = Some((done.status, done.x1, Some(done.x2), caller));
        }
        #[cfg(jnox_native_mailbox_ipc)]
        if let Some(done) = unsafe { (&mut *(&raw mut MAIL)).take(id) } {
            if result.is_some() { fail(9); }
            result = Some((done.status, done.x1, None, None));
        }
        #[cfg(jnox_native_notify_ipc)]
        if let Some(done) = unsafe { (&mut *(&raw mut NOTIFY)).take(id) } {
            if result.is_some() { fail(9); }
            let service = match live.as_ref() {
                Some((issuer, frame)) if *issuer == id => frame.x[8],
                _ => unsafe { FRAMES[id].x[8] },
            };
            result = Some((done.status, done.x1, (service == 27).then_some(done.x2), None));
        }
        #[cfg(jnox_native_waitset)]
        if let Some(done) = unsafe { (&mut *(&raw mut SETS)).take(id) } {
            if result.is_some() { fail(9); }
            result = Some((done.status, done.x1, Some(done.x2), None));
        }
        let Some((status, word, extra, caller)) = result else { continue };
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
        if let Some(pid) = caller { frame.x[3] = pid as u64; }
    }
    #[cfg(jnox_native_sync_ipc)]
    fixed.ipc_priorities(&unsafe { (&*(&raw const ENDPOINTS)).priorities(super::NATIVE_PRIORITIES) });
}

fn advance(now: u64, exiting: Option<usize>) {
    // Each family expires waits against the same sampled time. Its exit
    // operation includes that expiry before teardown.
    macro_rules! update {
        ($state:ident) => {{
            let state = unsafe { &mut *(&raw mut $state) };
            if let Some(pid) = exiting { state.exit(pid, now); } else { state.advance(now); }
        }};
    }
    #[cfg(jnox_native_sync_ipc)]
    update!(ENDPOINTS);
    #[cfg(jnox_native_mailbox_ipc)]
    update!(MAIL);
    #[cfg(jnox_native_notify_ipc)]
    update!(NOTIFY);
    #[cfg(jnox_native_waitset)]
    update!(SETS);
}

pub(super) fn boundary(fixed: &mut Fixed, stamp: u64, exit: bool) {
    let now = monotonic_ns(stamp);
    advance(now, exit.then(|| fixed.current().unwrap_or_else(|| fail(9))));
    #[cfg(jnox_native_waitset)]
    refresh(now);
    drain(fixed, None);
}

pub(super) fn service(stamp: u64, frame: &mut NativeFrame) -> Action {
    let id = unsafe { CURRENT };
    let now = monotonic_ns(stamp);
    advance(now, None);
    let op = frame.x[8];
    let deadline = match op { 21 | 24 => frame.x[2], 25 | 27 | 32 => frame.x[0], 29 => frame.x[1], _ => u64::MAX };
    #[cfg(jnox_native_waitset)]
    let mut handle = 0;
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
            #[cfg(jnox_native_socket_service)]
            37 if id == super::NATIVE_SOCKET_SERVICE => unsafe { (&mut *(&raw mut ENDPOINTS)).broker_receive(id, now) },
            #[cfg(jnox_native_socket_service)]
            37 => Err(-13),
            #[cfg(jnox_native_pubsub_service)]
            52 if id == super::NATIVE_PUBSUB_SERVICE => unsafe { (&mut *(&raw mut ENDPOINTS)).broker_receive(id, now) },
            #[cfg(jnox_native_pubsub_service)]
            52 => Err(-13),
            #[cfg(jnox_native_mailbox_ipc)]
            24 => unsafe { (&mut *(&raw mut MAIL)).send(id, frame.x[0] as usize, frame.x[1], deadline, now) },
            #[cfg(jnox_native_mailbox_ipc)]
            25 => unsafe { (&mut *(&raw mut MAIL)).receive(id, deadline, now) },
            #[cfg(jnox_native_notify_ipc)]
            26 => unsafe { (&mut *(&raw mut NOTIFY)).pulse_send(id, frame.x[0] as usize, frame.x[1], frame.x[2], now) },
            #[cfg(jnox_native_notify_ipc)]
            27 => unsafe { (&mut *(&raw mut NOTIFY)).pulse_recv(id, deadline, now) },
            #[cfg(jnox_native_notify_ipc)]
            28 => unsafe { (&mut *(&raw mut NOTIFY)).event_set(id, frame.x[0] as usize, frame.x[1], now) },
            #[cfg(jnox_native_notify_ipc)]
            29 => unsafe { (&mut *(&raw mut NOTIFY)).event_take(id, frame.x[0], deadline, now) },
            #[cfg(jnox_native_waitset)]
            30 => attach(id, frame.x[0] as usize, frame.x[1], frame.x[2]).map(|value| handle = value),
            #[cfg(jnox_native_waitset)]
            31 => unsafe { (&mut *(&raw mut SETS)).detach(id, frame.x[0]) },
            #[cfg(jnox_native_waitset)]
            32 => unsafe { (&mut *(&raw mut SETS)).wait(id, deadline, now, levels(id)) },
            _ => unreachable!(),
        }
    };
    frame.x[0] = i64::from(result.err().unwrap_or(0)) as u64;
    frame.x[1] = 0;
    #[cfg(jnox_native_waitset)]
    {
        if op == 30 { frame.x[1] = handle; }
        refresh(now);
    }
    if op <= 23 || op == 27 || op == 32 { frame.x[2] = 0; }
    #[cfg(jnox_native_socket_service)]
    if op == 37 { frame.x[2] = 0; frame.x[3] = 0; }
    #[cfg(jnox_native_pubsub_service)]
    if op == 52 { frame.x[2] = 0; frame.x[3] = 0; }
    drain(policy(), Some((id, frame)));
    #[cfg(jnox_native_sync_ipc)]
    if unsafe { (&*(&raw const ENDPOINTS)).blocked(id) } { return Action::BlockIpc; }
    #[cfg(jnox_native_mailbox_ipc)]
    if unsafe { (&*(&raw const MAIL)).blocked(id) } { return Action::BlockIpc; }
    #[cfg(jnox_native_notify_ipc)]
    if unsafe { (&*(&raw const NOTIFY)).blocked(id) } { return Action::BlockIpc; }
    #[cfg(jnox_native_waitset)]
    if unsafe { (&*(&raw const SETS)).blocked(id) } { return Action::BlockIpc; }
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

#[cfg(jnox_native_notify_ipc)]
pub(super) fn report_notifications(phase: u64) {
    let state = unsafe { &*(&raw const NOTIFY) };
    for id in 0..COUNT {
        super::uart_write(b"NATIVE NOTIFY STATE");
        for v in [phase, id as u64, state.occupancy(id) as u64, state.high_water(id) as u64,
                  u64::from(state.pending_mask(id))] {
            super::uart_write(b" ");
            super::unsigned(v);
        }
        super::uart_write(b"\n");
    }
}
