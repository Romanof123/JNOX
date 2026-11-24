#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr;

static mut BSS_PROBE: u64 = 0;

// SAFETY: The ELF loader enters _start with the platform state described in
// start.S. The assembly establishes the Rust ABI before calling kernel_main.
global_asm!(
    include_str!("start.S"),
    bss_probe = sym BSS_PROBE,
    rust_entry = sym kernel_main,
);

extern "C" fn kernel_main() -> ! {
    // SAFETY: The sole CPU has finished clearing this aligned static in RAM.
    // Taking a raw address creates no reference to the assembly-written word.
    if unsafe { ptr::read_volatile(&raw const BSS_PROBE) } != 0 {
        park();
    }

    uart_init();
    uart_write(b"JNOX M1 BOOT\nJNOX M1 READY\n");
    park()
}

// The fixed QEMU virt bench maps PL011 UART0 here. Its input clock is 24 MHz.
const UART0: usize = 0x0900_0000;

fn uart_init() {
    // SAFETY: These aligned 32-bit registers belong to the bench PL011 and
    // are outside Rust allocations. Address translation is off. This CPU is
    // its only owner, interrupts are masked, and the reset transmitter is idle.
    unsafe {
        ptr::write_volatile((UART0 + 0x30) as *mut u32, 0); // CR: disable
        ptr::write_volatile((UART0 + 0x24) as *mut u32, 13); // IBRD: 115200 baud
        ptr::write_volatile((UART0 + 0x28) as *mut u32, 1); // FBRD
        ptr::write_volatile((UART0 + 0x2c) as *mut u32, 3 << 5); // LCR_H: 8N1
        ptr::write_volatile((UART0 + 0x30) as *mut u32, (1 << 8) | 1); // TXE, UARTEN
    }
}

fn uart_write(bytes: &[u8]) {
    const TX_FIFO_FULL: u32 = 1 << 5;
    for &byte in bytes {
        // SAFETY: UARTFR is readable and UARTDR writable with aligned 32-bit
        // accesses under the MMIO and exclusive-owner assumptions of uart_init.
        unsafe {
            while ptr::read_volatile((UART0 + 0x18) as *const u32) & TX_FIFO_FULL != 0 {
                core::hint::spin_loop();
            }
            ptr::write_volatile(UART0 as *mut u32, u32::from(byte));
        }
    }
}

fn park() -> ! {
    loop {
        // SAFETY: At EL1 on this bench WFE may resume on any event. The loop
        // simply waits again and touches no memory, stack or condition flags.
        unsafe { asm!("wfe", options(nomem, nostack, preserves_flags)) }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    park()
}
