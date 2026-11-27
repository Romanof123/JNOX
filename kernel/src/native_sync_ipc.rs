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

#[cfg(jnox_native_capability_ipc)]
#[path = "native_capability_ipc.rs"]
pub(super) mod caps;

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
        || (cfg!(jnox_native_capability_ipc) && matches!(service, 53..=58))
        || (cfg!(jnox_native_socket_service) && service == 37)
        || (cfg!(jnox_native_shared_fd) && matches!(service, 184..=186))
        || (cfg!(jnox_native_pubsub_service) && service == 52)
        || (cfg!(jnox_native_net_service) && matches!(service, 38..=40 | 76))
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

#[cfg(jnox_native_display_standby)]
#[unsafe(no_mangle)]
static mut jnox_display_broker_deaths: [[u64; 3]; COUNT] = [[0; 3]; COUNT];

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
        if let Some(pid) = caller {
            frame.x[3] = pid as u64;
            #[cfg(jnox_native_display_standby)]
            if id == super::NATIVE_SOCKET_SERVICE && status == 0 && word == 0 {
                // Token zero identifies an authenticated peer-death delivery.
                unsafe {
                    (&raw mut jnox_display_broker_deaths[pid]).write_volatile([
                        super::counter(), id as u64, pid as u64,
                    ]);
                }
            }
        }
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
    #[cfg(jnox_native_shared_fd)]
    if let Some(pid) = exiting { super::shared_fd::exit(pid); }
    #[cfg(jnox_native_mailbox_ipc)]
    update!(MAIL);
    #[cfg(jnox_native_capability_ipc)]
    if let Some(pid) = exiting { caps::exit(pid); }
    #[cfg(jnox_native_notify_ipc)]
    update!(NOTIFY);
    #[cfg(jnox_native_waitset)]
    update!(SETS);
}

pub(super) fn boundary(fixed: &mut Fixed, stamp: u64, exiting: Option<usize>) {
    let now = monotonic_ns(stamp);
    advance(now, exiting);
    #[cfg(jnox_native_waitset)]
    refresh(now);
    drain(fixed, None);
}

pub(super) fn service(stamp: u64, frame: &mut NativeFrame) -> Action {
    let id = unsafe { CURRENT };
    let now = monotonic_ns(stamp);
    advance(now, None);
    let op = frame.x[8];
    #[cfg(jnox_native_shared_fd)]
    let mut escrow_result = 0;
    let deadline = match op { 21 | 24 | 56 => frame.x[2], 25 | 27 | 32 => frame.x[0], 29 => frame.x[1], _ => u64::MAX };
    #[cfg(jnox_native_waitset)]
    let mut handle = 0;
    #[cfg(jnox_native_capability_ipc)]
    let mut cap_output = [0; 5];
    #[cfg(jnox_native_net_service)]
    let mut transferred = 0;
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
            #[cfg(jnox_native_shared_fd)]
            184..=186 if id != super::NATIVE_SOCKET_SERVICE => Err(-13),
            #[cfg(jnox_native_shared_fd)]
            184 | 185 => {
                let (token, count, arg) = (frame.x[0], frame.x[1], frame.x[2]);
                let endpoints = unsafe { &mut *(&raw mut ENDPOINTS) };
                endpoints.reply_with(id, token, now, |caller, request| {
                    // The socket broker authenticates the staged BEGIN/CHUNK
                    // fields. Only an actual client COMMIT can move a right.
                    if request != 0x13u64 << 56 { return Err(-22); }
                    super::shared_fd::transfer(caller, op, token, count, arg)
                }).map(|value| escrow_result = value)
            }
            #[cfg(jnox_native_shared_fd)]
            186 => super::shared_fd::discard(frame.x[0]),
            #[cfg(jnox_native_pubsub_service)]
            52 if id == super::NATIVE_PUBSUB_SERVICE => unsafe { (&mut *(&raw mut ENDPOINTS)).broker_receive(id, now) },
            #[cfg(jnox_native_pubsub_service)]
            52 => Err(-13),
            #[cfg(jnox_native_net_service)]
            38 | 39 => net_frame(id, op, frame.x[0] as usize, frame.x[1] as usize)
                .map(|n| transferred = n as u64),
            #[cfg(jnox_native_net_service)]
            76 => super::net_owner::claim(id),
            #[cfg(jnox_native_net_service)]
            40 if id == super::NATIVE_SOCKET_SERVICE => unsafe { (&mut *(&raw mut ENDPOINTS)).broker_try_receive(id, now) },
            #[cfg(jnox_native_net_service)]
            40 => Err(-13),
            #[cfg(jnox_native_mailbox_ipc)]
            24 => unsafe { (&mut *(&raw mut MAIL)).send(id, frame.x[0] as usize, frame.x[1], deadline, now) },
            #[cfg(jnox_native_capability_ipc)]
            56 => caps::resolve_send(id, frame.x[0]).and_then(|owner| unsafe {
                (&mut *(&raw mut MAIL)).send_resolved(id, owner, frame.x[1], deadline, now)
            }),
            #[cfg(jnox_native_capability_ipc)]
            53..=55 | 57..=58 => caps::service(id, frame, &mut cap_output),
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
    #[cfg(jnox_native_net_service)]
    if matches!(op, 38 | 39) && result.is_ok() { frame.x[0] = transferred; }
    frame.x[1] = 0;
    #[cfg(jnox_native_shared_fd)]
    if matches!(op, 184..=186) { frame.x[1] = escrow_result; }
    #[cfg(jnox_native_waitset)]
    {
        if op == 30 { frame.x[1] = handle; }
        refresh(now);
    }
    if op <= 23 || op == 27 || op == 32 { frame.x[2] = 0; }
    #[cfg(jnox_native_socket_service)]
    if op == 37 || (cfg!(jnox_native_net_service) && op == 40) { frame.x[2] = 0; frame.x[3] = 0; }
    #[cfg(jnox_native_pubsub_service)]
    if op == 52 { frame.x[2] = 0; frame.x[3] = 0; }
    #[cfg(jnox_native_capability_ipc)]
    if matches!(op, 53..=58) { frame.x[1..6].copy_from_slice(&cap_output); }
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

#[cfg(jnox_native_net_service)]
fn net_frame(pid: usize, op: u64, address: usize, length: usize) -> Result<usize, i32> {
    use super::{native_el0_mmu as mmu, virtio_net};
    const MAX_FRAME: usize = 1514;
    if !super::net_owner::owns(pid) { return Err(-13); }
    if mmu::active() != Some(pid) { fail(9); }
    let mut data = [0u8; MAX_FRAME];
    if op == 38 {
        if length < MAX_FRAME { return Err(-90); }
        // Check every declared writable page before consuming the descriptor.
        // IRQs remain masked, so the active mapping cannot change before copyout.
        if !mmu::user_range(address, length, true) { return Err(-14); }
        let count = virtio_net::rx(&mut data)?;
        if !(14..=MAX_FRAME).contains(&count) { return Err(-5); }
        if !mmu::copy_to_user(address, &data[..count]) { fail(9); }
        Ok(count)
    } else {
        if !(14..=MAX_FRAME).contains(&length) { return Err(-90); }
        if !mmu::copy_from_user(address, &mut data[..length]) { return Err(-14); }
        virtio_net::tx(&data[..length])
    }
}

#[cfg(jnox_native_net_service)]
pub(super) fn report_net() {
    let s = super::virtio_net::stats();
    super::uart_write(b"NATIVE NET DEVICE");
    for value in [1, s.base, s.rx_frames, s.tx_frames, s.tx_completed, s.rx_empty, s.tx_full, s.faults] {
        super::uart_write(b" ");
        super::unsigned(value);
    }
    super::uart_write(b"\n");
}
