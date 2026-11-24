use std::env;

fn main() {
    println!("cargo::rerun-if-env-changed=JNOX_BOARD");
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4)");
    // A single selected backend owns the vector table. Cargo sets these
    // variables for enabled features and rebuilds the script for a new set.
    let bench = [
        ("COOP", "coop"),
        ("BATCH", "batch"),
        ("PARTITION", "partition"),
        ("CBS", "cbs"),
        ("PERIODIC", "periodic"),
        ("RR", "rr"),
        ("SCHED", "scheduler"),
    ]
    .iter()
    .find(|(feature, _)| env::var_os(format!("CARGO_FEATURE_{feature}_BENCH")).is_some())
    .map_or("boot", |(_, profile)| *profile);
    println!(
        r#"cargo::rustc-check-cfg=cfg(jnox_bench, values("boot", "scheduler", "rr", "periodic", "cbs", "partition", "batch", "coop"))"#
    );
    println!("cargo::rustc-cfg=jnox_bench=\"{bench}\"");
    let board = match env::var("JNOX_BOARD") {
        Ok(board) => board,
        Err(env::VarError::NotPresent) => "virt".into(),
        Err(error) => panic!("invalid JNOX_BOARD: {error}"),
    };
    let script = match board.as_str() {
        "virt" => "link.ld",
        "pi4" => {
            assert_eq!(bench, "boot", "JNOX_BOARD=pi4 supports serial boot only");
            println!("cargo::rustc-cfg=jnox_pi4");
            "pi4.ld"
        }
        _ => panic!("unknown JNOX_BOARD {board:?}; expected virt or pi4"),
    };
    println!("cargo::rerun-if-changed={script}");
    println!(
        "cargo::rustc-link-arg=-T{}/{script}",
        env!("CARGO_MANIFEST_DIR")
    );
}
