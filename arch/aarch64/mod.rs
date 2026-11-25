use core::arch::global_asm;

#[cfg(all(jnox_pi4, jnox_native_el0))]
global_asm!(include_str!("pi4_native_shim.S"));

// SAFETY: The ELF loader enters _start with the platform state described in
// start.S. The assembly establishes the Rust ABI before calling kernel_main.
global_asm!(
    include_str!("start.S"),
    pi4_native = const cfg!(all(jnox_pi4, jnox_native_el0)) as usize,
    cold = const cfg!(jnox_control_cold) as usize,
    pi4 = const cfg!(jnox_pi4) as usize,
    pi4_scheduler = const cfg!(all(jnox_pi4, any(jnox_bench = "scheduler", jnox_fixed, jnox_pi4_smp, jnox_native_el0))) as usize,
    native_partitioned = const cfg!(jnox_native_smp_partitioned) as usize,
    pi4_smp = const cfg!(jnox_pi4_smp) as usize,
    bss_probe = sym super::BSS_PROBE,
    rust_entry = sym super::kernel_main,
);
