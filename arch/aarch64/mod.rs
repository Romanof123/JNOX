use core::arch::global_asm;

// SAFETY: The ELF loader enters _start with the platform state described in
// start.S. The assembly establishes the Rust ABI before calling kernel_main.
global_asm!(
    include_str!("start.S"),
    cold = const cfg!(jnox_control_cold) as usize,
    pi4 = const cfg!(jnox_pi4) as usize,
    pi4_scheduler = const cfg!(all(jnox_pi4, any(jnox_bench = "scheduler", jnox_fixed))) as usize,
    bss_probe = sym super::BSS_PROBE,
    rust_entry = sym super::kernel_main,
);
