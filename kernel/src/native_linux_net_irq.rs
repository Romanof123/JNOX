//! QEMU virt slot 31, DT interrupt <0 47 1>. One owner on CPU0; native SVC,
//! IRQ entry and reset paths all run with local IRQs masked.
use super::{fence, queues, read, write, STATUS};
use crate::context::{gic_read, gic_write, GICC, GICD};
use core::ptr;

pub(in crate::native_el0) const IRQ: u32 = 79;
const WORD: usize = (IRQ as usize / 32) * 4;
const LINE: u32 = 1 << (IRQ % 32);
const GROUP: usize = GICD + 0x080 + WORD;
const ENABLE: usize = GICD + 0x100 + WORD;
const PENDING: usize = GICD + 0x200 + WORD;
const CONFIG: usize = GICD + 0xc00 + (IRQ as usize / 16) * 4;
const EDGE: u32 = 2 << ((IRQ % 16) * 2);
const PRIORITY: usize = GICD + 0x400 + IRQ as usize;
const TARGET: usize = GICD + 0x800 + IRQ as usize;
const INT_STATUS: usize = 0x60;
const INT_ACK: usize = 0x64;
const LIVE: u32 = 15;
const CATCHUP: i64 = i64::MAX;

#[unsafe(no_mangle)]
static mut jnox_linux_net_irq_metrics: [u64; 32] = [0; 32];
static mut EPOCH: u64 = 0;
static mut DELIVERED: u64 = 0;
// Last completion covered by an unconsumed interrupt. Once EL0 has drained it,
// a later unannounced buffer must be classified as catch-up, even across wraps.
static mut ANNOUNCED_HEAD: Option<u16> = None;

pub(super) fn consumed(head: u16) {
    unsafe {
        if ANNOUNCED_HEAD == Some(head) { ANNOUNCED_HEAD = None; }
    }
}


fn get(i: usize) -> u64 {
    unsafe { (&raw const jnox_linux_net_irq_metrics[i]).read_volatile() }
}
fn set(i: usize, n: u64) {
    unsafe { (&raw mut jnox_linux_net_irq_metrics[i]).write_volatile(n) }
}
fn inc(i: usize) { set(i, get(i).saturating_add(1)); }

fn mask() {
    gic_write(GICD + 0x180 + WORD, LINE);
    fence();
}
fn clear_pending() {
    gic_write(GICD + 0x280 + WORD, LINE);
    fence();
}

pub(super) fn enable(token: u64) -> bool {
    mask();
    clear_pending();
    gic_write(GROUP, gic_read(GROUP) & !LINE);
    gic_write(CONFIG, gic_read(CONFIG) | EDGE);
    unsafe {
        (PRIORITY as *mut u8).write_volatile(0xa0);
        (TARGET as *mut u8).write_volatile(1);
    }
    fence();
    gic_write(ENABLE, LINE);
    fence();
    let checks = u64::from(gic_read(GROUP) & LINE == 0)
        | u64::from(gic_read(CONFIG) & EDGE != 0) << 1
        | u64::from(unsafe { (PRIORITY as *const u8).read_volatile() } == 0xa0) << 2
        | u64::from(unsafe { (TARGET as *const u8).read_volatile() } == 1) << 3
        | u64::from(gic_read(ENABLE) & LINE != 0) << 4
        | u64::from(gic_read(GICD) & 1 != 0 && gic_read(GICC) & 1 != 0
            && gic_read(GICC + 4) & 0xff > 0xa0) << 5;
    unsafe {
        ptr::write_bytes(&raw mut jnox_linux_net_irq_metrics, 0, 1);
        DELIVERED = 0;
        ANNOUNCED_HEAD = None;
    }
    set(19, token);
    set(20, checks);
    if checks != 63 { mask(); return false; }
    let head = u64::from(queues::rx_progress().0);
    set(17, head);
    set(18, head);
    unsafe { EPOCH = token; }
    true
}

pub(super) fn retire() {
    mask();
    let bits = read(INT_STATUS) & 3;
    if bits != 0 { write(INT_ACK, bits); }
    if unsafe { EPOCH } != 0 {
        set(23, u64::from(bits));
        set(24, u64::from(gic_read(PENDING) & LINE != 0));
        inc(26);
    }
    unsafe { EPOCH = 0; DELIVERED = 0; ANNOUNCED_HEAD = None; }
    set(22, 0);
    clear_pending();
}

pub(super) fn finish_reset() -> bool {
    clear_pending();
    read(INT_STATUS) & 3 == 0 && gic_read(ENABLE) & LINE == 0 && gic_read(PENDING) & LINE == 0
}

fn authorize(pid: usize, token: u64) -> Result<(), i64> {
    if pid != 0 { return Err(-13); }
    if token == 0 || token != unsafe { EPOCH } { return Err(-116); }
    if read(STATUS) != LIVE { return Err(-5); }
    Ok(())
}

pub(in crate::native_el0) fn wait(pid: usize, token: u64, last: u64, reserved: u64) -> i64 {
    if let Err(e) = authorize(pid, token) { return e; }
    if reserved != 0 || last != unsafe { DELIVERED } { return -22; }
    let seq = get(21);
    if seq >= (CATCHUP - 1) as u64 { return -75; }
    if queues::rx_progress().1 {
        if seq > last && unsafe { ANNOUNCED_HEAD }.is_some() {
            unsafe { DELIVERED = seq; }
            inc(9);
            return seq as i64;
        }
        inc(10);
        return CATCHUP;
    }
    // New completions after the empty check will assert the edge or have
    // already latched it in GICD. The caller commits the block before DAIF.I
    // can clear. An empty wait with RX callbacks disabled cannot make progress.
    if !queues::rx_callbacks_enabled() { return -22; }
    set(22, 1);
    inc(6);
    0
}

pub(in crate::native_el0) fn metric(pid: usize, token: u64, i: u64, reserved: u64) -> i64 {
    if let Err(e) = authorize(pid, token) { return e; }
    #[cfg(jnox_linux_source_net_rx_irq_rearm_test)]
    let last_metric = 27;
    #[cfg(not(jnox_linux_source_net_rx_irq_rearm_test))]
    let last_metric = 26;
    if reserved != 0 || i > last_metric { return -22; }
    get(i as usize) as i64
}

pub(in crate::native_el0) fn interrupt(iar: u32, idle: bool) -> Option<u64> {
    let bits = read(INT_STATUS) & 3;
    if bits != 0 { write(INT_ACK, bits); }
    if unsafe { EPOCH } == 0 { return None; }
    inc(0);
    if idle { inc(8); }
    set(11, u64::from(iar));
    set(12, u64::from(bits));
    set(13, u64::from(bits));
    if bits == 0 { inc(5); }
    if bits & 2 != 0 { inc(3); }
    if bits & 1 == 0 { return None; }
    inc(1);
    let (head, work) = queues::rx_progress();
    let prior = get(18);
    set(17, prior);
    set(18, u64::from(head));
    if !work || u64::from(head) == prior {
        inc(4);
        return None;
    }
    let seq = (get(21) + 1).min((CATCHUP - 1) as u64);
    unsafe { ANNOUNCED_HEAD = Some(head); }
    set(21, seq);
    inc(2);
    if get(22) == 0 { return None; }
    set(22, 0);
    inc(7);
    if idle { inc(25); }
    unsafe { DELIVERED = seq; }
    Some(seq)
}

pub(in crate::native_el0) fn eoi(iar: u32) {
    if unsafe { EPOCH } != 0 { inc(14); set(15, u64::from(iar)); }
}

pub(in crate::native_el0) fn timer() {
    if unsafe { EPOCH } != 0 { inc(16); }
}

/// Directed board test: assert only the GIC's software pending bit. Neither
/// a virtio status bit, an RX descriptor nor a scheduler wake is synthesized.
#[cfg(jnox_linux_source_net_rx_irq_rearm_test)]
pub(in crate::native_el0) fn pend_test(pid: usize, token: u64, arg1: u64, arg2: u64) -> i64 {
    if let Err(e) = authorize(pid, token) { return e; }
    if arg1 != 0 || arg2 != 0 { return -22; }
    if get(27) != 0 || get(0) != 0 || get(6) != 0 { return -16; }
    if read(INT_STATUS) != 0 || queues::rx_progress().1 || gic_read(PENDING) & LINE != 0 {
        return -11;
    }
    set(27, 1);
    gic_write(PENDING, LINE);
    fence();
    // DAIF.I is still set; ordinary IRQ entry performs the only IAR read.
    if gic_read(PENDING) & LINE != 0 { 0 } else { -5 }
}
