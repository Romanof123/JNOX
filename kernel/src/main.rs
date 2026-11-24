#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr;

#[cfg(all(
    feature = "rr-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "periodic-bench",
        feature = "partition-bench"
    ))
))]
mod aperiodic_policy;
#[cfg(all(
    feature = "cbs-bench",
    not(any(feature = "partition-bench", feature = "coop-bench"))
))]
mod cbs;
#[cfg(any(
    feature = "sched-bench",
    feature = "cbs-bench",
    feature = "partition-bench"
))]
mod context;
#[cfg(feature = "coop-bench")]
mod coop;
#[cfg(all(
    feature = "sched-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "partition-bench"
    ))
))]
mod dispatch_policy;
#[cfg(all(
    feature = "rr-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "periodic-bench",
        feature = "partition-bench"
    ))
))]
mod domain_policy;
#[cfg(feature = "coop-bench")]
mod event_loop;
#[cfg(all(
    feature = "rr-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "periodic-bench",
        feature = "partition-bench"
    ))
))]
mod fair_policy;
#[cfg(all(
    feature = "rr-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "periodic-bench",
        feature = "partition-bench"
    ))
))]
mod feedback_policy;
#[cfg(all(
    feature = "rr-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "periodic-bench",
        feature = "partition-bench"
    ))
))]
mod group_policy;
#[cfg(any(
    feature = "coop-bench",
    all(
        feature = "sched-bench",
        not(any(feature = "cbs-bench", feature = "partition-bench"))
    )
))]
mod ready_queue;
#[cfg(all(
    feature = "sched-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "periodic-bench",
        feature = "partition-bench"
    ))
))]
mod sched;
#[cfg(all(
    feature = "rr-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "periodic-bench",
        feature = "partition-bench"
    ))
))]
mod service_policy;
#[cfg(not(feature = "coop-bench"))]
#[cfg(any(
    feature = "partition-bench",
    feature = "cbs-bench",
    all(feature = "rr-bench", not(feature = "periodic-bench"))
))]
mod temporal_policy;

#[cfg(all(
    feature = "periodic-bench",
    not(any(
        feature = "coop-bench",
        feature = "cbs-bench",
        feature = "partition-bench"
    ))
))]
mod periodic;

#[cfg(all(feature = "partition-bench", not(feature = "coop-bench")))]
mod partition;

static mut BSS_PROBE: u64 = 0;

// SAFETY: The ELF loader enters _start with the platform state described in
// start.S. The assembly establishes the Rust ABI before calling kernel_main.
global_asm!(
    include_str!("start.S"),
    pi4 = const cfg!(jnox_pi4) as usize,
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
    #[cfg(jnox_pi4)]
    uart_write(b"JNOX PI4 BOOT\nJNOX PI4 READY\n");
    #[cfg(not(jnox_pi4))]
    uart_write(b"JNOX M1 BOOT\nJNOX M1 READY\n");
    #[cfg(feature = "coop-bench")]
    coop::run();
    #[cfg(not(feature = "coop-bench"))]
    run_bench()
}

#[cfg(not(feature = "coop-bench"))]
fn run_bench() -> ! {
    #[cfg(all(
        feature = "rr-bench",
        not(any(
            feature = "cbs-bench",
            feature = "periodic-bench",
            feature = "partition-bench"
        ))
    ))]
    {
        if !dispatch_policy::bench_checks() {
            uart_write(b"POLICY RANK FAIL\n");
            park();
        }
        uart_write(b"POLICY RANK PASS\n");
        if !(service_policy::bench_checks()
            && temporal_policy::bench_checks()
            && fair_policy::bench_checks()
            && feedback_policy::bench_checks()
            && aperiodic_policy::bench_checks()
            && domain_policy::bench_checks()
            && group_policy::bench_checks())
        {
            uart_write(b"POLICY SERVICE FAIL\n");
            park();
        }
        uart_write(b"POLICY SERVICE PASS\n");
    }
    #[cfg(all(
        feature = "sched-bench",
        not(any(feature = "cbs-bench", feature = "partition-bench"))
    ))]
    #[cfg(not(feature = "periodic-bench"))]
    sched::run();
    #[cfg(all(
        feature = "periodic-bench",
        not(any(feature = "cbs-bench", feature = "partition-bench"))
    ))]
    periodic::run();
    #[cfg(all(feature = "cbs-bench", not(feature = "partition-bench")))]
    cbs::run();
    #[cfg(feature = "partition-bench")]
    partition::run();
    #[cfg(not(any(
        feature = "sched-bench",
        feature = "cbs-bench",
        feature = "partition-bench"
    )))]
    park()
}

// The fixed QEMU virt bench maps PL011 UART0 here. Its input clock is 24 MHz.
#[cfg(not(jnox_pi4))]
const UART0: usize = 0x0900_0000;
#[cfg(jnox_pi4)]
const UART0: usize = 0xfe20_1000;

#[cfg(jnox_pi4)]
fn pi4_uart_pins() {
    // BCM2711 low-peripheral mode, GPIO chapter. The boot CPU owns GPIO14/15
    // (header pins 8/10), and preserves all other fields in these registers.
    let fsel1 = 0xfe20_0004 as *mut u32;
    let pulls0 = 0xfe20_00e4 as *mut u32;
    unsafe {
        let pulls = ptr::read_volatile(pulls0) & !(0xf << 28);
        ptr::write_volatile(pulls0, pulls);
        let pins = ptr::read_volatile(fsel1) & !(0x3f << 12);
        ptr::write_volatile(fsel1, pins | (4 << 12) | (4 << 15));
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}

fn uart_init() {
    #[cfg(jnox_pi4)]
    {
        pi4_uart_pins();
        // config.txt leaves firmware UART logging off. Drain any outstanding
        // byte before changing its format and disable all UART interrupts.
        unsafe {
            while ptr::read_volatile((UART0 + 0x18) as *const u32) & (1 << 3) != 0 {
                core::hint::spin_loop();
            }
            ptr::write_volatile((UART0 + 0x38) as *mut u32, 0);
            ptr::write_volatile((UART0 + 0x44) as *mut u32, 0x7ff);
            ptr::write_volatile((UART0 + 0x48) as *mut u32, 0);
        }
    }
    // 48 MHz / (16 * 115200) rounds to 26 + 3/64; virt uses 24 MHz.
    let (integer, fraction) = if cfg!(jnox_pi4) { (26, 3) } else { (13, 1) };
    // SAFETY: These aligned 32-bit registers belong to the bench PL011 and
    // are outside Rust allocations. Address translation is off. This CPU is
    // its only owner, interrupts are masked, and the reset transmitter is idle.
    unsafe {
        ptr::write_volatile((UART0 + 0x30) as *mut u32, 0); // CR: disable
        ptr::write_volatile((UART0 + 0x24) as *mut u32, integer); // IBRD: 115200 baud
        ptr::write_volatile((UART0 + 0x28) as *mut u32, fraction); // FBRD
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
