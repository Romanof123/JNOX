//! Single-core QEMU virt PL011 receive interrupts. The GIC and UART pages and
//! the receive ring belong to EL1. All entries run with CPU0 IRQs masked.
use crate::{context::{GICD, gic_read, gic_write}, uart_try_read, UART0};
use core::{arch::asm, ptr};

pub(super) const IRQ: u32 = 33; // virt UART0, SPI 1
const LINE: u32 = 1 << (IRQ % 32);
const GIC_WORD: usize = (IRQ as usize / 32) * 4;
const UART_IMSC: usize = UART0 + 0x38;
const UART_MIS: usize = UART0 + 0x40;
const UART_ICR: usize = UART0 + 0x44;
const RX: u32 = 1 << 4;
const RX_TIMEOUT: u32 = 1 << 6;
const CAPACITY: usize = 16;
const DRAIN_LIMIT: usize = 16;

struct Receiver {
    bytes: [u8; CAPACITY],
    head: usize,
    len: usize,
    interrupts: u32,
    received: u32,
    drops: u32,
    errors: u32,
    idle_interrupts: u32,
}

static mut RECEIVER: Receiver = Receiver {
    bytes: [0; CAPACITY], head: 0, len: 0, interrupts: 0,
    received: 0, drops: 0, errors: 0, idle_interrupts: 0,
};

fn barrier() {
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
}

pub(super) fn enable() {
    // The native timer configured CPU0's GIC interface and Group 0. Configure
    // only the UART's distributor fields, with level trigger and CPU0 target.
    gic_write(GICD + 0x180 + GIC_WORD, LINE); // ICENABLER
    barrier();
    let group = GICD + 0x080 + GIC_WORD;
    gic_write(group, gic_read(group) & !LINE);
    let config = GICD + 0xc00 + (IRQ as usize / 16) * 4;
    gic_write(config, gic_read(config) & !(2 << (2 * (IRQ % 16))));
    unsafe {
        ptr::write_volatile((GICD + 0x400 + IRQ as usize) as *mut u8, 0xa0);
        ptr::write_volatile((GICD + 0x800 + IRQ as usize) as *mut u8, 1);
    }
    gic_write(GICD + 0x280 + GIC_WORD, LINE); // ICPENDR
    gic_write(GICD + 0x100 + GIC_WORD, LINE); // ISENABLER
    barrier();
    // Keep an already received byte pending. FIFO-disabled UART0 signals RX
    // as soon as a character is available; TX remains polling-only.
    unsafe { ptr::write_volatile(UART_IMSC as *mut u32, RX | RX_TIMEOUT) };
    barrier();
}

pub(super) fn disable() {
    unsafe { ptr::write_volatile(UART_IMSC as *mut u32, 0) };
    barrier();
    gic_write(GICD + 0x180 + GIC_WORD, LINE);
    gic_write(GICD + 0x280 + GIC_WORD, LINE);
    barrier();
    // The static package has no replacement display owner.
    let state = unsafe { &mut *(&raw mut RECEIVER) };
    state.head = 0;
    state.len = 0;
    state.bytes.fill(0);
}

pub(super) fn interrupt(idle: bool) {
    let state = unsafe { &mut *(&raw mut RECEIVER) };
    state.interrupts = state.interrupts.saturating_add(1);
    if idle {
        state.idle_interrupts = state.idle_interrupts.saturating_add(1);
    }
    let pending = unsafe { ptr::read_volatile(UART_MIS as *const u32) };
    if pending & RX_TIMEOUT != 0 {
        unsafe { ptr::write_volatile(UART_ICR as *mut u32, RX_TIMEOUT) };
    }
    // Reading DR deasserts the level on an empty receiver. Do not clear RX
    // via ICR: any unconsumed input must be able to trigger another entry.
    for _ in 0..DRAIN_LIMIT {
        match uart_try_read() {
            Ok(byte) => {
                state.received = state.received.saturating_add(1);
                if state.len == CAPACITY {
                    state.drops = state.drops.saturating_add(1);
                } else {
                    state.bytes[(state.head + state.len) % CAPACITY] = byte;
                    state.len += 1;
                }
            }
            Err(-11) => break,
            Err(_) => state.errors = state.errors.saturating_add(1),
        }
    }
}

pub(super) fn pop() -> Result<u8, i64> {
    let state = unsafe { &mut *(&raw mut RECEIVER) };
    if state.len == 0 {
        return Err(-11);
    }
    let byte = state.bytes[state.head];
    state.head = (state.head + 1) % CAPACITY;
    state.len -= 1;
    Ok(byte)
}

pub(super) fn statistic(metric: u64) -> i64 {
    let state = unsafe { &*(&raw const RECEIVER) };
    match metric {
        0 => i64::from(state.interrupts),
        1 => i64::from(state.received),
        2 => i64::from(state.drops),
        3 => i64::from(state.errors),
        4 => state.len as i64,
        5 => CAPACITY as i64,
        6 => i64::from(state.idle_interrupts),
        _ => -22,
    }
}
