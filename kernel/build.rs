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
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel_limits)");
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel)");
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel_trace)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("mlq", "mlfq", "mlq-blocked", "mlq-exited", "mlq-saturated", "mlfq-delayed"))"#);
    if let Some(value) = selector("JNOX_MULTILEVEL_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("multilevel"),
                "JNOX_MULTILEVEL_TRACE=1 requires multilevel profile");
        println!("cargo::rustc-cfg=jnox_multilevel_trace");
    }
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
    println!("cargo::rustc-check-cfg=cfg(jnox_batch)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_fixed)"#);
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("fixed-fifo", "fixed-rr", "fixed-empty", "fixed-delayed", "fixed-ties", "fixed-late", "coop-events", "coop-single", "batch-sjf", "batch-srtf", "batch-hrrn"))"#);
    println!("cargo::rerun-if-env-changed=JNOX_BOARD");
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4)");
    // A single selected backend owns the vector table. Cargo sets these
    // variables for enabled features and rebuilds the script for a new set.
    let legacy_bench = [
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
    let board = match env::var("JNOX_BOARD") {
        Ok(board) => board,
        Err(env::VarError::NotPresent) => "virt".into(),
        Err(error) => panic!("invalid JNOX_BOARD: {error}"),
    };
    let table_p = env::var_os("CARGO_FEATURE_ORDINARY_PARTITION_P").is_some();
    let table_q = env::var_os("CARGO_FEATURE_ORDINARY_PARTITION_Q").is_some();
    println!("cargo::rustc-check-cfg=cfg(jnox_ordinary_partition)");
    // Ordinary table selectors take effect only when no legacy bench owns the vectors.
    let bench = if legacy_bench == "boot" && (table_p || table_q) {
        assert!(table_p != table_q, "select exactly one ordinary partition table");
        assert!(board == "virt" && profile.is_none() && app.is_none(),
            "ordinary partition requires virt with JNOX_PROFILE and JNOX_APP unset");
        println!("cargo::rustc-cfg=jnox_ordinary_partition");
        "partition"
    } else {
        legacy_bench
    };
    // The winning backend owns its selector validation. A shadowed partition
    // feature cannot select diagnostic code or reject a higher-ranked bench.
    if bench == "partition" && env::var_os("CARGO_FEATURE_PARTITION_LEGACY_DIAG").is_some() {
        let allowed = ["CARGO_FEATURE_PARTITION_BENCH", "CARGO_FEATURE_PARTITION_LEGACY_DIAG",
                       "CARGO_FEATURE_PARTITION_LEGACY_DIAG_NEGATIVE"];
        assert!(board == "virt" && profile.is_none() && app.is_none()
            && env::vars_os().filter_map(|(key, _)| key.into_string().ok())
                .filter(|key| key.starts_with("CARGO_FEATURE_")).all(|key| allowed.contains(&key.as_str())),
            "legacy partition diagnostic requires standalone virt assembly partition fixture");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_partition_mask)");
    println!("cargo::rustc-check-cfg=cfg(jnox_partition_mask_long)");
    if let Some(mode) = selector("JNOX_PARTITION_WORKER_MASK") {
        assert!(legacy_bench == "boot" && bench == "partition" && table_p != table_q
            && selector("JNOX_PARTITION_NOTICE").is_none() && scenario.is_none(),
            "worker mask requires standalone ordinary P or Q without delayed notice");
        assert!(matches!(mode.as_str(), "short" | "long"),
            "JNOX_PARTITION_WORKER_MASK must be short or long");
        println!("cargo::rustc-cfg=jnox_partition_mask");
        if mode == "long" {
            println!("cargo::rustc-cfg=jnox_partition_mask_long");
        }
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_partition_notice)");
    println!("cargo::rustc-check-cfg=cfg(jnox_partition_notice_long)");
    if let Some(mode) = selector("JNOX_PARTITION_NOTICE") {
        assert!(legacy_bench == "boot" && bench == "partition" && table_p != table_q,
            "JNOX_PARTITION_NOTICE requires an effective ordinary partition P or Q on virt");
        assert!(matches!(mode.as_str(), "short" | "long"),
            "JNOX_PARTITION_NOTICE must be short or long");
        println!("cargo::rustc-cfg=jnox_partition_notice");
        if mode == "long" {
            println!("cargo::rustc-cfg=jnox_partition_notice_long");
        }
    }
    println!("cargo::rustc-cfg=jnox_bench=\"{bench}\"");
    println!("cargo::rustc-check-cfg=cfg(jnox_groups)");
    println!("cargo::rustc-check-cfg=cfg(jnox_groups_late)");
    println!("cargo::rustc-check-cfg=cfg(jnox_fixed_cost)");
    if let Some(value) = selector("JNOX_FIXED_COST") {
        assert!(value == "1" && board == "virt" && profile.as_deref() == Some("fixed")
            && matches!(app.as_deref(), Some("fixed-fifo" | "fixed-rr")),
            "JNOX_FIXED_COST requires 1, virt and fixed FIFO/RR");
        println!("cargo::rustc-cfg=jnox_fixed_cost");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_ap)");
    println!("cargo::rustc-check-cfg=cfg(jnox_ap_late)");
    if let Some(value) = selector("JNOX_APERIODIC_LATE") {
        assert!(value == "1" && profile.as_deref() == Some("aperiodic"), "JNOX_APERIODIC_LATE=1 requires aperiodic profile");
        println!("cargo::rustc-cfg=jnox_ap_late");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_ordinary_deadline)");
    println!("cargo::rustc-check-cfg=cfg(jnox_deadline_trace)");
    if let Some(value) = selector("JNOX_DEADLINE_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("deadline"), "JNOX_DEADLINE_TRACE=1 requires ordinary deadline profile");
        println!("cargo::rustc-cfg=jnox_deadline_trace");
    }
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("ordinary-edf", "ordinary-llf", "ordinary-llf-overload", "ordinary-edf-overrun"))"#);
    println!("cargo::rustc-check-cfg=cfg(jnox_weighted)");
    println!("cargo::rustc-check-cfg=cfg(jnox_weighted_wake)");
    println!("cargo::rustc-check-cfg=cfg(jnox_weighted_trace)");
    if let Some(value) = selector("JNOX_WEIGHTED_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("weighted"));
        println!("cargo::rustc-cfg=jnox_weighted_trace");
    }
    match profile.as_deref() {
        Some("aperiodic") => {
            assert!(board == "virt" && bench == "boot", "ordinary aperiodic requires virt without benchmarks");
            let name = app.as_deref().expect("aperiodic profile requires JNOX_APP");
            assert!(matches!(name, "a-ps" | "a-ds" | "a-ss1" | "a-ss2" | "b-ps" | "b-ds" | "b-ss1" | "b-ss2"), "invalid aperiodic application");
            println!("cargo::rustc-cfg=jnox_ap");
            println!("cargo::rustc-env=JNOX_AP_APP={name}");
        }
        Some("deadline") => {
            assert!(bench == "boot" && board == "virt", "ordinary deadline requires virt without benchmark features");
            match app.as_deref() {
                Some(name @ ("ordinary-edf" | "ordinary-llf" | "ordinary-llf-overload" | "ordinary-edf-overrun")) => {
                    println!("cargo::rustc-cfg=jnox_ordinary_deadline");
                    println!("cargo::rustc-cfg=jnox_app=\"{name}\"");
                }
                _ => panic!("deadline requires an ordinary EDF/LLF application"),
            }
        }
        Some("weighted") => {
            assert!(board == "virt" && bench == "boot" && scenario.is_none());
            assert!(matches!(app.as_deref(), Some("weighted-share" | "weighted-wake")));
            println!("cargo::rustc-cfg=jnox_weighted");
            if app.as_deref() == Some("weighted-wake") {
                println!("cargo::rustc-cfg=jnox_weighted_wake");
            }
        }
        Some("groups") => {
            assert!(board == "virt" && bench == "boot" && matches!(app.as_deref(), Some("ordinary-groups" | "ordinary-groups-late")),
                    "groups requires virt, ordinary-groups or ordinary-groups-late, and no benchmark");
            println!("cargo::rustc-cfg=jnox_groups");
            if app.as_deref() == Some("ordinary-groups-late") { println!("cargo::rustc-cfg=jnox_groups_late"); }
        }
        Some("multilevel") => {
            assert!(bench == "boot" && board == "virt", "multilevel requires virt without bench features");
            match app.as_deref() {
                Some(name @ ("mlq" | "mlfq" | "mlq-blocked" | "mlq-exited" | "mlq-saturated" | "mlfq-delayed")) => {
                    if name.contains('-') { println!("cargo::rustc-cfg=jnox_multilevel_limits"); }
                    println!("cargo::rustc-cfg=jnox_multilevel");
                    println!("cargo::rustc-cfg=jnox_app=\"{name}\"");
                }
                _ => panic!("multilevel requires a supported MLQ or MLFQ application"),
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
        Some(other) => panic!("unknown JNOX_PROFILE {other:?}; expected fixed, batch, groups, weighted, multilevel, periodic, aperiodic or deadline"),
        None => assert!(app.is_none(), "JNOX_APP requires JNOX_PROFILE=fixed, batch, groups, weighted, multilevel, periodic, aperiodic or deadline"),
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
                    .all(|key| matches!(key.as_str(), "CARGO_FEATURE_SCHED_BENCH"
                        | "CARGO_FEATURE_ORDINARY_PARTITION_P" | "CARGO_FEATURE_ORDINARY_PARTITION_Q")),
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
