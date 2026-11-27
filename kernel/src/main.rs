#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr;

#[cfg(jnox_native_el0)]
#[path = "../native/elf.rs"]
mod native_elf;
#[cfg(jnox_native_el0)]
#[cfg_attr(jnox_task_policy, path = "native_task_policy.rs")]
mod native_el0;
#[cfg(all(jnox_native_el0, jnox_task_policy))]
mod task_policy_image;
#[cfg(all(jnox_native_el0, jnox_task_policy))]
mod task_policy_scheduler;
#[cfg(jnox_native_el0)]
mod native_el0_mmu;
#[cfg(any(jnox_task_policy, jnox_native_console_write))]
mod native_console;


#[cfg(jnox_bench = "aperiodic")]
mod aperiodic;
#[cfg(jnox_bench = "amc")]
mod amc;
#[cfg(any(jnox_rust_amc, jnox_bench = "amc"))]
mod amc_policy;
#[cfg(jnox_rust_amc)]
mod ordinary_amc;
#[cfg(any(jnox_native_aperiodic, jnox_ap, jnox_bench = "rr", jnox_bench = "aperiodic"))]
mod aperiodic_policy;
#[cfg(jnox_bench = "batch")]
mod batch;
#[cfg(jnox_batch)]
mod batch_runtime;
#[cfg(jnox_batch)]
#[cfg_attr(jnox_batch_telemetry, path = "batch_telemetry.rs")]
mod batch_app;
#[cfg(jnox_batch_trace)]
mod batch_trace;
#[cfg(any(jnox_native_batch, jnox_batch, jnox_bench = "batch"))]
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
#[cfg_attr(jnox_app = "coop-bounded", path = "coop_bounded_app.rs")]
mod fixed_apps;
#[cfg(jnox_app = "coop-bounded")]
mod coop_bounded;
#[cfg(any(jnox_app = "coop-events", jnox_app = "coop-single", jnox_app = "coop-bounded"))]
mod coop_peers;
#[cfg(any(jnox_app = "coop-events", jnox_app = "coop-single"))]
mod coop_service;
#[cfg(jnox_control)]
mod control_runtime;
#[cfg(jnox_control)]
mod control_app;
#[cfg(any(jnox_fixed, jnox_native_el0))]
mod fixed_policy;
#[cfg(jnox_fixed)]
mod fixed_runtime;
#[cfg(any(jnox_native_el0, jnox_eevdf_app, jnox_rust_amc, jnox_pi4_smp, jnox_ordinary_mutex, jnox_control, jnox_ap, jnox_ordinary_prop, jnox_ordinary_deadline, jnox_fixed, jnox_batch, jnox_groups, jnox_weighted, jnox_multilevel, not(any(jnox_bench = "boot", jnox_bench = "power"))))]
mod context;
#[cfg(jnox_bench = "coop")]
mod coop;
#[cfg(jnox_bench = "deadline")]
mod deadline;
#[cfg(jnox_ordinary_deadline)]
mod ordinary_deadline;
#[cfg(jnox_ordinary_deadline)]
mod deadline_jobs;
#[cfg(any(jnox_native_edf, jnox_ordinary_deadline, jnox_bench = "deadline"))]
mod deadline_policy;
#[cfg(jnox_native_periodic)]
mod native_periodic_policy;
#[cfg(any(
    jnox_bench = "scheduler",
    jnox_bench = "rr",
    jnox_control,
    jnox_bench = "periodic",
    jnox_bench = "batch",
    jnox_batch,
    jnox_native_batch,
    jnox_native_periodic,
    jnox_bench = "periodic_overrun"
))]
mod dispatch_policy;
#[cfg(any(jnox_native_amc, jnox_native_thread_fixed, jnox_pi4_migration, jnox_rust_amc, jnox_ordinary_mutex, jnox_bench = "rr", jnox_bench = "smp", jnox_bench = "mutex", jnox_bench = "amc"))]
#[cfg_attr(any(jnox_bench = "smp", jnox_bench = "mutex", jnox_bench = "amc"), allow(dead_code))]
mod domain_policy;
#[cfg(any(jnox_bench = "eevdf", jnox_bench = "eevdf_wake", jnox_bench = "eevdf_dynamic"))]
mod eevdf;
#[cfg(jnox_eevdf_app)]
mod eevdf_app;
#[cfg(jnox_eevdf_app)]
mod eevdf_app_tasks;
#[cfg(any(jnox_eevdf_app, jnox_bench = "eevdf", jnox_bench = "eevdf_wake", jnox_bench = "eevdf_dynamic"))]
mod eevdf_policy;
#[cfg(any(jnox_bench = "coop", jnox_app = "coop-events", jnox_app = "coop-single", jnox_app = "coop-bounded"))]
mod event_loop;
#[cfg(jnox_bench = "extension")]
mod extension;
#[cfg(jnox_bench = "extension")]
mod extension_policy;
#[cfg(any(jnox_native_weighted, jnox_eevdf_app, jnox_weighted, jnox_bench = "rr", jnox_bench = "vruntime", jnox_bench = "eevdf", jnox_bench = "eevdf_wake", jnox_bench = "eevdf_dynamic"))]
#[cfg_attr(any(jnox_native_weighted, jnox_eevdf_app, jnox_weighted, jnox_bench = "vruntime", jnox_bench = "eevdf", jnox_bench = "eevdf_wake", jnox_bench = "eevdf_dynamic"), allow(dead_code))]
mod fair_policy;
#[cfg(any(jnox_ordinary_prop, jnox_multilevel, jnox_native_multilevel, jnox_bench = "rr", jnox_bench = "multilevel", jnox_bench = "proportional"))]
#[cfg_attr(any(jnox_ordinary_prop, jnox_multilevel, jnox_native_multilevel, jnox_bench = "multilevel", jnox_bench = "proportional"), allow(dead_code))]
mod feedback_policy;
#[cfg(any(jnox_bench = "rr", jnox_bench = "power", jnox_bench = "extension"))]
#[cfg_attr(any(jnox_bench = "power", jnox_bench = "extension"), allow(dead_code))]
mod group_policy;
#[cfg(jnox_bench = "hierarchy")]
mod hierarchy;
#[cfg(any(jnox_groups, jnox_bench = "hierarchy"))]
mod hierarchy_policy;
#[cfg(jnox_groups)]
mod group_runtime;
#[cfg(jnox_groups)]
mod group_apps;
#[cfg(jnox_bench = "mutex")]
mod mutex;
#[cfg(any(jnox_ordinary_mutex, jnox_bench = "mutex"))]
mod mutex_policy;
#[cfg(jnox_mutex_chain)]
#[path = "ordinary_mutex_chain.rs"]
mod ordinary_mutex;
#[cfg(all(jnox_ordinary_mutex, not(any(jnox_mutex_chain, jnox_mutex_fifo, jnox_mutex_survivors))))]
mod ordinary_mutex;
#[cfg(jnox_mutex_fifo)]
mod ordinary_mutex_fifo;
#[cfg(jnox_mutex_survivors)]
mod ordinary_mutex_survivors;
#[cfg(all(jnox_ordinary_mutex, not(any(jnox_mutex_chain, jnox_mutex_fifo, jnox_mutex_survivors))))]
mod ordinary_mutex_manifest;
#[cfg(jnox_bench = "multilevel")]
mod multilevel;
#[cfg(any(jnox_multilevel, jnox_native_multilevel, jnox_bench = "multilevel"))]
mod multilevel_policy;
#[cfg(jnox_multilevel)]
mod multilevel_service;
#[cfg(jnox_mlq_admission)]
mod mlq_admission;
#[cfg(jnox_mlq_admission)]
mod mlq_bound;
#[cfg(jnox_mlfq_wake)]
mod mlfq_wake;
#[cfg(jnox_sched_app_trace)]
mod sched_app_trace;
#[cfg(jnox_mlfq_wake)]
mod wake_bound;
#[cfg(jnox_multilevel)]
mod multilevel_apps;
#[cfg(jnox_bench = "partition")]
#[cfg_attr(jnox_ordinary_partition, path = "ordinary_partition.rs")]
mod partition;
#[cfg(jnox_bench = "periodic")]
mod periodic;
#[cfg(jnox_bench = "power")]
mod power;
#[cfg(jnox_bench = "power")]
mod power_policy;
#[cfg(jnox_bench = "proportional")]
mod proportional;
#[cfg(any(jnox_ordinary_prop, jnox_bench = "proportional"))]
mod proportional_policy;
#[cfg(jnox_ordinary_prop)]
mod ordinary_proportional;
#[cfg(any(
    jnox_bench = "scheduler",
    jnox_bench = "rr",
    jnox_control,
    jnox_bench = "periodic",
    jnox_bench = "batch",
    jnox_batch,
    jnox_fixed,
    jnox_multilevel,
    jnox_ordinary_deadline,
    jnox_bench = "deadline",
    jnox_bench = "coop",
    jnox_ordinary_mutex,
    jnox_bench = "mutex",
    jnox_bench = "multilevel",
    jnox_bench = "priority_resume",
    jnox_native_el0,
    jnox_bench = "periodic_overrun"
))]
mod ready_queue;
#[cfg(any(jnox_ap, jnox_bench = "aperiodic"))]
mod request_server;
#[cfg(jnox_ap)]
mod ordinary_server;
#[cfg(jnox_ap_admission)]
mod aperiodic_admission;
#[cfg(any(jnox_bench = "scheduler", jnox_bench = "rr"))]
mod sched;
#[cfg(jnox_bench = "rr")]
mod service_policy;
#[cfg(jnox_bench = "smp")]
mod smp;
#[cfg(any(jnox_bench = "smp", jnox_bench = "power"))]
mod smp_boot;
#[cfg(any(jnox_pi4_migration, jnox_bench = "smp"))]
mod smp_policy;
#[cfg(any(jnox_native_partition, jnox_native_cbs, jnox_bench = "rr", jnox_bench = "cbs", jnox_bench = "cbs_wake", jnox_bench = "partition"))]
mod temporal_policy;
#[cfg(jnox_bench = "vruntime")]
mod vruntime;
#[cfg(jnox_weighted)]
mod weighted;
#[cfg(any(jnox_native_weighted, jnox_weighted, jnox_bench = "vruntime"))]
mod vruntime_policy;

#[cfg(jnox_pi4_smp)]
mod pi4_smp_bsp;
#[cfg(all(jnox_pi4_smp, not(jnox_pi4_migration)))]
mod pi4_smp_workload;
#[cfg(jnox_pi4_migration)]
mod pi4_migration;
#[cfg(jnox_pi4_migration)]
mod pi4_migration_local;
#[cfg(jnox_pi4_migration)]
mod pi4_migration_tasks;
#[cfg(jnox_pi4_migration)]
mod pi4_migration_log;

static mut BSS_PROBE: u64 = 0;

#[cfg(target_arch = "aarch64")]
#[path = "../../arch/aarch64/mod.rs"]
mod arch;

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
    #[cfg(jnox_native_el0)]
    native_el0::run();
    #[cfg(jnox_pi4_smp)]
    pi4_smp_bsp::primary();
    #[cfg(jnox_fixed)]
    fixed_apps::run();
    #[cfg(jnox_batch)]
    batch_app::run();
    #[cfg(jnox_ap)]
    ordinary_server::run();
    #[cfg(jnox_ordinary_deadline)]
    ordinary_deadline::run();
    #[cfg(jnox_ordinary_prop)]
    ordinary_proportional::run();
    #[cfg(all(jnox_ordinary_mutex, not(any(jnox_mutex_fifo, jnox_mutex_survivors))))]
    ordinary_mutex::run();
    #[cfg(jnox_mutex_fifo)]
    ordinary_mutex_fifo::run();
    #[cfg(jnox_mutex_survivors)]
    ordinary_mutex_survivors::run();
    #[cfg(jnox_eevdf_app)]
    eevdf_app::run();
    #[cfg(jnox_rust_amc)]
    ordinary_amc::run();
    #[cfg(jnox_weighted)]
    weighted::run();
    #[cfg(jnox_groups)]
    group_apps::run();
    #[cfg(jnox_multilevel)]
    multilevel_apps::run();
    #[cfg(jnox_control)]
    control_runtime::run();
    #[cfg(not(any(jnox_native_el0, jnox_eevdf_app, jnox_rust_amc, jnox_pi4_smp, jnox_ordinary_mutex, jnox_control, jnox_ap, jnox_ordinary_prop, jnox_ordinary_deadline, jnox_fixed, jnox_batch, jnox_groups, jnox_weighted, jnox_multilevel)))]
    run_bench()
}

#[cfg(not(any(jnox_native_el0, jnox_eevdf_app, jnox_rust_amc, jnox_pi4_smp, jnox_ordinary_mutex, jnox_control, jnox_ap, jnox_ordinary_prop, jnox_ordinary_deadline, jnox_fixed, jnox_batch, jnox_groups, jnox_weighted, jnox_multilevel)))]
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
    #[cfg(any(jnox_bench = "eevdf", jnox_bench = "eevdf_wake", jnox_bench = "eevdf_dynamic"))]
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
        let rx = if cfg!(any(jnox_native_display, jnox_native_shell)) { 1 << 9 } else { 0 };
        ptr::write_volatile((UART0 + 0x30) as *mut u32, rx | (1 << 8) | 1); // RXE, TXE, UARTEN
    }
}

/// Read at most one PL011 byte with CPU0 interrupts masked.
#[cfg(any(jnox_native_display, jnox_native_shell))]
pub(crate) fn uart_try_read() -> Result<u8, i64> {
    unsafe {
        if ((UART0 + 0x18) as *const u32).read_volatile() & (1 << 4) != 0 {
            return Err(-11); // EAGAIN: RX FIFO empty
        }
        let data = (UART0 as *const u32).read_volatile();
        if data & 0xf00 != 0 {
            ((UART0 + 4) as *mut u32).write_volatile(0); // PL011 error clear
            return Err(-5); // EIO
        }
        Ok(data as u8)
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
