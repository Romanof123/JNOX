//! CPU0's PL031 resource. EL0 sees register offsets and one latched SPI event.
//! The native syscall and interrupt vectors serialize every operation with IRQs masked.
use crate::{context::{gic_read, gic_write, GICD}, native_el0_mmu::PL031};
use core::{arch::asm, ptr};

pub(super) const IRQ: u32 = 34;
const LINE: u32 = 1 << (IRQ % 32);
const WORD: usize = (IRQ as usize / 32) * 4;
const DR: u64 = 0;
const MR: u64 = 4;
const LR: u64 = 8;
const CR: u64 = 12;
const IMSC: u64 = 16;
const RIS: u64 = 20;
const MIS: u64 = 24;
const ICR: u64 = 28;

static mut OWNED: bool = false;
static mut PENDING: bool = false;
static mut IRQ_MASKED: bool = false;

fn barrier() {
    unsafe { asm!("dsb sy", options(nostack, preserves_flags)) }
}

fn read(offset: u64) -> u32 {
    unsafe { ptr::read_volatile((PL031 + offset as usize) as *const u32) }
}

fn write(offset: u64, value: u32) {
    unsafe { ptr::write_volatile((PL031 + offset as usize) as *mut u32, value) };
    barrier();
}

fn disable_line() {
    gic_write(GICD + 0x180 + WORD, LINE);
    barrier();
}

fn enable_line() {
    gic_write(GICD + 0x100 + WORD, LINE);
    barrier();
}

fn clear_pending() {
    gic_write(GICD + 0x280 + WORD, LINE);
    barrier();
}

fn quiesce() {
    disable_line();
    write(IMSC, 0);
    write(ICR, 1);
    clear_pending();
    unsafe { PENDING = false; IRQ_MASKED = false; }
}

fn claim() -> i64 {
    if unsafe { OWNED } { return -16; }
    quiesce();
    let group = GICD + 0x080 + WORD;
    gic_write(group, gic_read(group) & !LINE);
    let config = GICD + 0xc00 + (IRQ as usize / 16) * 4;
    gic_write(config, gic_read(config) & !(2 << (2 * (IRQ % 16))));
    unsafe {
        ptr::write_volatile((GICD + 0x400 + IRQ as usize) as *mut u8, 0xa0);
        ptr::write_volatile((GICD + 0x800 + IRQ as usize) as *mut u8, 1);
        OWNED = true;
    }
    enable_line();
    0
}

pub(super) fn release_if_owned() {
    if unsafe { OWNED } {
        quiesce();
        unsafe { OWNED = false };
    }
}

pub(super) fn interrupt() {
    disable_line();
    if unsafe { OWNED } {
        unsafe { PENDING = true; IRQ_MASKED = true; }
    }
}

fn write_register(offset: u64, value: u64) -> i64 {
    if !matches!(offset, MR | LR | CR | IMSC | ICR) { return -22; }
    let Ok(word) = u32::try_from(value) else { return -22; };
    if matches!(offset, CR | IMSC | ICR) && word > 1 { return -22; }
    write(offset, word);
    if offset == ICR && word == 1 && unsafe { IRQ_MASKED } {
        // The source is now deasserted. Remove any latched GIC pending bit
        // from the old level before allowing the next alarm through.
        clear_pending();
        unsafe { IRQ_MASKED = false; PENDING = false; }
        enable_line();
    }
    0
}

pub(super) fn service(pid: usize, number: u64, offset: u64, value: u64) -> i64 {
    if pid != 0 { return -13; }
    if number == 155 && offset == 1 { return claim(); }
    if !unsafe { OWNED } { return -19; }
    match number {
        152 if matches!(offset, DR | MR | LR | CR | IMSC | RIS | MIS)
            || ((0xfe0..=0xffc).contains(&offset) && offset % 4 == 0) => i64::from(read(offset)),
        152 => -22,
        153 => write_register(offset, value),
        154 => unsafe {
            let pending = PENDING;
            PENDING = false;
            i64::from(pending)
        },
        155 if offset == 0 => { release_if_owned(); 0 },
        155 => -22,
        _ => -38,
    }
}
