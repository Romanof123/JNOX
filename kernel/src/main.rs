#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr;

#[cfg(jnox_bench = "aperiodic")]
mod aperiodic;
#[cfg(jnox_bench = "amc")]
mod amc;
#[cfg(jnox_bench = "amc")]
mod amc_policy;
#[cfg(any(jnox_bench = "rr", jnox_bench = "aperiodic"))]
mod aperiodic_policy;
#[cfg(jnox_bench = "batch")]
mod batch;
#[cfg(jnox_batch)]
mod batch_runtime;
#[cfg(jnox_batch)]
mod batch_app;
#[cfg(jnox_batch_trace)]
mod batch_trace;
#[cfg(any(jnox_batch, jnox_bench = "batch"))]
mod batch_policy;
#[cfg(jnox_bench = "cbs")]
mod cbs;
#[cfg(jnox_bench = "priority_resume")]
mod priority_resume;
#[cfg(jnox_bench = "periodic_overrun")]
mod periodic_overrun;
#[cfg(jnox_bench = "cbs_wake")]
mod cbs_wake;
#[cfg(jnox_fixed)]
#[cfg_attr(any(jnox_app = "fixed-ties", jnox_app = "fixed-late"), path = "fixed_edge_apps.rs")]
#[cfg_attr(any(jnox_app = "coop-events", jnox_app = "coop-single"), path = "coop_app.rs")]
mod fixed_apps;
#[cfg(any(jnox_app = "coop-events", jnox_app = "coop-single"))]
mod coop_service;
#[cfg(jnox_fixed)]
mod fixed_policy;
#[cfg(jnox_fixed)]
mod fixed_runtime;
#[cfg(any(jnox_fixed, jnox_batch, not(any(jnox_bench = "boot", jnox_bench = "power"))))]
mod context;
#[cfg(jnox_bench = "coop")]
mod coop;
#[cfg(jnox_bench = "deadline")]
mod deadline;
#[cfg(jnox_bench = "deadline")]
mod deadline_policy;
#[cfg(any(
    jnox_bench = "scheduler",
    jnox_bench = "rr",
    jnox_bench = "periodic",
    jnox_bench = "batch",
    jnox_batch,
    jnox_bench = "periodic_overrun"
))]
mod dispatch_policy;
#[cfg(any(jnox_bench = "rr", jnox_bench = "smp", jnox_bench = "mutex", jnox_bench = "amc"))]
#[cfg_attr(any(jnox_bench = "smp", jnox_bench = "mutex", jnox_bench = "amc"), allow(dead_code))]
mod domain_policy;
#[cfg(any(jnox_bench = "eevdf", jnox_bench = "eevdf_wake"))]
mod eevdf;
#[cfg(any(jnox_bench = "eevdf", jnox_bench = "eevdf_wake"))]
mod eevdf_policy;
#[cfg(any(jnox_bench = "coop", jnox_app = "coop-events", jnox_app = "coop-single"))]
mod event_loop;
#[cfg(jnox_bench = "extension")]
mod extension;
#[cfg(jnox_bench = "extension")]
mod extension_policy;
#[cfg(any(jnox_bench = "rr", jnox_bench = "vruntime", jnox_bench = "eevdf", jnox_bench = "eevdf_wake"))]
#[cfg_attr(any(jnox_bench = "vruntime", jnox_bench = "eevdf", jnox_bench = "eevdf_wake"), allow(dead_code))]
mod fair_policy;
#[cfg(any(jnox_bench = "rr", jnox_bench = "multilevel", jnox_bench = "proportional"))]
#[cfg_attr(any(jnox_bench = "multilevel", jnox_bench = "proportional"), allow(dead_code))]
mod feedback_policy;
#[cfg(any(jnox_bench = "rr", jnox_bench = "power", jnox_bench = "extension"))]
#[cfg_attr(any(jnox_bench = "power", jnox_bench = "extension"), allow(dead_code))]
mod group_policy;
#[cfg(jnox_bench = "hierarchy")]
mod hierarchy;
#[cfg(jnox_bench = "hierarchy")]
mod hierarchy_policy;
#[cfg(jnox_bench = "mutex")]
mod mutex;
#[cfg(jnox_bench = "mutex")]
mod mutex_policy;
#[cfg(jnox_bench = "multilevel")]
mod multilevel;
#[cfg(jnox_bench = "multilevel")]
mod multilevel_policy;
#[cfg(jnox_bench = "partition")]
mod partition;
#[cfg(jnox_bench = "periodic")]
mod periodic;
#[cfg(jnox_bench = "power")]
mod power;
#[cfg(jnox_bench = "power")]
mod power_policy;
#[cfg(jnox_bench = "proportional")]
mod proportional;
#[cfg(jnox_bench = "proportional")]
mod proportional_policy;
#[cfg(any(
    jnox_bench = "scheduler",
    jnox_bench = "rr",
    jnox_bench = "periodic",
    jnox_bench = "batch",
    jnox_batch,
    jnox_fixed,
    jnox_bench = "deadline",
    jnox_bench = "coop",
    jnox_bench = "mutex",
    jnox_bench = "multilevel",
    jnox_bench = "priority_resume",
    jnox_bench = "periodic_overrun"
))]
mod ready_queue;
#[cfg(jnox_bench = "aperiodic")]
mod request_server;
#[cfg(any(jnox_bench = "scheduler", jnox_bench = "rr"))]
mod sched;
#[cfg(jnox_bench = "rr")]
mod service_policy;
#[cfg(jnox_bench = "smp")]
mod smp;
#[cfg(any(jnox_bench = "smp", jnox_bench = "power"))]
mod smp_boot;
#[cfg(jnox_bench = "smp")]
mod smp_policy;
#[cfg(any(jnox_bench = "rr", jnox_bench = "cbs", jnox_bench = "cbs_wake", jnox_bench = "partition"))]
mod temporal_policy;
#[cfg(jnox_bench = "vruntime")]
mod vruntime;
#[cfg(jnox_bench = "vruntime")]
mod vruntime_policy;

static mut BSS_PROBE: u64 = 0;

// SAFETY: The ELF loader enters _start with the platform state described in
// start.S. The assembly establishes the Rust ABI before calling kernel_main.
global_asm!(
    include_str!("start.S"),
    pi4 = const cfg!(jnox_pi4) as usize,
    pi4_scheduler = const cfg!(all(jnox_pi4, any(jnox_bench = "scheduler", jnox_fixed))) as usize,
    bss_probe = sym BSS_PROBE,
    rust_entry = sym kernel_main,
);

extern "C" fn kernel_main() -> ! {
    // SAFETY: The boot CPU has finished clearing this aligned static in RAM.
    // Taking a raw address creates no reference to the assembly-written word.
    if unsafe { ptr::read_volatile(&raw const BSS_PROBE) } != 0 {
        park();
    }

    uart_init();
    #[cfg(jnox_pi4)]
    uart_write(b"JNOX PI4 BOOT\nJNOX PI4 READY\n");
    #[cfg(not(jnox_pi4))]
    uart_write(b"JNOX M1 BOOT\nJNOX M1 READY\n");
    #[cfg(jnox_fixed)]
    fixed_apps::run();
    #[cfg(jnox_batch)]
    batch_app::run();
    #[cfg(not(any(jnox_fixed, jnox_batch)))]
    run_bench()
}

#[cfg(not(any(jnox_fixed, jnox_batch)))]
fn run_bench() -> ! {
    #[cfg(jnox_bench = "priority_resume")]
    priority_resume::run();
    #[cfg(jnox_bench = "periodic_overrun")]
    periodic_overrun::run();
    #[cfg(jnox_bench = "cbs_wake")]
    cbs_wake::run();
    #[cfg(jnox_bench = "proportional")]
    proportional::run();
    #[cfg(jnox_bench = "vruntime")]
    vruntime::run();
    #[cfg(jnox_bench = "amc")]
    amc::run();
    #[cfg(jnox_bench = "multilevel")]
    multilevel::run();
    #[cfg(jnox_bench = "mutex")]
    mutex::run();
    #[cfg(any(jnox_bench = "smp", jnox_bench = "power"))]
    smp_boot::boot();
    #[cfg(any(jnox_bench = "eevdf", jnox_bench = "eevdf_wake"))]
    eevdf::run();
    #[cfg(jnox_bench = "hierarchy")]
    hierarchy::run();
    #[cfg(jnox_bench = "extension")]
    extension::run();
    #[cfg(jnox_bench = "rr")]
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
    #[cfg(any(jnox_bench = "scheduler", jnox_bench = "rr"))]
    sched::run();
    #[cfg(jnox_bench = "periodic")]
    periodic::run();
    #[cfg(jnox_bench = "cbs")]
    cbs::run();
    #[cfg(jnox_bench = "partition")]
    partition::run();
    #[cfg(jnox_bench = "batch")]
    batch::run();
    #[cfg(jnox_bench = "deadline")]
    deadline::run();
    #[cfg(jnox_bench = "aperiodic")]
    aperiodic::run();
    #[cfg(jnox_bench = "coop")]
    coop::run();
    #[cfg(jnox_bench = "boot")]
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
