//! Fixed DMA buffers for two split rings. SVCs serialize on one masked CPU.
//! Descriptor IDs and payload addresses are selected exclusively by EL1.
use super::{abort_setup, fence, jnox_linux_net_queue_pool, read, write, Ring,
            ACK_DRIVER, ACTIVE_TOKEN, FEATURES_OK, SIZE, STATUS};
use crate::native_el0_mmu::{copy_from_user, copy_to_user};
use core::{ptr, slice};

const RX: usize = 0;
const TX: usize = 1;
const HEADER: usize = 12;
const MIN: usize = HEADER + 14;
const CAPACITY: usize = HEADER + 1514;
// Linux add_recvbuf_small posts hdr_len + GOOD_PACKET_LEN; the latter
// includes four bytes of VLAN headroom. Both buffer layouts stride 1536.
#[cfg(jnox_linux_source_net_rx)]
const RX_CAPACITY: usize = HEADER + 1518;
#[cfg(not(jnox_linux_source_net_rx))]
const RX_CAPACITY: usize = CAPACITY;
const NOTIFY: usize = 0x50;
const INTERRUPT_STATUS: usize = 0x60;
const INTERRUPT_ACK: usize = 0x64;
const LIVE: u32 = ACK_DRIVER | FEATURES_OK | 4;

#[repr(C, align(16))]
struct Buffer([u8; RX_CAPACITY]);

#[unsafe(no_mangle)]
static mut jnox_linux_net_packet_buffers: [[Buffer; SIZE]; 2] = unsafe { core::mem::zeroed() };

#[derive(Clone, Copy, Default)]
struct Completion { id: usize, len: usize }

#[derive(Clone, Copy, Default)]
struct Cursor {
    available: u16,
    used: u16,
    device_owned: u32,
    held: u32,
    completions: [Completion; SIZE],
    head: usize,
    count: usize,
}

static mut CURSORS: [Cursor; 2] = unsafe { core::mem::zeroed() };
static mut RUNNING_TOKEN: u64 = 0;

#[cfg(jnox_linux_source_net_rx)]
#[cfg_attr(jnox_linux_source_net_restart, repr(C))]
#[derive(Clone, Copy)]
struct RxGrant {
    token: u64,
    address: usize,
    len: usize,
}

#[cfg(jnox_linux_source_net_rx)]
const EMPTY_GRANT: RxGrant = RxGrant { token: 0, address: 0, len: 0 };
#[cfg(jnox_linux_source_net_restart)]
const _: () = assert!(core::mem::size_of::<RxGrant>() == 24
    && core::mem::offset_of!(RxGrant, address) == 8 && core::mem::offset_of!(RxGrant, len) == 16);

#[cfg(jnox_linux_source_net_rx)]
#[cfg_attr(jnox_linux_source_net_restart, unsafe(export_name = "jnox_linux_net_restart_grants"))]
static mut RX_GRANTS: [RxGrant; SIZE] = [EMPTY_GRANT; SIZE];

pub(super) fn running(token: u64) -> bool { token != 0 && token == unsafe { RUNNING_TOKEN } }

pub(super) fn retired() {
    // Only called once the transport confirmed Status=0 and both queues idle.
    unsafe {
        RUNNING_TOKEN = 0;
        #[cfg(jnox_linux_source_net_rx)]
        { RX_GRANTS = [EMPTY_GRANT; SIZE]; }
        CURSORS = [Cursor::default(); 2];
        #[cfg(jnox_linux_source_net_rx_poll)]
        (&raw mut (*ring(RX)).available[0]).write_volatile(0);
        ptr::write_bytes(&raw mut jnox_linux_net_packet_buffers, 0, 1);
    }
    fence();
}

fn ring(q: usize) -> *mut Ring { unsafe { &raw mut jnox_linux_net_queue_pool[q] } }
fn buffer(q: usize, id: usize) -> *mut u8 {
    unsafe { (&raw mut jnox_linux_net_packet_buffers[q][id]).cast() }
}

fn post(cursor: &mut Cursor, q: usize, id: usize, len: usize) {
    let ring = ring(q);
    let slot = usize::from(cursor.available) % SIZE;
    // Each descriptor references one contiguous kernel buffer, unchained.
    // Low 32 bits in word 1 are len, next 16 flags, then next=0 (little-endian).
    let flags = if q == RX { 2u64 } else { 0 };
    unsafe {
        (&raw mut (*ring).desc[id][0]).write_volatile(buffer(q, id).addr() as u64);
        (&raw mut (*ring).desc[id][1]).write_volatile((flags << 32) | len as u64);
        (&raw mut (*ring).available[2 + slot]).write_volatile(id as u16);
        cursor.device_owned |= 1u32 << id;
        fence();
        cursor.available = cursor.available.wrapping_add(1);
        (&raw mut (*ring).available[1]).write_volatile(cursor.available);
    }
    fence();
}

fn harvest(cursor: &mut Cursor, q: usize) -> Result<(), i64> {
    let ring = ring(q);
    let head = unsafe { (&raw const (*ring).used_header[1]).read_volatile() };
    fence();
    let delta = usize::from(head.wrapping_sub(cursor.used));
    if delta > cursor.device_owned.count_ones() as usize || delta > SIZE - cursor.count {
        return Err(-5);
    }
    for _ in 0..delta {
        let slot = usize::from(cursor.used) % SIZE;
        let id = unsafe { (&raw const (*ring).used[slot][0]).read_volatile() as usize };
        let len = unsafe { (&raw const (*ring).used[slot][1]).read_volatile() as usize };
        if id >= SIZE || cursor.device_owned & (1u32 << id) == 0
            || (q == TX && len != 0) || (q == RX && !(MIN..=RX_CAPACITY).contains(&len)) {
            return Err(-5);
        }
        cursor.device_owned &= !(1u32 << id);
        cursor.held |= 1u32 << id;
        cursor.completions[(cursor.head + cursor.count) % SIZE] = Completion { id, len };
        cursor.count += 1;
        cursor.used = cursor.used.wrapping_add(1);
    }
    Ok(())
}

fn pop(cursor: &mut Cursor, id: usize) {
    cursor.head = (cursor.head + 1) % SIZE;
    cursor.count -= 1;
    cursor.held &= !(1u32 << id);
}

fn start(token: u64) -> i64 {
    if running(token) { return -16; }
    if read(STATUS) != (ACK_DRIVER | FEATURES_OK) { return abort_setup(); }
    for q in [RX, TX] {
        write(super::SELECT, q as u32);
        if read(super::READY) != 1 { return abort_setup(); }
    }
    #[cfg(jnox_linux_source_net_lifecycle)]
    {
        let rx = unsafe { &*(&raw const CURSORS[RX]) };
        let avail = unsafe { (&raw const (*ring(RX)).available[1]).read_volatile() };
        let used = unsafe { (&raw const (*ring(RX)).used_header[1]).read_volatile() };
        if rx.device_owned != 0 || rx.held != 0 || avail != 0 || used != 0 {
            return abort_setup();
        }
    }
    #[cfg(all(jnox_linux_source_net_rx, not(jnox_linux_source_net_lifecycle)))]
    if unsafe { CURSORS[RX].device_owned } != u32::MAX { return -11; }
    #[cfg(not(jnox_linux_source_net_rx))]
    {
        let rx = unsafe { &mut *(&raw mut CURSORS[RX]) };
        for id in 0..SIZE { post(rx, RX, id, CAPACITY); }
    }
    #[cfg(jnox_linux_source_net_rx_irq)]
    {
        let flags = unsafe { &raw mut (*ring(RX)).available[0] };
        unsafe { flags.write_volatile(0) };
        fence();
        if unsafe { flags.read_volatile() } != 0 || !super::super::irq::enable(token) {
            return abort_setup();
        }
    }
    write(STATUS, LIVE);
    if read(STATUS) != LIVE { return abort_setup(); }
    unsafe { RUNNING_TOKEN = token };
    #[cfg(not(jnox_linux_source_net_lifecycle))]
    write(NOTIFY, RX as u32);
    0
}

fn submit(tx: &mut Cursor, address: usize, len: usize) -> i64 {
    if !(MIN..=CAPACITY).contains(&len) { return -90; }
    let unavailable = tx.device_owned | tx.held;
    let id = unavailable.trailing_ones() as usize;
    if id == SIZE { return -11; }
    // Only touch a free TX buffer. Keep the exact upstream-generated header
    // and packet; neither validation failure nor a copy fault publishes it.
    let bytes = unsafe { slice::from_raw_parts_mut(buffer(TX, id), len) };
    if !copy_from_user(address, bytes) { return -14; }
    if bytes[..HEADER] != [0; HEADER] { return -22; }
    post(tx, TX, id, len);
    id as i64
}

fn completion(cursor: &mut Cursor, q: usize, address: usize, cap: usize) -> Result<i64, i64> {
    harvest(cursor, q)?;
    if cursor.count == 0 { return Ok(-11); }
    let Completion { id, len } = cursor.completions[cursor.head];
    if q == TX {
        pop(cursor, id);
        return Ok(id as i64);
    }
    if cap < len { return Ok(-90); }
    let bytes = unsafe { slice::from_raw_parts(buffer(RX, id), len) };
    if !copy_to_user(address, bytes) { return Ok(-14); }
    pop(cursor, id);
    post(cursor, RX, id, CAPACITY);
    write(NOTIFY, RX as u32);
    Ok(len as i64)
}

#[cfg(jnox_linux_source_net_rx)]
fn grant_rx(token: u64, address: usize, len: usize) -> i64 {
    if len != RX_CAPACITY { return -90; }
    let Some(end) = address.checked_add(len) else { return -14; };
    if !crate::native_el0_mmu::user_range(address, len, true) { return -14; }
    let live = running(token);
    #[cfg(jnox_linux_source_net_lifecycle)]
    if !live { return -19; }
    let expected = if live { LIVE } else { ACK_DRIVER | FEATURES_OK };
    if read(STATUS) != expected { return abort_setup(); }
    let grants = unsafe { &mut *(&raw mut RX_GRANTS) };
    for grant in grants.iter().filter(|grant| grant.token != 0) {
        // Both exclusive ends passed checked_add before entering this table.
        if address < grant.address + grant.len && grant.address < end { return -16; }
    }
    let Some(id) = grants.iter().position(|grant| grant.token == 0) else { return -11; };
    grants[id] = RxGrant { token, address, len };
    post(unsafe { &mut *(&raw mut CURSORS[RX]) }, RX, id, len);
    #[cfg(not(jnox_linux_source_net_rx_poll))]
    if live { write(NOTIFY, RX as u32); }
    id as i64
}

#[cfg(jnox_linux_source_net_rx)]
fn reap_rx(token: u64) -> Result<i64, i64> {
    let rx = unsafe { &mut *(&raw mut CURSORS[RX]) };
    harvest(rx, RX)?;
    if rx.count == 0 { return Ok(-11); }
    let Completion { id, len } = rx.completions[rx.head];
    let grant = unsafe { RX_GRANTS[id] };
    if grant.token != token || len > grant.len { return Err(-5); }
    let bytes = unsafe { slice::from_raw_parts(buffer(RX, id), len) };
    // Retain this exact completion and grant until the entire copy succeeds.
    if !copy_to_user(grant.address, bytes) { return Ok(-14); }
    pop(rx, id);
    #[cfg(jnox_linux_source_net_rx_irq)]
    super::super::irq::consumed(rx.used.wrapping_sub(rx.count as u16));
    unsafe { RX_GRANTS[id] = EMPTY_GRANT; }
    Ok(((len as i64) << 16) | id as i64)
}

#[cfg(jnox_linux_source_net_rx_poll)]
fn rx_callbacks(mode: u64, arg: u64) -> i64 {
    let ring = ring(RX);
    let cursor = unsafe { &*(&raw const CURSORS[RX]) };
    match (mode, arg) {
        (0 | 1, 0) => {
            // With EVENT_IDX absent, NO_INTERRUPT is the sole defined bit.
            // The split ring is coherent, Normal non-cacheable DMA memory.
            let flags = unsafe { &raw mut (*ring).available[0] };
            let desired = u16::from(mode == 0);
            unsafe { flags.write_volatile(desired) };
            fence();
            if unsafe { flags.read_volatile() } != desired { return abort_setup(); }
            if mode == 0 { return 0; }
            // Harvest can advance used before EL0 dequeues the held entries.
            // Expose the index of the last completion delivered to EL0.
            i64::from(cursor.used.wrapping_sub(cursor.count as u16))
        }
        (2, snapshot) if snapshot <= u64::from(u16::MAX) => {
            // Publish callback enable before rechecking the actual used ring,
            // including a completion that raced the preceding enable_prepare.
            fence();
            let head = unsafe { (&raw const (*ring).used_header[1]).read_volatile() };
            fence();
            i64::from(cursor.count != 0 || head != snapshot as u16)
        }
        (3, 0) => {
            fence();
            write(NOTIFY, RX as u32);
            0
        }
        _ => -22,
    }
}

fn tx_callbacks(mode: u64, reserved: u64) -> i64 {
    if reserved != 0 || mode > 1 { return -22; }
    const NO_INTERRUPT: u16 = 1;
    let ring = ring(TX);
    // EVENT_IDX is excluded by feature negotiation. EL1 is the sole writer
    // of avail.flags. Publish this change before sampling the device index.
    let flags = unsafe { &raw mut (*ring).available[0] };
    let desired = if mode == 0 { NO_INTERRUPT } else { 0 };
    unsafe { flags.write_volatile(desired) };
    fence();
    if unsafe { flags.read_volatile() } != desired { return abort_setup(); }
    if mode == 0 { return 0; }

    let cursor = unsafe { &*(&raw const CURSORS[TX]) };
    let device_used = unsafe { (&raw const (*ring).used_header[1]).read_volatile() };
    fence();
    i64::from(cursor.count == 0 && device_used == cursor.used)
}

pub(in crate::native_el0::linux_net) fn packet_service(number: u64, token: u64, address: u64, len: u64) -> i64 {
    if token == 0 || token != unsafe { ACTIVE_TOKEN as u64 } { return -116; }
    #[cfg(jnox_linux_source_net_rx)]
    match number {
        163 | 167 => return -95,
        170 | 172 if address != 0 || len != 0 => return -22,
        #[cfg(jnox_linux_source_net_lifecycle)]
        170 => return -95,
        #[cfg(jnox_linux_source_net_lifecycle)]
        177 if address == 0 && len == 0 => return start(token),
        #[cfg(jnox_linux_source_net_lifecycle)]
        177 => return -22,
        #[cfg(not(jnox_linux_source_net_lifecycle))]
        170 => return start(token),
        171 => return grant_rx(token, address as usize, len as usize),
        _ => {}
    }
    #[cfg(not(jnox_linux_source_net_rx))]
    if number == 163 { return start(token); }
    if !running(token) { return -19; }
    if read(STATUS) != LIVE { return abort_setup(); }
    let result = match number {
        164 => return submit(unsafe { &mut *(&raw mut CURSORS[TX]) }, address as usize, len as usize),
        165 => { write(NOTIFY, TX as u32); return 0; }
        166 => completion(unsafe { &mut *(&raw mut CURSORS[TX]) }, TX, 0, 0),
        #[cfg(jnox_linux_source_net_rx)]
        172 => reap_rx(token),
        #[cfg(not(jnox_linux_source_net_rx))]
        167 => completion(unsafe { &mut *(&raw mut CURSORS[RX]) }, RX, address as usize, len as usize),
        169 => return tx_callbacks(address, len),
        #[cfg(jnox_linux_source_net_rx_poll)]
        173 => return rx_callbacks(address, len),
        #[cfg(jnox_linux_source_net_rx_irq)]
        168 => return -95, // The IRQ handler owns INTERRUPT_ACK.
        #[cfg(not(jnox_linux_source_net_rx_irq))]
        168 => {
            let pending = read(INTERRUPT_STATUS) & 3;
            if pending != 0 { write(INTERRUPT_ACK, pending); }
            return i64::from(pending);
        }
        _ => return -38,
    };
    result.unwrap_or_else(|_| abort_setup())
}

#[cfg(jnox_linux_source_net_rx_irq)]
pub(super) fn rx_progress() -> (u16, bool) {
    let cursor = unsafe { &*(&raw const CURSORS[RX]) };
    fence();
    let head = unsafe { (&raw const (*ring(RX)).used_header[1]).read_volatile() };
    fence();
    (head, cursor.count != 0 || head != cursor.used)
}

#[cfg(jnox_linux_source_net_rx_irq)]
pub(super) fn rx_callbacks_enabled() -> bool {
    unsafe { (&raw const (*ring(RX)).available[0]).read_volatile() == 0 }
}

#[cfg(jnox_linux_source_net_restart)]
pub(super) fn retirement_snapshot() -> [u64; 20] {
    let rx = unsafe { &*(&raw const CURSORS[RX]) };
    let grants = unsafe { &*(&raw const RX_GRANTS) };
    let r = ring(RX);
    let load_head = || unsafe { (&raw const (*r).used_header[1]).read_volatile() };
    let head = load_head();
    fence();
    let mut row = [0; 20];
    row[0] = unsafe { (&raw const (*r).available[1]).read_volatile() as u64 };
    row[1] = u64::from(head);
    row[2] = u64::from(rx.used);
    row[3] = u64::from(rx.used.wrapping_sub(rx.count as u16));
    row[4] = u64::from(rx.device_owned);
    row[5] = u64::from(rx.held);
    let mut waiting_on_device = rx.device_owned;
    let delta = usize::from(head.wrapping_sub(rx.used));
    row[6] = delta as u64;
    row[7] = u64::MAX;
    row[12] = u64::MAX;
    for (id, grant) in grants.iter().enumerate() {
        if grant.token != 0 { row[17] |= 1u64 << id; }
    }
    if delta <= SIZE {
        for i in 0..delta {
            let entry = (usize::from(rx.used) + i) % SIZE;
            let id = unsafe { (&raw const (*r).used[entry][0]).read_volatile() as usize };
            if id >= SIZE { waiting_on_device = 0; break; }
            waiting_on_device &= !(1u32 << id);
            if i == 0 {
                row[7] = id as u64;
                row[8] = unsafe { (&raw const (*r).used[entry][1]).read_volatile() as u64 };
                row[9] = grants[id].address as u64;
                row[10] = grants[id].len as u64;
                row[11] = unsafe { (&raw const (*r).desc[id][0]).read_volatile() };
            }
        }
        if waiting_on_device != 0 {
            let id = waiting_on_device.trailing_zeros() as usize;
            row[12] = id as u64;
            row[13] = unsafe { (&raw const (*r).desc[id][1]).read_volatile() };
            row[14] = grants[id].address as u64;
            row[15] = grants[id].token;
        }
    }
    row[16] = u64::from(waiting_on_device);
    fence();
    row[18] = u64::from(load_head());
    row[19] = unsafe { RUNNING_TOKEN };
    row
}

/// Observe a returned RX buffer without popping the used entry, copying to the
/// grantee, modifying a descriptor, or acknowledging the hardware interrupt.
#[cfg(jnox_linux_source_net_restart_pending_test)]
pub(super) fn observe_pending_frame(token: u64, slot: u16, out: &mut [u8; RX_CAPACITY]) -> Option<[u64; 4]> {
    let r = ring(RX);
    let entry = usize::from(slot) % SIZE;
    fence();
    let id = unsafe { (&raw const (*r).used[entry][0]).read_volatile() as usize };
    let len = unsafe { (&raw const (*r).used[entry][1]).read_volatile() as usize };
    if id >= SIZE || !(MIN..=RX_CAPACITY).contains(&len) { return None; }
    let grant = unsafe { RX_GRANTS[id] };
    let addr = unsafe { (&raw const (*r).desc[id][0]).read_volatile() };
    if token == 0 || grant.token != token || grant.len < len || addr != buffer(RX, id).addr() as u64 {
        return None;
    }
    for (i, byte) in out[..len].iter_mut().enumerate() {
        *byte = unsafe { buffer(RX, id).add(i).read_volatile() };
    }
    Some([id as u64, len as u64, grant.address as u64, addr])
}
