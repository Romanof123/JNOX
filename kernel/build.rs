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
    validate_architecture();
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
    let cold = selector("JNOX_CONTROL_COLD");
    if let Some(mode) = cold.as_deref() {
        assert!(mode == "diagnostic" && profile.as_deref() == Some("periodic")
            && scenario.as_deref() == Some("healthy"),
            "JNOX_CONTROL_COLD requires diagnostic, periodic and explicit healthy");
        println!("cargo::rustc-cfg=jnox_control_cold");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_control_descriptor)");
    let descriptor = selector("JNOX_CONTROL_DESCRIPTOR");
    let markers = selector("JNOX_CONTROL_MARKERS");
    println!("cargo::rustc-check-cfg=cfg(jnox_permit_phase)");
    println!("cargo::rustc-check-cfg=cfg(jnox_permit_anchor)");
    println!("cargo::rustc-check-cfg=cfg(jnox_permit_off)");
    let permit_arm=selector("JNOX_PERMIT_PHASE");
    if let Some(mode)=selector("JNOX_PERMIT_PHASE_MODE") {
        assert!(mode=="characterize-off" && permit_arm.is_some(),"JNOX_PERMIT_PHASE_MODE requires characterize-off and an A1/A2 arm");
        println!("cargo::rustc-cfg=jnox_permit_off");
    }
    if let Some(arm)=permit_arm {
        assert!(descriptor.as_deref()==Some("1") && matches!(arm.as_str(),"nominal"|"anchor"),
            "JNOX_PERMIT_PHASE requires nominal or anchor and JNOX_CONTROL_DESCRIPTOR=1");
        println!("cargo::rustc-cfg=jnox_permit_phase");
        if arm=="anchor" {println!("cargo::rustc-cfg=jnox_permit_anchor")}
    }
    if let Some(value) = descriptor.as_deref() {
        assert!(value == "1" && profile.as_deref() == Some("periodic")
            && matches!(app.as_deref(), Some("control-rm" | "control-dm"))
            && scenario.as_deref() == Some("healthy"),
            "JNOX_CONTROL_DESCRIPTOR=1 requires ordinary periodic healthy RM or DM");
        assert!(cold.is_none() && markers.is_none(),
            "control descriptor, cold and marker comparison arms must be built separately");
        println!("cargo::rustc-cfg=jnox_control_descriptor");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_control_markers)");
    if let Some(value) = markers.as_deref() {
        assert!(descriptor.is_none() && cold.is_none(),
            "control marker, descriptor and cold comparison arms must be built separately");
        assert!(value == "1" && profile.as_deref() == Some("periodic")
            && matches!(app.as_deref(), Some("control-rm" | "control-dm"))
            && scenario.as_deref() == Some("healthy"),
            "JNOX_CONTROL_MARKERS=1 requires ordinary healthy periodic control-rm or control-dm");
        println!("cargo::rustc-cfg=jnox_control_markers");
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
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4_witness)");
    println!("cargo::rustc-check-cfg=cfg(jnox_rr_handoff)");
    println!("cargo::rustc-check-cfg=cfg(jnox_rr_inner)");
    let handoff = selector("JNOX_RR_HANDOFF");
    let witness = selector("JNOX_PI4_WITNESS");
    let image_id = selector("JNOX_PI4_WITNESS_ID");
    if let Some(value) = handoff {
        assert!(matches!(value.as_str(), "1" | "2") && witness.as_deref() == Some("1") && app.as_deref() == Some("fixed-rr")
            && env::var("TARGET").as_deref() == Ok("aarch64-unknown-none")
            && selector("JNOX_ARCH").as_deref().unwrap_or("aarch64") == "aarch64",
            "JNOX_RR_HANDOFF=1 or 2 requires the AArch64 Pi 4 fixed RR witness");
        println!("cargo::rustc-cfg=jnox_rr_handoff");
        if value == "2" { println!("cargo::rustc-cfg=jnox_rr_inner"); }
    }
    if let Some(mode) = witness {
        assert!(mode == "1" && board == "pi4" && bench == "boot"
            && profile.as_deref() == Some("fixed") && trace.is_none()
            && matches!(app.as_deref(), Some("fixed-fifo" | "fixed-rr"))
            && env::vars_os().all(|(k,_)| !k.to_string_lossy().starts_with("CARGO_FEATURE_")),
            "JNOX_PI4_WITNESS=1 requires pi4 fixed FIFO/RR without features, cost or trace");
        let id = image_id.as_deref().and_then(|s| s.parse::<u64>().ok()).filter(|&v| v != 0)
            .expect("provide a nonzero decimal JNOX_PI4_WITNESS_ID for the capture manifest");
        println!("cargo::rustc-env=PI4_WITNESS_ID={id}");
        println!("cargo::rustc-cfg=jnox_pi4_witness");
    } else {
        assert!(image_id.is_none(), "JNOX_PI4_WITNESS_ID requires JNOX_PI4_WITNESS=1");
    }
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
    println!("cargo::rustc-check-cfg=cfg(jnox_ordinary_prop)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_prop_case, values("lottery1","lottery2","stride","stride-delay","lottery-miss"))"#);
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4_smp)");
    println!("cargo::rustc-check-cfg=cfg(jnox_ordinary_mutex)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mutex_ceiling)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mutex_anchor)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mutex_return)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mutex_stages)");
    if let Some(anchor) = selector("JNOX_MUTEX_ANCHOR") {
        assert!(profile.as_deref() == Some("mutex") && board == "virt" && bench == "boot"
            && matches!(app.as_deref(), Some("ordinary-pi" | "ordinary-ceiling"))
            && trace.is_none() && scenario.is_none(),
            "JNOX_MUTEX_ANCHOR requires the ordinary virt mutex PI or ceiling app");
        match anchor.as_str() {
            "rust-prepared" => {},
            "rust-stages" => println!("cargo::rustc-cfg=jnox_mutex_stages"),
            "selected-return" => println!("cargo::rustc-cfg=jnox_mutex_return"),
            _ => panic!("JNOX_MUTEX_ANCHOR requires rust-prepared, rust-stages or selected-return"),
        }
        println!("cargo::rustc-cfg=jnox_mutex_anchor");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_rust_amc)");
    match profile.as_deref() {
        Some("amc") => {
            let name = app.as_deref().unwrap_or("");
            assert!(board == "virt" && bench == "boot" && trace.is_none() && scenario.is_none()
                && !env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CARGO_FEATURE_"))
                && matches!(name, "exact-lo" | "exact-hi" | "over-hi"),
                "ordinary AMC requires virt, exact-lo/exact-hi/over-hi and no benchmark features");
            println!("cargo::rustc-cfg=jnox_rust_amc");
            println!("cargo::rustc-env=JNOX_AMC_APP={name}");
        }
        Some("smp") => {
            assert!(board == "pi4" && app.as_deref() == Some("directed-wake")
                && bench == "boot" && trace.is_none() && scenario.is_none()
                && !env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CARGO_FEATURE_")),
                "directed-wake requires Pi 4, smp profile and no other benchmark or runtime selector");
            println!("cargo::rustc-cfg=jnox_pi4_smp");
        }
        Some("mutex") => {
            assert!(board == "virt" && bench == "boot" && trace.is_none() && scenario.is_none(),
                "ordinary mutex requires virt, no benchmark, trace or control case");
            assert!(matches!(app.as_deref(), Some("ordinary-pi" | "ordinary-ceiling")),
                "ordinary mutex requires ordinary-pi or ordinary-ceiling");
            println!("cargo::rustc-cfg=jnox_ordinary_mutex");
            if app.as_deref() == Some("ordinary-ceiling") {
                println!("cargo::rustc-cfg=jnox_mutex_ceiling");
            }
        }
        Some("proportional") => {
            assert!(scenario.is_none(), "JNOX_CONTROL_CASE must be unset for JNOX_PROFILE=proportional");
            assert!(board == "virt" && bench == "boot", "ordinary proportional requires virt and no benchmark features");
            let case = app.as_deref().unwrap_or("");
            assert!(matches!(case,"lottery1"|"lottery2"|"stride"|"stride-delay"|"lottery-miss"),
                    "ordinary proportional requires an explicit lottery, stride or diagnostic app");
            println!("cargo::rustc-cfg=jnox_ordinary_prop");
            println!("cargo::rustc-cfg=jnox_prop_case=\"{case}\"");
        }
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

fn validate_architecture() {
    const TARGET: &str = include_str!("../arch/aarch64/target");
    if let Some(arch) = selector("JNOX_ARCH") {
        assert!(arch == "aarch64", "unsupported JNOX_ARCH {arch:?}; expected aarch64");
    }
    // Cargo always supplies TARGET. Standalone selector checks can omit it.
    if let Some(target) = selector("TARGET") {
        let expected = TARGET.trim_end();
        assert!(target == expected, "aarch64 requires TARGET={expected}, got {target:?}");
    }
}
