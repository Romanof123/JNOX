//! Optional bounded observations. No record influences scheduling or task work.
use super::{context::Frame, uart_write};

const LIMIT: usize = 64;
#[unsafe(export_name = "jnox_batch_events")]
static mut EVENTS: [[u64; 15]; LIMIT] = [[0; 15]; LIMIT];
static mut COUNT: usize = 0;

pub(super) fn record(
    kind: u64,
    now: u64,
    old: u64,
    new: u64,
    from: *mut Frame,
    to: *mut Frame,
    request: [u64; 5],
    remaining: u64,
) {
    let ctl: u64;
    let compare: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntp_ctl_el0", "mrs {}, cntp_cval_el0",
        out(reg) ctl, out(reg) compare, options(nostack))
    };

    // Called exclusively under exception masks, before returning either frame.
    unsafe {
        let i = COUNT;
        COUNT = i.saturating_add(1);
        if i < LIMIT {
            EVENTS[i] = [
                kind,
                now,
                old,
                new,
                from.addr() as u64,
                if from.is_null() { 0 } else { (*from).elr },
                to.addr() as u64,
                (*to).elr,
                request[0],
                request[1],
                request[2],
                request[3],
                request[4],
                remaining,
                if ctl & 1 == 1 { compare } else { 0 },
            ];
        }
    }
}

static EMPTY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
pub(super) fn mark_empty() {
    EMPTY.store(true, core::sync::atomic::Ordering::Release);
}
pub(super) fn idle_observation(pc: u64, sp: u64) {
    if !EMPTY.swap(false, core::sync::atomic::Ordering::AcqRel) {
        return;
    }
    uart_write(b"BATCH IDLE EMPTY");
    word(pc);
    word(sp);
    uart_write(b"\n");
    let count = unsafe { COUNT };
    for i in 0..count.min(LIMIT) {
        uart_write(b"BATCH EVENT");
        for value in unsafe { EVENTS[i] } {
            word(value);
        }
        uart_write(b"\n");
    }
    if count > LIMIT {
        uart_write(b"BATCH TRACE TRUNCATED\n");
    }
    uart_write(b"BATCH TRACE END\n");
}

fn word(mut value: u64) {
    let mut hex = [b'0'; 16];
    for digit in hex.iter_mut().rev() {
        *digit = b"0123456789abcdef"[(value & 15) as usize];
        value >>= 4;
    }
    uart_write(b" ");
    uart_write(&hex);
}
