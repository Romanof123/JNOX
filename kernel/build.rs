use std::env;

fn selector(key: &str) -> Option<String> {
    println!("cargo::rerun-if-env-changed={key}");
    match env::var(key) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(error) => panic!("invalid {key}: {error}"),
    }
}

fn main() {
    let profile = selector("JNOX_PROFILE");
    let app = selector("JNOX_APP");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_control)"#);
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_control_dm)"#);
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_control_case, values("handoff-healthy", "handoff-loss", "healthy", "loss", "late", "demand", "worst", "tie", "early-command", "expired-command", "no-command", "missing-permit", "invalid", "silence", "saturation", "recovery", "old-generation"))"#);
    let scenario = selector("JNOX_CONTROL_CASE");
    println!("cargo::rustc-check-cfg=cfg(jnox_control_cold)");
    if let Some(mode) = selector("JNOX_CONTROL_COLD") {
        assert!(mode == "diagnostic" && profile.as_deref() == Some("periodic")
            && scenario.as_deref() == Some("healthy"),
            "JNOX_CONTROL_COLD requires diagnostic, periodic and explicit healthy");
        println!("cargo::rustc-cfg=jnox_control_cold");
    }
    let trace = selector("JNOX_FIXED_TRACE");
    println!("cargo::rustc-check-cfg=cfg(jnox_fixed_trace)");
    if let Some(value) = trace.as_deref() {
        assert!(value == "1", "JNOX_FIXED_TRACE must be unset or 1");
        assert!(
            profile.as_deref() == Some("fixed"),
            "JNOX_FIXED_TRACE requires JNOX_PROFILE=fixed"
        );
        println!("cargo::rustc-cfg=jnox_fixed_trace");
    }
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_fixed)"#);
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("fixed-fifo", "fixed-rr", "fixed-empty", "fixed-delayed", "fixed-ties", "fixed-late", "coop-events", "coop-single"))"#);
    println!("cargo::rerun-if-env-changed=JNOX_BOARD");
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4)");
    // A single selected backend owns the vector table. Cargo sets these
    // variables for enabled features and rebuilds the script for a new set.
    let bench = [
        ("COOP", "coop"),
        ("BATCH", "batch"),
        ("DEADLINE", "deadline"),
        ("APERIODIC", "aperiodic"),
        ("SMP", "smp"),
        ("MUTEX", "mutex"),
        ("MULTILEVEL", "multilevel"),
        ("AMC", "amc"),
        ("PROPORTIONAL", "proportional"),
        ("VRUNTIME", "vruntime"),
        ("EEVDF", "eevdf"),
        ("EEVDF_WAKE", "eevdf_wake"),
        ("HIERARCHY", "hierarchy"),
        ("POWER", "power"),
        ("EXTENSION", "extension"),
        ("PARTITION", "partition"),
        ("CBS", "cbs"),
        ("PERIODIC", "periodic"),
        ("RR", "rr"),
        ("PRIORITY_RESUME", "priority_resume"),
        ("PERIODIC_OVERRUN", "periodic_overrun"),
        ("CBS_WAKE", "cbs_wake"),
        ("SCHED", "scheduler"),
    ]
    .iter()
    .find(|(feature, _)| env::var_os(format!("CARGO_FEATURE_{feature}_BENCH")).is_some())
    .map_or("boot", |(_, profile)| *profile);
    println!(
        r#"cargo::rustc-check-cfg=cfg(jnox_bench, values("boot", "scheduler", "rr", "periodic", "cbs", "partition", "smp", "mutex", "multilevel", "amc", "proportional", "vruntime", "eevdf", "eevdf_wake", "hierarchy", "power", "extension", "aperiodic", "deadline", "batch", "coop", "priority_resume", "periodic_overrun", "cbs_wake"))"#
    );
    println!("cargo::rustc-cfg=jnox_bench=\"{bench}\"");
    let board = match env::var("JNOX_BOARD") {
        Ok(board) => board,
        Err(env::VarError::NotPresent) => "virt".into(),
        Err(error) => panic!("invalid JNOX_BOARD: {error}"),
    };
    match profile.as_deref() {
        Some("fixed") => {
            assert!(bench == "boot", "JNOX_PROFILE=fixed conflicts with bench features");
            if board == "pi4" {
                assert!(
                    matches!(app.as_deref(), Some("fixed-fifo" | "fixed-rr")),
                    "JNOX_BOARD=pi4 fixed profile supports fixed-fifo or fixed-rr only"
                );
                assert!(trace.is_none(), "Pi 4 fixed runtime requires JNOX_FIXED_TRACE unset");
            }
            match app.as_deref() {
                Some(name @ ("fixed-fifo" | "fixed-rr" | "fixed-empty" | "fixed-delayed" | "fixed-ties" | "fixed-late" | "coop-events" | "coop-single")) => {
                    if name.starts_with("coop-") {
                        assert!(trace.is_none(), "ordinary cooperative service requires JNOX_FIXED_TRACE unset");
                    }
                    println!("cargo::rustc-cfg=jnox_fixed");
                    println!("cargo::rustc-cfg=jnox_app=\"{name}\"");
                }
                _ => panic!("JNOX_PROFILE=fixed requires JNOX_APP=fixed-fifo, fixed-rr, fixed-empty, fixed-delayed, fixed-ties, fixed-late, coop-events, or coop-single"),
            }
        }
        Some("periodic") => {
            assert!(board == "virt" && bench == "boot" && trace.is_none());
            assert!(matches!(app.as_deref(), Some("control-rm" | "control-dm")));
            let case = scenario.as_deref().unwrap_or("loss");
            assert!(matches!(case, "handoff-healthy" | "handoff-loss" | "healthy" | "loss" | "late" | "demand" | "worst" | "tie" | "early-command" | "expired-command" | "no-command" | "missing-permit" | "invalid" | "silence" | "saturation" | "recovery" | "old-generation"));
            println!("cargo::rustc-env=JNOX_BUILT_CONTROL_CASE={case}");
            println!("cargo::rustc-cfg=jnox_control");
            println!("cargo::rustc-cfg=jnox_control_case=\"{case}\"");
            if app.as_deref() == Some("control-dm") {
                println!("cargo::rustc-cfg=jnox_control_dm");
            }
        }
        Some(other) => panic!("unknown JNOX_PROFILE {other:?}; expected fixed or periodic"),
        None => assert!(app.is_none(), "JNOX_APP requires JNOX_PROFILE=fixed"),
    }
    let script = match board.as_str() {
        "virt" => "link.ld",
        "pi4" => {
            // EDF and FIFO also select the scheduler backend. Check the Cargo
            // feature set itself to keep the Pi profile strictly fixed-priority.
            assert!(
                env::vars_os()
                    .filter_map(|(key, _)| key.into_string().ok())
                    .filter(|key| key.starts_with("CARGO_FEATURE_"))
                    .all(|key| key == "CARGO_FEATURE_SCHED_BENCH"),
                "JNOX_BOARD=pi4 supports serial boot, sched-bench, or uninstrumented fixed FIFO/RR"
            );
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
