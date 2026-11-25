#[cfg(not(jnox_native_extension_register_observations))]
global_asm!(".set NATIVE_EXTENSION,1\n.set NATIVE_SERVER,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cyclic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
#[cfg(jnox_native_extension_register_observations)]
global_asm!(".set NATIVE_EXTENSION_REGISTERS,1\n.set NATIVE_EXTENSION,1\n.set NATIVE_SERVER,1\n.set NATIVE_CYCLIC,1\n.set NATIVE_MEASURED,1\n.macro measured_return\nbl native_cyclic_return\n.endm", include_str!("native_el0_vectors.S"), options(raw));
