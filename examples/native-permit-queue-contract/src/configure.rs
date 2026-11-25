// These selectors configure only the private copies linked into this example.
fn main() {
    for selector in [
        "jnox_native_safety_channel",
        "jnox_native_safety_boundary",
        "jnox_native_safety_arrival_observation",
        "jnox_native_safety_watchdog",
        "jnox_native_safety_qualification",
        "jnox_native_safety_downstream",
        "jnox_native_safety_phase_observation",
    ] {
        println!("cargo::rustc-check-cfg=cfg({selector})");
    }
    println!("cargo::rustc-cfg=jnox_native_safety_channel");
    println!("cargo::rustc-cfg=jnox_native_safety_boundary");
    println!("cargo::rustc-cfg=jnox_native_safety_arrival_observation");
    println!("cargo::rerun-if-changed=src/configure.rs");
}
