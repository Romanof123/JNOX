#[cfg(not(jnox_native_amc_clock_page))]
global_asm!(".set NATIVE_AMC,1\n.set NATIVE_SERVER,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cyclic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_amc_clock_page)]
global_asm!(".set NATIVE_AMC_CLOCK_PAGE,1\n.set NATIVE_AMC,1\n.set NATIVE_SERVER,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cyclic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
