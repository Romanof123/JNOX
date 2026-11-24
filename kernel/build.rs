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
    println!("cargo::rustc-check-cfg=cfg(jnox_batch_trace)");
    if let Some(value) = selector("JNOX_BATCH_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("batch"), "JNOX_BATCH_TRACE=1 requires batch profile");
        println!("cargo::rustc-cfg=jnox_batch_trace");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_group_trace)");
    if let Some(value) = selector("JNOX_GROUP_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("groups"),
                "JNOX_GROUP_TRACE requires 1 and JNOX_PROFILE=groups");
        println!("cargo::rustc-cfg=jnox_group_trace");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel)");
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel_trace)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("mlq", "mlfq"))"#);
    if let Some(value) = selector("JNOX_MULTILEVEL_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("multilevel"),
                "JNOX_MULTILEVEL_TRACE=1 requires multilevel profile");
        println!("cargo::rustc-cfg=jnox_multilevel_trace");
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
    println!("cargo::rustc-check-cfg=cfg(jnox_batch)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_fixed)"#);
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("fixed-fifo", "fixed-rr", "fixed-empty", "fixed-delayed", "fixed-ties", "fixed-late", "coop-events", "coop-single", "batch-sjf", "batch-srtf", "batch-hrrn"))"#);
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
        ("EEVDF_DYNAMIC", "eevdf_dynamic"),
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
        r#"cargo::rustc-check-cfg=cfg(jnox_bench, values("boot", "scheduler", "rr", "periodic", "cbs", "partition", "smp", "mutex", "multilevel", "amc", "proportional", "vruntime", "eevdf", "eevdf_wake", "eevdf_dynamic", "hierarchy", "power", "extension", "aperiodic", "deadline", "batch", "coop", "priority_resume", "periodic_overrun", "cbs_wake"))"#
    );
    println!("cargo::rustc-cfg=jnox_bench=\"{bench}\"");
    let board = match env::var("JNOX_BOARD") {
        Ok(board) => board,
        Err(env::VarError::NotPresent) => "virt".into(),
        Err(error) => panic!("invalid JNOX_BOARD: {error}"),
    };
    println!("cargo::rustc-check-cfg=cfg(jnox_groups)");
    println!("cargo::rustc-check-cfg=cfg(jnox_groups_late)");
    match profile.as_deref() {
        Some("groups") => {
            assert!(board == "virt" && bench == "boot" && matches!(app.as_deref(), Some("ordinary-groups" | "ordinary-groups-late")),
                    "groups requires virt, ordinary-groups or ordinary-groups-late, and no benchmark");
            println!("cargo::rustc-cfg=jnox_groups");
            if app.as_deref() == Some("ordinary-groups-late") { println!("cargo::rustc-cfg=jnox_groups_late"); }
        }
        Some("multilevel") => {
            assert!(bench == "boot" && board == "virt", "multilevel requires virt without bench features");
            match app.as_deref() {
                Some(name @ ("mlq" | "mlfq")) => {
                    println!("cargo::rustc-cfg=jnox_multilevel");
                    println!("cargo::rustc-cfg=jnox_app=\"{name}\"");
                }
                _ => panic!("multilevel requires JNOX_APP=mlq or mlfq"),
            }
        }
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
        Some("batch") => {
            assert!(board == "virt" && bench == "boot", "ordinary batch requires virt and no bench features");
            match app.as_deref() {
                Some(name @ ("batch-sjf" | "batch-srtf" | "batch-hrrn")) => {
                    println!("cargo::rustc-cfg=jnox_batch");
                    println!("cargo::rustc-cfg=jnox_app=\"{name}\"");
                }
                _ => panic!("batch profile requires batch-sjf, batch-srtf or batch-hrrn"),
            }
        }
        Some(other) => panic!("unknown JNOX_PROFILE {other:?}; expected fixed, batch, groups or multilevel"),
        None => assert!(app.is_none(), "JNOX_APP requires JNOX_PROFILE=fixed, batch, groups or multilevel"),
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
