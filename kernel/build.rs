use std::env;

fn main() {
    println!("cargo::rerun-if-env-changed=JNOX_BOARD");
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4)");
    let board = match env::var("JNOX_BOARD") {
        Ok(board) => board,
        Err(env::VarError::NotPresent) => "virt".into(),
        Err(error) => panic!("invalid JNOX_BOARD: {error}"),
    };
    let script = match board.as_str() {
        "virt" => "link.ld",
        "pi4" => {
            for feature in ["SCHED_BENCH", "CBS_BENCH", "PARTITION_BENCH"] {
                assert!(
                    env::var_os(format!("CARGO_FEATURE_{feature}")).is_none(),
                    "JNOX_BOARD=pi4 supports serial boot only"
                );
            }
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
