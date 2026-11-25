#[allow(dead_code, unexpected_cfgs)]
#[path = "src/feedback_policy.rs"]
mod feedback_policy;
#[allow(dead_code, unexpected_cfgs)]
#[path = "src/proportional_policy.rs"]
mod proportional_policy;

mod build_native_extension;
use std::env;
mod build_native_hierarchy;

fn selector(key: &str) -> Option<String> {
    println!("cargo::rerun-if-env-changed={key}");
    match env::var(key) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(error) => panic!("invalid {key}: {error}"),
    }
}

fn verify_ap_report(directory: &std::path::Path, digest: &str) {
    // build.rs is compiled for the host, independently of the kernel target.
    #[cfg(windows)]
    let (tool, args): (&str, &[&str]) = ("certutil.exe", &["-hashfile", "ap-report.txt", "SHA256"]);
    #[cfg(not(windows))]
    let (tool, args): (&str, &[&str]) = ("sha256sum", &["ap-report.txt"]);
    let result = std::process::Command::new(tool).args(args).current_dir(directory)
        .output().unwrap_or_else(|error| panic!("run {tool}: {error}"));
    let stdout = String::from_utf8_lossy(&result.stdout);
    #[cfg(windows)]
    let matches = stdout.lines().any(|line| line.trim().eq_ignore_ascii_case(digest));
    #[cfg(not(windows))]
    let matches = stdout.split_ascii_whitespace().next() == Some(digest);
    assert!(result.status.success() && matches, "aperiodic report digest mismatch");
}

// Bind the exact embedded report bytes to the integrator's independently approved pin.
fn configure_ap_admission(profile: Option<&str>) {
    println!("cargo::rustc-check-cfg=cfg(jnox_ap_admission)");
    let mode = selector("JNOX_AP_MODE");
    let report = selector("JNOX_AP_REPORT");
    let pin = selector("JNOX_AP_REPORT_SHA256");
    if mode.as_deref() != Some("admission") {
        assert!(mode.is_none() || (mode.as_deref() == Some("engineering") && profile == Some("aperiodic")),
                "invalid JNOX_AP_MODE");
        assert!(report.is_none() && pin.is_none(), "aperiodic cost inputs require admission mode");
        return;
    }
    assert!(profile == Some("aperiodic") && env::var_os("JNOX_APERIODIC_LATE").is_none(),
            "admission requires a normal aperiodic workload");
    let directory = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let output = directory.join("ap-report.txt");
    match (report, pin) {
        (None, None) => std::fs::write(&output, b"").unwrap(),
        (Some(path), Some(digest)) => {
            assert!(digest.len() == 64 && digest.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')),
                    "invalid JNOX_AP_REPORT_SHA256");
            assert!(!path.is_empty() && !path.contains(['\r', '\n']), "invalid JNOX_AP_REPORT");
            let source = std::fs::canonicalize(path).expect("resolve aperiodic report");
            println!("cargo::rerun-if-changed={}", source.display());
            std::fs::copy(source, &output).expect("copy approved aperiodic report");
            verify_ap_report(&directory, &digest);
            println!("cargo::rustc-env=JNOX_AP_APPROVED_SHA256={digest}");
        }
        _ => panic!("supply both aperiodic report and independently approved digest"),
    }
    println!("cargo::rustc-cfg=jnox_ap_admission");
}

// Cargo exposes the expanded set, not the --all-features flag. Read the
// package's own feature keys so a partial set cannot impersonate that legacy
// invocation when new features are added.
fn all_package_features_enabled() -> bool {
    let manifest = include_str!("Cargo.toml");
    let (_, features) = manifest.split_once("[features]").expect("package feature table");
    features.lines().map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .all(|name| {
            let key = name.replace('-', "_").to_ascii_uppercase();
            env::var_os(format!("CARGO_FEATURE_{key}")).is_some()
        })
}

fn main() {
    validate_architecture();
    let profile = selector("JNOX_PROFILE");
    let app = selector("JNOX_APP");
    configure_ap_admission(profile.as_deref());
    println!("cargo::rustc-check-cfg=cfg(jnox_batch_telemetry)");
    if let Some(workload) = selector("JNOX_BATCH_WORKLOAD") {
        assert!(workload == "telemetry" && profile.as_deref() == Some("batch"),
                "JNOX_BATCH_WORKLOAD=telemetry requires the batch profile");
        println!("cargo::rustc-cfg=jnox_batch_telemetry");
    }
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
    println!("cargo::rustc-check-cfg=cfg(jnox_mlfq_wake)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mlq_admission)");
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel_limits)");
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel)");
    println!("cargo::rustc-check-cfg=cfg(jnox_multilevel_trace)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("mlq", "mlfq", "mlq-blocked", "mlq-exited", "mlq-saturated", "mlfq-delayed", "mlq-admit", "mlq-refuse", "mlfq-wake", "mlfq-wake-refuse"))"#);
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
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("fixed-fifo", "fixed-rr", "fixed-empty", "fixed-delayed", "fixed-ties", "fixed-late", "coop-events", "coop-single", "coop-bounded", "batch-sjf", "batch-srtf", "batch-hrrn"))"#);
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
    println!("cargo::rustc-check-cfg=cfg(jnox_sched_app_trace)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_sched_app_trace_mode, values("disabled", "active", "saturate"))"#);
    if let Some(mode) = selector("JNOX_SCHED_APP_TRACE") {
        assert!(matches!(mode.as_str(), "disabled" | "active" | "saturate"),
                "invalid JNOX_SCHED_APP_TRACE");
        assert!(board == "virt" && legacy_bench == "boot" && profile.as_deref() == Some("multilevel")
                && app.as_deref() == Some("mlfq-wake"),
                "JNOX_SCHED_APP_TRACE requires ordinary virt/multilevel/mlfq-wake");
        println!("cargo::rustc-cfg=jnox_sched_app_trace");
        println!(r#"cargo::rustc-cfg=jnox_sched_app_trace_mode="{mode}""#);
    }
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
    println!("cargo::rustc-check-cfg=cfg(jnox_groups_recurring)");
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
    println!("cargo::rustc-check-cfg=cfg(jnox_deadline_owner_cap)");
    println!("cargo::rustc-check-cfg=cfg(jnox_deadline_recurring)");
    let recurring = matches!(app.as_deref(), Some("ordinary-edf-recurring" | "ordinary-edf-recurring-refuse" | "ordinary-llf-recurring" | "ordinary-llf-recurring-refuse"));
    if recurring {
        println!("cargo::rustc-cfg=jnox_deadline_recurring");
    }
    let owner_cap = selector("JNOX_DEADLINE_OWNER_CAP");
    if let Some(value) = owner_cap.as_deref().or(if recurring { Some("4") } else { None }) {
        assert!(profile.as_deref() == Some("deadline")
            && (recurring || matches!(app.as_deref(), Some("ordinary-edf-supply" | "ordinary-llf-supply"))),
            "JNOX_DEADLINE_OWNER_CAP requires an ordinary finite supply profile");
        assert!(!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
            "owner cap must be an unsigned decimal integer");
        let ceiling: u64 = value.parse().expect("owner cap exceeds u64");
        println!("cargo::rustc-env=JNOX_OWNER_CEILING={ceiling}");
        println!("cargo::rustc-cfg=jnox_deadline_owner_cap");
    }
    if let Some(value) = selector("JNOX_DEADLINE_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("deadline"), "JNOX_DEADLINE_TRACE=1 requires ordinary deadline profile");
        println!("cargo::rustc-cfg=jnox_deadline_trace");
    }
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_app, values("ordinary-edf", "ordinary-llf", "ordinary-llf-overload", "ordinary-edf-overrun", "ordinary-edf-supply", "ordinary-llf-supply", "ordinary-edf-supply-refuse", "ordinary-llf-supply-refuse", "ordinary-edf-recurring", "ordinary-edf-recurring-refuse", "ordinary-llf-recurring", "ordinary-llf-recurring-refuse"))"#);
    println!("cargo::rustc-check-cfg=cfg(jnox_weighted)");
    println!("cargo::rustc-check-cfg=cfg(jnox_weighted_wake)");
    println!("cargo::rustc-check-cfg=cfg(jnox_weighted_trace)");
    if let Some(value) = selector("JNOX_WEIGHTED_TRACE") {
        assert!(value == "1" && profile.as_deref() == Some("weighted"));
        println!("cargo::rustc-cfg=jnox_weighted_trace");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_ordinary_prop)");
    println!(r#"cargo::rustc-check-cfg=cfg(jnox_prop_case, values("lottery1","lottery2","stride","stride-delay","lottery-miss","lottery-window1","lottery-window2","lottery-window-tight"))"#);
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4_smp)");
    println!("cargo::rustc-check-cfg=cfg(jnox_ordinary_mutex)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mutex_fifo)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mutex_survivors)");
    println!("cargo::rustc-check-cfg=cfg(jnox_mutex_chain)");
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
    println!("cargo::rustc-check-cfg=cfg(jnox_pi4_migration)");
    println!("cargo::rustc-check-cfg=cfg(jnox_eevdf_app)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_el0)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_process)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_peer)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_peer2)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_fixed)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_weighted)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_edf)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_periodic)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_cbs)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_multilevel)");
    let peer2_image = selector("JNOX_NATIVE_PEER2_ELF");
    let peer2_hash = selector("JNOX_NATIVE_PEER2_ELF_SHA256");
    let policy = selector("JNOX_NATIVE_POLICY");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_hierarchy)");
    println!("cargo::rerun-if-changed=build_native_hierarchy.rs");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_extension)");
    let extension = selector("JNOX_NATIVE_EXTENSION_PROGRAM");
    let extension_hash = selector("JNOX_NATIVE_EXTENSION_SHA256");
    assert_eq!(extension.is_some(), policy.as_deref() == Some("extension"), "extension policy requires a program");
    assert_eq!(extension_hash.is_some(), extension.is_some(), "extension requires a canonical digest");
    let hierarchy = selector("JNOX_NATIVE_HIERARCHY");
    assert_eq!(hierarchy.is_some(), policy.as_deref() == Some("hierarchy"),
        "hierarchy policy requires JNOX_NATIVE_HIERARCHY exclusively");
    let priorities = selector("JNOX_NATIVE_PRIORITIES");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_weighted_continuous)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_weighted_notify)");
    let continuous = selector("JNOX_NATIVE_WEIGHTED_CONTINUOUS");
    let notify = selector("JNOX_NATIVE_WEIGHTED_NOTIFY");
    if let Some(value) = continuous.as_deref() {
        assert!(value == "1" && board == "virt" && policy.as_deref() == Some("weighted"),
            "continuous leases require native weighted scheduling on virt");
        println!("cargo::rustc-cfg=jnox_native_weighted_continuous");
    }
    let notification = if let Some(raw) = notify.as_deref() {
        assert!(continuous.is_some(), "weighted notification requires continuous leases");
        let (mode, count) = raw.split_once(':').expect("expected closures:N or wakes:N");
        assert!(matches!(mode, "closures" | "wakes") && !count.is_empty()
            && count.bytes().all(|b| b.is_ascii_digit()), "invalid weighted notification");
        let count: u64 = count.parse().expect("notification count exceeds u64");
        assert!(count > 0, "notification count must be positive");
        println!("cargo::rustc-cfg=jnox_native_weighted_notify");
        (mode, count)
    } else {
        ("none", 0)
    };
    println!("cargo::rustc-check-cfg=cfg(jnox_native_proportional)");
    let seed = selector("JNOX_NATIVE_LOTTERY_SEED");
    let lottery_window = selector("JNOX_NATIVE_LOTTERY_WINDOW");
    let lottery_floors = selector("JNOX_NATIVE_LOTTERY_FLOORS");
    assert_eq!(seed.is_some(), policy.as_deref() == Some("lottery"), "only lottery requires a seed");
    assert_eq!(lottery_window.is_some(), lottery_floors.is_some(), "supply window and floors together");
    assert!(lottery_window.is_none() || seed.is_some(), "window requires lottery");
    let lottery = [seed.as_deref(), lottery_window.as_deref(), lottery_floors.as_deref()];
    let weights = selector("JNOX_NATIVE_WEIGHTS");
    let periodic = selector("JNOX_NATIVE_PERIODIC");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_partition)");
    let partition = selector("JNOX_NATIVE_PARTITION_WINDOWS");
    assert_eq!(partition.is_some(), policy.as_deref() == Some("partition"),
        "partition policy requires JNOX_NATIVE_PARTITION_WINDOWS exclusively");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_aperiodic)");
    let slot_ms = selector("JNOX_NATIVE_SERVER_SLOT_MS");
    let budget = selector("JNOX_NATIVE_SERVER_BUDGET");
    let credits = selector("JNOX_NATIVE_SERVER_CREDITS");
    let server = matches!(policy.as_deref(), Some("polling" | "deferrable" | "sporadic"));
    assert_eq!(slot_ms.is_some(), server, "server policies require slot milliseconds exclusively");
    assert_eq!(budget.is_some(), server, "server policies require quota and period exclusively");
    assert_eq!(credits.is_some(), policy.as_deref() == Some("sporadic"), "only sporadic requires credits");
    let server_settings = server.then(|| {
        let ms = decimal_number(slot_ms.as_deref().unwrap());
        let (q, p) = parse_budget_tuple(budget.as_deref().unwrap());
        assert!(ms > 0 && ms.checked_mul(u64::from(p)).is_some(), "invalid server period in milliseconds");
        let cap = credits.as_deref().map(|s| {
            let cap = decimal_number(s);
            assert!((1..=8).contains(&cap), "sporadic credits must be in 1..8");
            cap
        }).unwrap_or(0);
        (ms, q, p, cap)
    });
    let classes = selector("JNOX_NATIVE_CLASSES");
    let allotments = selector("JNOX_NATIVE_ALLOTMENT_TICKS");
    let boost = selector("JNOX_NATIVE_BOOST_TICKS");
    let multilevel = [classes.as_deref(), allotments.as_deref(), boost.as_deref()];
    let reservations = selector("JNOX_NATIVE_RESERVATIONS");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_batch)");
    let batch = selector("JNOX_NATIVE_BATCH_JOBS");
    assert_eq!(batch.is_some(), matches!(policy.as_deref(), Some("sjf" | "srtf" | "hrrn")),
        "native batch policies require JNOX_NATIVE_BATCH_JOBS exclusively");
    let edf_jobs = selector("JNOX_NATIVE_EDF_JOBS");
    let quantum = selector("JNOX_NATIVE_QUANTUM_TICKS");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_amc)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_peer3)");
    let amc = selector("JNOX_NATIVE_AMC_MANIFEST");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_amc_clock_page)");
    if let Some(setting) = selector("JNOX_AMC_CLOCK_PAGE") {
        assert!(setting == "1" && amc.is_some() && policy.as_deref() == Some("amc"),
                "JNOX_AMC_CLOCK_PAGE requires value 1 and native AMC");
        println!("cargo::rustc-cfg=jnox_native_amc_clock_page");
    }
    let amc_sha = selector("JNOX_NATIVE_AMC_SHA256");
    assert_eq!(amc.is_some(), policy.as_deref() == Some("amc"), "AMC requires a canonical manifest exclusively");
    assert_eq!(amc.is_some(), amc_sha.is_some(), "AMC requires a canonical digest");
    let peer3_image = selector("JNOX_NATIVE_PEER3_ELF");
    let peer3_hash = selector("JNOX_NATIVE_PEER3_ELF_SHA256");
    assert_eq!(peer3_image.is_some(), peer3_hash.is_some(), "third peer requires digest");
    assert!(peer3_image.is_none() || (amc.is_some() && peer2_image.is_some()), "only AMC supports a third peer");
    assert!(selector("JNOX_NATIVE_PEER4_ELF").is_none() && selector("JNOX_NATIVE_PEER4_ELF_SHA256").is_none(),
        "at most four native slots");
    let peer_image = selector("JNOX_NATIVE_PEER_ELF");
    let peer_hash = selector("JNOX_NATIVE_PEER_ELF_SHA256");
    assert_eq!(peer_image.is_some(), peer_hash.is_some(), "peer ELF requires its digest");
    assert_eq!(peer2_image.is_some(), peer2_hash.is_some(), "second peer requires its digest");
    assert!(peer2_image.is_none() || peer_image.is_some(), "missing first peer");
    if multilevel.iter().any(Option::is_some) || peer_image.is_some() || policy.is_some() || priorities.is_some() || weights.is_some() || edf_jobs.is_some() || periodic.is_some() || reservations.is_some() || quantum.is_some() {
        assert!(profile.as_deref() == Some("native") && app.as_deref() == Some("process"),
            "native peers and scheduling require ordinary native process packaging");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_native_threads)");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_thread_rr)");
    let threads = selector("JNOX_NATIVE_THREADS");
    let thread_rr = threads.is_some() && policy.as_deref() == Some("rr");
    println!("cargo::rustc-check-cfg=cfg(jnox_native_thread_fixed)");
    let thread_roles = selector("JNOX_NATIVE_THREAD_PRIORITIES");
    assert_eq!(thread_roles.is_some(), threads.as_deref() == Some("4"),
        "four thread slots require JNOX_NATIVE_THREAD_PRIORITIES exclusively");
    let thread_priorities = thread_roles.map(|roles| {
        let values: Vec<u8> = roles.split(',').map(|field| {
            assert!(!field.is_empty() && field.bytes().all(|b| b.is_ascii_digit()), "invalid thread priority");
            let priority = field.parse::<u8>().expect("thread priority exceeds u8");
            assert!((1..=31).contains(&priority), "thread priority outside 1..31");
            priority
        }).collect();
        <[u8; 4]>::try_from(values).expect("expected four thread priorities")
    });
    if let Some(capacity) = threads.as_deref() {
        assert!(board == "virt" && profile.as_deref() == Some("native")
            && app.as_deref() == Some("process") && peer_image.is_none(),
            "native threads require one ordinary virt process");
        match capacity {
            "2" => assert!(policy.is_none() || thread_rr, "two threads require cooperative or RR scheduling"),
            "4" => {
                assert!(matches!(policy.as_deref(), Some("fifo" | "rr")), "four threads require FIFO or RR");
                println!("cargo::rustc-cfg=jnox_native_thread_fixed");
            }
            _ => panic!("unsupported thread capacity"),
        }
        println!("cargo::rustc-cfg=jnox_native_threads");
    }
    let service_image = selector("JNOX_NATIVE_DEADLINE_SERVICE_ELF");
    let service_hash = selector("JNOX_NATIVE_DEADLINE_SERVICE_ELF_SHA256");
    let service_cells = selector("JNOX_NATIVE_DEADLINE_SERVICE_CELLS");
    let switch_cap = selector("JNOX_NATIVE_DEADLINE_SWITCH_CAP");
    let mut service_mask = 0u16;
    let mut cap = 0u64;
    if service_image.is_some() || service_hash.is_some() || service_cells.is_some() || switch_cap.is_some() {
        assert!(profile.as_deref() == Some("native") && app.as_deref() == Some("process")
            && matches!(policy.as_deref(), Some("edf" | "llf")) && peer_image.is_some() && peer2_image.is_none()
            && service_image.is_some() && service_hash.is_some(), "deadline service requires two ordinary deadline jobs and its own ELF/hash");
        let mut previous = None;
        for field in service_cells.as_deref().expect("missing deadline service calendar").split(',') {
            assert!(!field.is_empty() && field.bytes().all(|c| c.is_ascii_digit()), "invalid deadline service cell");
            let cell: u8 = field.parse().expect("invalid service cell");
            assert!(cell < 16 && previous.is_none_or(|v| v < cell), "service cells must increase within 0..15");
            previous = Some(cell);
            service_mask |= 1u16 << cell;
        }
        let value = switch_cap.as_deref().expect("missing deadline switch cap");
        assert!(!value.is_empty() && value.bytes().all(|c| c.is_ascii_digit()), "invalid deadline switch cap");
        cap = value.parse().expect("deadline switch cap exceeds u64");
        assert!(cap > 0, "deadline switch cap must be positive");
    }
    let native_argv = [0, 1, 2, 3].map(|slot| selector(&format!("JNOX_NATIVE_ARGV{slot}_HEX")));
    assert!(amc.is_none() || native_argv.iter().all(Option::is_none), "declare AMC arguments in its manifest");
    for (slot, args) in native_argv.iter().enumerate() {
        if args.is_some() {
            assert!(profile.as_deref() == Some("native") && app.as_deref() == Some("process"),
                "slot {slot}: arguments require an ordinary native application");
            assert!(slot == 0 || (slot == 1 && peer_image.is_some()) || (slot == 2 && peer2_image.is_some()),
                "slot {slot}: arguments supplied without an ELF");
        }
    }
    let native_image = selector("JNOX_NATIVE_ELF");
    let native_hash = selector("JNOX_NATIVE_ELF_SHA256");
    if profile.as_deref() != Some("native") {
        assert!(native_image.is_none() && native_hash.is_none(),
            "native ELF packaging variables require JNOX_PROFILE=native");
    }
    let migration_app = matches!(app.as_deref(),
        Some("migration-global" | "migration-partitioned" | "migration-clustered"));
    let stream_feature = env::var_os("CARGO_FEATURE_PI4_STREAM_V1").is_some();
    let migration_stream = stream_feature && board == "pi4" && profile.as_deref() == Some("smp")
        && migration_app;
    if stream_feature && !migration_stream {
        assert!(board == "virt" && bench == "coop" && profile.is_none() && app.is_none()
            && all_package_features_enabled(),
            "pi4-stream-v1 requires a Pi 4 SMP migration application");
    }
    println!("cargo::rustc-check-cfg=cfg(jnox_native_smp_partitioned)");
    let smp_masks = selector("JNOX_NATIVE_CPU_MASKS");
    let smp_cells = selector("JNOX_NATIVE_RELEASE_CELLS");
    let pi4_native_smp = policy.as_deref() == Some("smp-partitioned");
    let smp_table = if pi4_native_smp {
        assert!(board == "pi4" && profile.as_deref() == Some("native") &&
                app.as_deref() == Some("process") && peer_image.is_some() && threads.is_none(),
                "smp-partitioned requires two or three Pi 4 native processes");
        let slots = 2 + usize::from(peer2_image.is_some());
        let masks = native_smp_bytes(smp_masks.as_deref().expect("missing CPU masks"), slots);
        assert!(masks.iter().all(|m| matches!(m, 1 | 2)) && masks.contains(&1) && masks.contains(&2),
                "singleton CPU masks must cover both cores");
        let cells = smp_cells.as_deref().map(|v| native_smp_bytes(v, slots)).unwrap_or_else(|| vec![0; slots]);
        Some((masks, cells))
    } else {
        assert!(smp_masks.is_none() && smp_cells.is_none(), "CPU mask and release table require smp-partitioned");
        None
    };
    let pi4_native_fixed = board == "pi4" && profile.as_deref() == Some("native")
        && app.as_deref() == Some("process") && peer_image.is_some() && peer2_image.is_none()
        && matches!(policy.as_deref(), Some("fifo" | "rr"));
    match profile.as_deref() {
        Some("native") => {
            assert!((board == "virt" || pi4_native_fixed || pi4_native_smp) && matches!(app.as_deref(), Some("startup-probe" | "process"))
                && bench == "boot" && trace.is_none() && scenario.is_none()
                && !env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CARGO_FEATURE_")),
                "native requires virt or two Pi fixed processes and no benchmark features");
            if app.as_deref() == Some("process") {
                println!("cargo::rustc-cfg=jnox_native_process");
            }
            let peer2_image = peer2_image.or(service_image);
            let peer2_hash = peer2_hash.or(service_hash);
            let slots = 1 + usize::from(peer_image.is_some()) + usize::from(peer2_image.is_some()) + usize::from(peer3_image.is_some());
            if let (Some(program), Some(hash)) = (extension, extension_hash) {
                assert_eq!(slots, 3, "extension requires three ordinary applications");
                build_native_extension::configure(&program, &hash);
            }
            write_native_schedule(slots, policy.as_deref(), priorities.as_deref(), weights.as_deref(), quantum.as_deref(), edf_jobs.as_deref(), periodic.as_deref(), reservations.as_deref(), multilevel, &native_argv, thread_rr, service_mask, cap, partition.as_deref(), server_settings, notification, hierarchy.as_deref(), thread_priorities, lottery, batch.as_deref(), amc.as_deref().zip(amc_sha.as_deref()), smp_table);
            package_native_image(native_image.as_deref().expect("missing JNOX_NATIVE_ELF"),
                native_hash.as_deref().expect("missing JNOX_NATIVE_ELF_SHA256"),
                "native_user.elf", "JNOX_PACKAGED_ELF_SHA256");
            if let (Some(source), Some(hash)) = (peer_image.as_deref(), peer_hash.as_deref()) {
                package_native_image(source, hash, "native_peer.elf", "JNOX_PACKAGED_PEER_SHA256");
                println!("cargo::rustc-cfg=jnox_native_peer");
            }
            if let (Some(source), Some(hash)) = (peer2_image.as_deref(), peer2_hash.as_deref()) {
                package_native_image(source, hash, "native_peer2.elf", "JNOX_PACKAGED_PEER2_SHA256");
                println!("cargo::rustc-cfg=jnox_native_peer2");
            }
            if let (Some(source), Some(hash)) = (peer3_image.as_deref(), peer3_hash.as_deref()) {
                package_native_image(source, hash, "native_peer3.elf", "JNOX_PACKAGED_PEER3_SHA256");
                println!("cargo::rustc-cfg=jnox_native_peer3");
            }
            println!("cargo::rustc-cfg=jnox_native_el0");
        }
        Some("eevdf") => {
            assert!(board == "virt" && bench == "boot" && app.as_deref() == Some("telemetry")
                && trace.is_none() && scenario.is_none()
                && !env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CARGO_FEATURE_")),
                "ordinary EEVDF requires virt, telemetry and no benchmark features");
            println!("cargo::rustc-cfg=jnox_eevdf_app");
        }
        Some("amc") => {
            let name = app.as_deref().unwrap_or("");
            assert!(board == "virt" && bench == "boot" && trace.is_none() && scenario.is_none()
                && !env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CARGO_FEATURE_"))
                && matches!(name, "exact-lo" | "exact-hi" | "over-hi" | "stagger-idle"),
                "ordinary AMC requires virt, exact-lo/exact-hi/over-hi/stagger-idle and no benchmark features");
            println!("cargo::rustc-cfg=jnox_rust_amc");
            println!("cargo::rustc-env=JNOX_AMC_APP={name}");
        }
        Some("smp") => {
            let selected = app.as_deref().unwrap_or("");
            assert!(board == "pi4" && (selected == "directed-wake" || migration_app)
                && bench == "boot" && trace.is_none() && scenario.is_none()
                && env::vars_os().all(|(key, _)| !key.to_string_lossy().starts_with("CARGO_FEATURE_")
                    || (migration_stream && key == "CARGO_FEATURE_PI4_STREAM_V1")),
                "directed-wake requires Pi 4, smp profile and no other benchmark or runtime selector");
            println!("cargo::rustc-cfg=jnox_pi4_smp");
            if selected != "directed-wake" {
                println!("cargo::rustc-cfg=jnox_pi4_migration");
                println!("cargo::rustc-env=JNOX_MIGRATION_APP={selected}");
            }
        }
        Some("mutex") => {
            assert!(board == "virt" && bench == "boot" && trace.is_none() && scenario.is_none(),
                "ordinary mutex requires virt, no benchmark, trace or control case");
            assert!(matches!(app.as_deref(), Some("ordinary-pi" | "ordinary-ceiling" | "ordinary-fifo" | "ordinary-chain" | "ordinary-survivors")),
                "ordinary mutex requires ordinary-pi, ordinary-ceiling, ordinary-fifo, ordinary-chain or ordinary-survivors");
            println!("cargo::rustc-cfg=jnox_ordinary_mutex");
            if app.as_deref() == Some("ordinary-fifo") {
                println!("cargo::rustc-cfg=jnox_mutex_fifo");
            } else if app.as_deref() == Some("ordinary-survivors") {
                assert!(!env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CARGO_FEATURE_")),
                    "ordinary-survivors requires no benchmark or diagnostic features");
                println!("cargo::rustc-cfg=jnox_mutex_survivors");
            } else if app.as_deref() == Some("ordinary-chain") {
                println!("cargo::rustc-cfg=jnox_mutex_chain");
            } else if app.as_deref() == Some("ordinary-ceiling") {
                println!("cargo::rustc-cfg=jnox_mutex_ceiling");
            }
        }
        Some("proportional") => {
            assert!(scenario.is_none(), "JNOX_CONTROL_CASE must be unset for JNOX_PROFILE=proportional");
            assert!(board == "virt" && bench == "boot", "ordinary proportional requires virt and no benchmark features");
            let case = app.as_deref().unwrap_or("");
            assert!(matches!(case,"lottery1"|"lottery2"|"stride"|"stride-delay"|"lottery-miss"|"lottery-window1"|"lottery-window2"|"lottery-window-tight"),
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
                Some(name @ ("ordinary-edf" | "ordinary-llf" | "ordinary-llf-overload" | "ordinary-edf-overrun" | "ordinary-edf-supply" | "ordinary-llf-supply" | "ordinary-edf-supply-refuse" | "ordinary-llf-supply-refuse" | "ordinary-edf-recurring" | "ordinary-edf-recurring-refuse" | "ordinary-llf-recurring" | "ordinary-llf-recurring-refuse")) => {
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
            assert!(board == "virt" && bench == "boot" && matches!(app.as_deref(),
                    Some("ordinary-groups" | "ordinary-groups-late" | "ordinary-groups-recurring")),
                    "groups requires virt, a supported ordinary group application, and no benchmark");
            println!("cargo::rustc-cfg=jnox_groups");
            if app.as_deref() == Some("ordinary-groups-late") { println!("cargo::rustc-cfg=jnox_groups_late"); }
            if app.as_deref() == Some("ordinary-groups-recurring") { println!("cargo::rustc-cfg=jnox_groups_recurring"); }
        }
        Some("multilevel") => {
            assert!(bench == "boot" && board == "virt", "multilevel requires virt without bench features");
            match app.as_deref() {
                Some(name @ ("mlq" | "mlfq" | "mlq-blocked" | "mlq-exited" | "mlq-saturated" | "mlfq-delayed" | "mlq-admit" | "mlq-refuse" | "mlfq-wake" | "mlfq-wake-refuse")) => {
                    if name.starts_with("mlfq-wake") { println!("cargo::rustc-cfg=jnox_mlfq_wake"); }
                    if matches!(name, "mlq-admit" | "mlq-refuse") { println!("cargo::rustc-cfg=jnox_mlq_admission"); }
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
                Some(name @ ("fixed-fifo" | "fixed-rr" | "fixed-empty" | "fixed-delayed" | "fixed-ties" | "fixed-late" | "coop-events" | "coop-single" | "coop-bounded")) => {
                    if name.starts_with("coop-") {
                        assert!(trace.is_none(), "ordinary cooperative service requires JNOX_FIXED_TRACE unset");
                    }
                    println!("cargo::rustc-cfg=jnox_fixed");
                    println!("cargo::rustc-cfg=jnox_app=\"{name}\"");
                }
                _ => panic!("JNOX_PROFILE=fixed requires JNOX_APP=fixed-fifo, fixed-rr, fixed-empty, fixed-delayed, fixed-ties, fixed-late, coop-events, coop-single, or coop-bounded"),
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
        Some(other) => panic!("unknown JNOX_PROFILE {other:?}; expected fixed, batch, groups, weighted, eevdf, multilevel, periodic, aperiodic, native or deadline"),
        None => assert!(app.is_none(), "JNOX_APP requires JNOX_PROFILE=fixed, batch, groups, weighted, eevdf, multilevel, periodic, aperiodic, native or deadline"),
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
                        | "CARGO_FEATURE_ORDINARY_PARTITION_P" | "CARGO_FEATURE_ORDINARY_PARTITION_Q")
                        || (migration_stream && key == "CARGO_FEATURE_PI4_STREAM_V1")),
                "JNOX_BOARD=pi4 supports serial boot, sched-bench, or uninstrumented fixed FIFO/RR"
            );
            println!("cargo::rustc-cfg=jnox_pi4");
            if pi4_native_fixed || pi4_native_smp { "pi4-native.ld" } else { "pi4.ld" }
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

fn parse_batch_job(value: &str) -> (u64, u64) {
    let (offset, estimate) = value.split_once(',').expect("batch job requires offset,estimate");
    let parse = |s: &str| {
        assert!(!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()), "invalid batch job decimal");
        let ticks = s.parse::<u64>().expect("batch ticks exceed u64");
        assert!(ticks <= i64::MAX as u64, "batch ticks exceed i64::MAX");
        ticks
    };
    let job = (parse(offset), parse(estimate));
    assert!(job.1 > 0, "batch estimate must be positive");
    job
}

fn parse_native_tuple(value: &str) -> (u64, u64, u64) {
    let mut fields = value.split(',');
    let mut next = || {
        let field = fields.next().expect("native scheduling tuple requires three integers");
        assert!(!field.is_empty() && field.bytes().all(|c| c.is_ascii_digit()), "invalid scheduling tuple number");
        field.parse::<u64>().expect("scheduling tuple number exceeds u64")
    };
    let job = (next(), next(), next());
    assert!(fields.next().is_none(), "extra scheduling tuple fields");
    job
}

fn parse_deadline_job(value: &str) -> (u64, u64, u64) {
    let job = parse_native_tuple(value);
    assert!(job.0 < job.1 && job.1 <= 16 && job.2 > 0, "invalid finite deadline job");
    job
}

fn parse_budget_tuple(value: &str) -> (u32, u32) {
    let (quota, period) = value.split_once(',').expect("budget requires quota,period");
    let parse = |field: &str| {
        assert!(!field.is_empty() && field.bytes().all(|c| c.is_ascii_digit()), "invalid budget number");
        field.parse::<u32>().expect("budget number exceeds u32")
    };
    let (quota, period) = (parse(quota), parse(period));
    assert!(quota > 0 && quota <= period, "budget requires 0 < quota <= period");
    (quota, period)
}

fn decimal_number(field: &str) -> u64 {
    assert!(!field.is_empty() && field.bytes().all(|c| c.is_ascii_digit()), "expected unsigned decimal digits");
    field.parse().expect("decimal setting exceeds u64")
}

fn multilevel_duration(field: &str) -> u64 {
    let value = decimal_number(field);
    assert!((1..=i64::MAX as u64).contains(&value), "duration must be in 1..=i64::MAX");
    value
}

// Hex wire: u8 count, then (u16 little-endian byte length, UTF-8 payload) per extra.
fn native_extra_literals(slot: usize, encoded: Option<&str>) -> String {
    let Some(encoded) = encoded else { return "&[]".to_owned() };
    const MAX_COUNT: usize = 15;
    const MAX_BYTES: usize = 1024;
    // Two length bytes per item, one of which replaces its budgeted NUL.
    const MAX_HEX: usize = 2 * (1 + MAX_BYTES + MAX_COUNT);
    assert!(!encoded.is_empty() && encoded.len() <= MAX_HEX && encoded.len() % 2 == 0,
        "slot {slot}: invalid argument vector hex length (up to {MAX_HEX} digits)");
    assert!(encoded.bytes().all(|c| c.is_ascii_hexdigit()), "slot {slot}: invalid hex digit");
    let raw: Vec<u8> = (0..encoded.len()).step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).unwrap()).collect();
    let count = usize::from(raw[0]);
    assert!(count <= MAX_COUNT, "slot {slot}: at most {MAX_COUNT} extra arguments");
    let mut rest = &raw[1..];
    let mut used = 0;
    let mut literals = Vec::with_capacity(count);
    for _ in 0..count {
        assert!(rest.len() >= 2, "slot {slot}: missing argument length");
        let size = usize::from(u16::from_le_bytes([rest[0], rest[1]]));
        rest = &rest[2..];
        assert!(rest.len() >= size, "slot {slot}: truncated argument");
        let (value, tail) = rest.split_at(size);
        rest = tail;
        used += size + 1;
        assert!(used <= MAX_BYTES, "slot {slot}: arguments exceed {MAX_BYTES} bytes including NULs");
        assert!(!value.contains(&0), "slot {slot}: argument contains NUL");
        assert!(std::str::from_utf8(value).is_ok(), "slot {slot}: invalid UTF-8");
        literals.push(format!("&{value:?}"));
    }
    assert!(rest.is_empty(), "slot {slot}: trailing argument bytes");
    format!("&[{}]", literals.join(", "))
}

// Canonical amc-v1 is ASCII LF-delimited. Each task is followed by one native
// argument-vector hex record per generation. Reset rows always follow all tasks.
fn amc_schedule(file: &str, expected: &str, startup: &mut [String]) -> String {
    use std::{fs, io::Read, path::{Path, PathBuf}, process::Command};
    const MAX_CELL: u64 = u64::MAX / 3_125_000;
    fn decimal(text: &str) -> u64 {
        assert!(!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
            && (text == "0" || !text.starts_with('0')), "invalid canonical AMC decimal");
        text.parse().expect("AMC integer exceeds u64")
    }
    let input = Path::new(file).canonicalize().expect("locate AMC input");
    println!("cargo::rerun-if-changed={}", input.display());
    let mut raw = String::new();
    fs::File::open(input).unwrap().take(150_001).read_to_string(&mut raw).expect("read canonical AMC manifest");
    assert!(raw.len() <= 150_000 && raw.is_ascii() && raw.ends_with('\n'), "invalid AMC canonical size or encoding");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("amc-v1.txt");
    fs::write(&out, &raw).expect("snapshot canonical AMC input");
    let result = Command::new("sha256sum").arg(out).output().expect("run sha256sum");
    assert!(result.status.success() && std::str::from_utf8(&result.stdout).unwrap().split_whitespace().next() == Some(expected),
        "canonical AMC SHA-256 mismatch");
    let mut lines = raw.split_terminator('\n');
    assert_eq!(lines.next(), Some("amc-v1"), "unknown AMC wire version");
    let mut tasks = Vec::new();
    let mut argument_tables = Vec::new();
    let mut resets = Vec::new();
    while let Some(line) = lines.next() {
        let fields: Vec<_> = line.split(' ').collect();
        match fields.as_slice() {
            ["task", tier, priority, clo, chi, deadline, period, offset, count] => {
                let slot = tasks.len();
                assert!(slot < startup.len() && resets.is_empty(), "invalid AMC task order/count");
                let hi = match *tier { "HI" => true, "LO" => false, _ => panic!("invalid tier") };
                let (priority, lo, high) = (decimal(priority), decimal(clo), decimal(chi));
                let (d, t, first, n) = (decimal(deadline), decimal(period), decimal(offset), decimal(count));
                assert!((1..=255).contains(&priority) && 0 < lo && lo <= high && high <= d && d <= t
                    && (hi || lo == high) && (1..=16).contains(&n), "invalid AMC descriptor");
                let last = (n-1).checked_mul(t).and_then(|v| first.checked_add(v)).and_then(|v| v.checked_add(d));
                assert!(last.is_some_and(|v| v <= MAX_CELL), "AMC deployment timeline overflow");
                tasks.push((hi, priority as u8, lo, high, d, t, first, n));
                let mut generations = Vec::new();
                for generation in 0..n {
                    let hex = lines.next().and_then(|line| line.strip_prefix("argv ")).expect("missing generation argv");
                    assert!(!hex.bytes().any(|b| b.is_ascii_uppercase()), "noncanonical argument hex");
                    let literal = native_extra_literals(slot, Some(hex));
                    if generation == 0 { startup[slot] = literal.clone(); }
                    generations.push(literal);
                }
                argument_tables.push(format!("&[{}]", generations.join(",")));
            }
            ["reset", cell, approval] => {
                let cell = decimal(cell);
                assert!(tasks.len() == startup.len() && resets.len() < 16 && cell <= MAX_CELL
                    && resets.last().is_none_or(|&(prev, _)| prev < cell), "invalid AMC reset sequence");
                let approved = match *approval { "0" => false, "1" => true, _ => panic!("invalid approval") };
                resets.push((cell, approved));
            }
            _ => panic!("invalid canonical AMC row"),
        }
    }
    assert_eq!(tasks.len(), startup.len(), "AMC slot descriptor count mismatch");
    format!("pub const NATIVE_AMC_TASKS: [(bool,u8,u64,u64,u64,u64,u64,u64); {}] = {tasks:?};\npub const NATIVE_AMC_RESETS: &[(u64,bool)] = &{resets:?};\npub const NATIVE_AMC_ARGV: &[&[&[&[u8]]]] = &[{}];\n", tasks.len(), argument_tables.join(","))
}

fn partition_windows(table: &str) -> Vec<(Option<usize>, u64)> {
    let mut windows = Vec::new();
    let mut frame_ms = 0u64;
    for window in table.split(',') {
        assert!(windows.len() < 16, "too many partition windows");
        let (owner, raw) = window.split_once(':').expect("expected owner:duration");
        let slot = match owner {
            "a" => Some(0),
            "b" => Some(1),
            "idle" => None,
            _ => panic!("unknown partition owner"),
        };
        assert!(!raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()), "invalid duration");
        let ms: u64 = raw.parse().expect("duration exceeds u64");
        assert!(ms > 0, "duration must be positive");
        frame_ms = frame_ms.checked_add(ms).expect("partition frame exceeds u64 milliseconds");
        windows.push((slot, ms));
    }
    for owner in [Some(0), Some(1)] {
        assert!(windows.iter().any(|&(slot, _)| slot == owner), "partition requires both owners");
    }
    windows
}

fn forecast_lottery<const N: usize>(weights: &[u32], seed: u64, horizon: usize, width: usize, floors: &[u8]) {
    let input = proportional_policy::NativeWindowInput::<N> {
        weights: std::array::from_fn(|id| weights[id] as u8), seed, horizon, width,
        floors: floors.try_into().expect("one floor per owner"),
    };
    let report = input.analyze().expect("invalid lottery admission configuration");
    if let Some((start, owner, count)) = report.first_short {
        panic!("lottery interval [{start},{}) owner {owner}: count {count} < floor {}",
            start + width, floors[owner]);
    }
}

fn native_smp_bytes(raw: &str, slots: usize) -> Vec<u8> {
    let values: Vec<_> = raw.split(',').map(|s| {
        assert!(!s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()), "expected decimal SMP field");
        s.parse::<u8>().expect("SMP field outside 0..255")
    }).collect();
    assert_eq!(values.len(), slots, "expected one SMP value per slot");
    values
}

fn write_native_schedule(slots: usize, policy: Option<&str>, priorities: Option<&str>, weights: Option<&str>, quantum: Option<&str>, edf_jobs: Option<&str>, periodic: Option<&str>, reservations: Option<&str>, multilevel: [Option<&str>; 3], argv: &[Option<String>; 4], thread_rr: bool, service_mask: u16, switch_cap: u64, partition: Option<&str>, server_settings: Option<(u64, u32, u32, u64)>, notification: (&str, u64), hierarchy: Option<&str>, thread_priorities: Option<[u8; 4]>, lottery: [Option<&str>; 3], batch: Option<&str>, amc: Option<(&str, &str)>, smp_table: Option<(Vec<u8>, Vec<u8>)>) {
    let batch_jobs = if let Some(raw) = batch {
        let jobs: Vec<_> = raw.split(';').map(parse_batch_job).collect();
        assert_eq!(jobs.len(), slots, "provide one batch tuple per slot");
        println!("cargo::rustc-cfg=jnox_native_batch");
        jobs
    } else {
        vec![(0, 0); slots]
    };
    if server_settings.is_some() {
        assert_eq!(slots, 2, "native servers require exactly two ordinary applications");
        println!("cargo::rustc-cfg=jnox_native_aperiodic");
    }
    let (slot_ms, quota, period, credits) = server_settings.unwrap_or_default();
    let (notify_mode, notify_count) = notification;
    let windows = if let Some(table) = partition {
        assert_eq!(slots, 2, "partition requires exactly two ordinary processes");
        println!("cargo::rustc-cfg=jnox_native_partition");
        partition_windows(table)
    } else {
        Vec::new()
    };
    let mode = policy.unwrap_or("cooperative");
    let slot_weights = if matches!(mode, "lottery" | "stride") {
        let list: Vec<u32> = weights.expect("proportional scheduling requires weights").split(',')
            .map(|field| {
                let value = decimal_number(field);
                assert!((1..=16).contains(&value), "proportional weights must be in 1..16");
                value as u32
            }).collect();
        assert_eq!(list.len(), slots, "supply one weight per slot");
        println!("cargo::rustc-cfg=jnox_native_proportional");
        list
    } else if mode == "weighted" {
        let values: Vec<u32> = weights.expect("weighted scheduling requires weights").split(',')
            .map(|value| value.parse().expect("weight must be a positive u32")).collect();
        assert_eq!(values.len(), slots, "supply one weight per slot");
        assert!(values.iter().all(|&value| value != 0), "weights must be positive");
        println!("cargo::rustc-cfg=jnox_native_weighted");
        values
    } else {
        assert!(weights.is_none(), "weights require weighted scheduling");
        vec![0; slots]
    };
    let [seed, window, floors] = lottery;
    let seed = seed.map(decimal_number).unwrap_or(0);
    let mut horizon = 0usize;
    let mut width = 0usize;
    let mut counts = vec![0u8; slots];
    if let (Some(window), Some(floors)) = (window, floors) {
        let (h, w) = window.split_once(',').expect("expected H,W");
        let (h, w) = (decimal_number(h), decimal_number(w));
        assert!(1 <= w && w <= h && h <= 64, "require 1 <= W <= H <= 64");
        (horizon, width) = (h as usize, w as usize);
        counts = floors.split(',').map(|v| u8::try_from(decimal_number(v)).expect("floor exceeds u8")).collect();
        assert!(counts.len() == slots && counts.iter().map(|&v| usize::from(v)).sum::<usize>() <= width,
            "supply one floor per slot summing to at most W");
        match slots {
            1 => forecast_lottery::<1>(&slot_weights, seed, horizon, width, &counts),
            2 => forecast_lottery::<2>(&slot_weights, seed, horizon, width, &counts),
            3 => forecast_lottery::<3>(&slot_weights, seed, horizon, width, &counts),
            _ => unreachable!(),
        }
    }
    let llf = mode == "llf";
    let jobs = if matches!(mode, "edf" | "llf") {
        let jobs: Vec<_> = edf_jobs.expect("EDF and LLF require job tuples").split(';').map(parse_deadline_job).collect();
        assert_eq!(jobs.len(), slots - usize::from(service_mask != 0), "supply one tuple per ordinary deadline job");
        println!("cargo::rustc-cfg=jnox_native_edf");
        jobs
    } else {
        assert!(edf_jobs.is_none(), "job tuples require EDF or LLF");
        vec![(0, 0, 0); slots]
    };
    let periods = if matches!(mode, "rm" | "dm") {
        let list: Vec<_> = periodic.expect("RM and DM require periodic tuples")
            .split(';').map(parse_native_tuple).collect();
        assert_eq!(list.len(), slots, "supply one periodic tuple per slot");
        assert!(list.iter().all(|&(_, period, deadline)| deadline > 0 && deadline <= period),
            "require 0 < relative deadline <= period");
        println!("cargo::rustc-cfg=jnox_native_periodic");
        list
    } else {
        assert!(periodic.is_none(), "periodic tuples require RM or DM");
        vec![(0, 0, 0); slots]
    };
    let servers = if mode == "cbs" {
        let list: Vec<_> = reservations.expect("CBS requires reservations")
            .split(';').map(parse_budget_tuple).collect();
        assert_eq!(list.len(), slots, "supply one CBS reservation per slot");
        println!("cargo::rustc-cfg=jnox_native_cbs");
        list
    } else {
        assert!(reservations.is_none(), "reservations require CBS");
        vec![(0, 0); slots]
    };
    let [class_arg, allotment_arg, boost_arg] = multilevel;
    let mut classes = vec![0u8; slots];
    let mut allotments = [0u64; 3];
    let mut boost = 0u64;
    assert!(class_arg.is_none() || mode == "mlq", "classes require MLQ");
    assert!((allotment_arg.is_none() && boost_arg.is_none()) || mode == "mlfq",
        "allotments and boost require MLFQ");
    if mode == "mlq" {
        classes = class_arg.expect("MLQ requires classes").split(',').map(|field| {
            u8::try_from(decimal_number(field)).expect("class must be 0..255")
        }).collect();
        assert_eq!(classes.len(), slots, "supply one MLQ class per slot");
    }
    if mode == "mlfq" {
        let values: Vec<_> = allotment_arg.expect("MLFQ requires allotments").split(',')
            .map(multilevel_duration).collect();
        allotments = values.try_into().expect("MLFQ requires three allotments");
        boost = multilevel_duration(boost_arg.expect("MLFQ requires boost period"));
    }
    if matches!(mode, "mlq" | "mlfq") {
        println!("cargo::rustc-cfg=jnox_native_multilevel");
    }
    let values = match policy {
        Some("fifo" | "rr") if thread_priorities.is_some() || thread_rr => {
            assert!(priorities.is_none(), "process priorities are incompatible with thread scheduling");
            if thread_rr {
                println!("cargo::rustc-cfg=jnox_native_thread_rr");
            }
            vec![0; slots]
        }
        Some("smp-partitioned") => {
            let list = native_smp_bytes(priorities.expect("missing SMP priorities"), slots);
            assert!(list.iter().all(|p| *p != 0), "SMP priorities must be 1..255");
            list
        }
        Some("fifo" | "rr") => {
            let list: Vec<u8> = priorities.expect("fixed scheduling requires priorities").split(',')
                .map(|value| value.parse().expect("priority must be 0..255")).collect();
            assert_eq!(list.len(), slots, "supply one priority per slot");
            println!("cargo::rustc-cfg=jnox_native_fixed");
            list
        }
        Some("amc" | "sjf" | "srtf" | "hrrn" | "lottery" | "stride" | "weighted" | "edf" | "llf" | "rm" | "dm" | "cbs" | "mlq" | "mlfq" | "partition" | "polling" | "deferrable" | "sporadic" | "hierarchy" | "extension") | None => {
            assert!(priorities.is_none(), "priorities require an explicit fixed policy");
            vec![0; slots]
        }
        Some(other) => panic!("invalid native policy {other:?}"),
    };
    let ticks: u64 = if thread_rr || matches!(mode, "lottery" | "stride") {
        let raw = quantum.expect("quantum ticks required");
        assert!(!raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()), "expected unsigned decimal quantum");
        let value = raw.parse().expect("expected unsigned decimal quantum");
        assert!((1..=i64::MAX as u64).contains(&value), "quantum must be in 1..=i64::MAX");
        value
    } else if mode == "mlq" {
        multilevel_duration(quantum.expect("MLQ requires quantum ticks"))
    } else if matches!(mode, "rr" | "weighted") {
        let value = quantum.expect("RR and weighted scheduling require quantum ticks").parse().expect("invalid quantum ticks");
        assert!((1..=i64::MAX as u64).contains(&value), "quantum ticks must be in 1..=i64::MAX");
        value
    } else {
        assert!(quantum.is_none(), "quantum ticks require RR, weighted scheduling or MLQ");
        0
    };
    let mut extras: Vec<_> = argv[..slots].iter().enumerate()
        .map(|(slot, value)| native_extra_literals(slot, value.as_deref())).collect();
    if service_mask != 0 {
        // Only the package calendar can supply slot 2's argv[1].
        let calendar = (0..16).filter(|&k| service_mask & (1 << k) != 0)
            .map(|k| k.to_string()).collect::<Vec<_>>().join(",");
        extras[2] = format!("&[&{:?}]", calendar.as_bytes());
    }
    let job_count = jobs.len();
    let amc_constants = amc.map(|(file, hash)| amc_schedule(file, hash, &mut extras));
    let extra_slots = extras.join(", ");
    let thread_bases = thread_priorities.unwrap_or_default();
    let mut generated = format!(
        "pub const NATIVE_BATCH_JOBS: [(u64, u64); {slots}] = {batch_jobs:?};\npub const NATIVE_THREAD_PRIORITIES: [u8; 4] = {thread_bases:?};\npub const NATIVE_LOTTERY_SEED: u64 = {seed};\npub const NATIVE_LOTTERY_HORIZON: usize = {horizon};\npub const NATIVE_LOTTERY_WINDOW: usize = {width};\npub const NATIVE_LOTTERY_FLOORS: [u8; {slots}] = {counts:?};\npub const NATIVE_WEIGHTED_NOTIFY_MODE: &str = {notify_mode:?};\npub const NATIVE_WEIGHTED_NOTIFY_COUNT: u64 = {notify_count};\npub const NATIVE_SERVER_SLOT_MS: u64 = {slot_ms};\npub const NATIVE_SERVER_QUOTA: u64 = {quota};\npub const NATIVE_SERVER_PERIOD: u64 = {period};\npub const NATIVE_SERVER_CREDITS: usize = {credits};\npub const NATIVE_PARTITION_WINDOWS: &[(Option<usize>, u64)] = &{windows:?};\npub const NATIVE_ARGV_EXTRAS: [&[&[u8]]; {slots}] = [{extra_slots}];\npub const NATIVE_POLICY: &str = {mode:?};\npub const NATIVE_PRIORITIES: [u8; {slots}] = {values:?};\npub const NATIVE_WEIGHTS: [u32; {slots}] = {slot_weights:?};\npub const NATIVE_DEADLINE_LLF: bool = {llf};\npub const NATIVE_DEADLINE_SERVICE_MASK: u16 = {service_mask};\npub const NATIVE_DEADLINE_SWITCH_CAP: u64 = {switch_cap};\npub const NATIVE_EDF_JOBS: [(u64, u64, u64); {job_count}] = {jobs:?};\npub const NATIVE_PERIODIC: [(u64, u64, u64); {slots}] = {periods:?};\npub const NATIVE_RESERVATIONS: [(u32, u32); {slots}] = {servers:?};\npub const NATIVE_CLASSES: [u8; {slots}] = {classes:?};\npub const NATIVE_ALLOTMENT_TICKS: [u64; 3] = {allotments:?};\npub const NATIVE_BOOST_TICKS: u64 = {boost};\npub const NATIVE_QUANTUM_TICKS: u64 = {ticks};\n"
    );
    let path = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("native_schedule.rs");
    if let Some((masks, cells)) = smp_table {
        generated.push_str(&format!("pub const NATIVE_SMP_CPU_MASKS: [u8; {slots}] = {masks:?};\npub const NATIVE_SMP_RELEASE_CELLS: [u64; {slots}] = {cells:?};\n"));
        println!("cargo::rustc-cfg=jnox_native_smp_partitioned");
    }
    if mode == "extension" {
        generated.push_str("include!(concat!(env!(\"OUT_DIR\"), \"/native_extension_ops.rs\"));\n");
    }
    if let Some(constants) = amc_constants {
        generated.push_str(&constants);
        println!("cargo::rustc-cfg=jnox_native_amc");
    }
    if let Some(raw) = hierarchy {
        generated.push_str(&build_native_hierarchy::constants(raw, slots));
        println!("cargo::rustc-cfg=jnox_native_hierarchy");
    }
    std::fs::write(path, generated).expect("write native schedule");
}

fn package_native_image(source: &str, expected: &str, name: &str, digest_env: &str) {
    use std::{fs, path::Path, process::Command};
    assert!(expected.len() == 64 && expected.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')),
        "native ELF needs a lowercase SHA-256");
    let source = Path::new(source).canonicalize().expect("locate native ELF");
    println!("cargo::rerun-if-changed={}", source.display());
    let output = Path::new(&env::var_os("OUT_DIR").expect("missing Cargo output directory"))
        .join(name);
    let len = fs::copy(source, &output).expect("snapshot native ELF");
    assert!((64..=8 * 1024 * 1024).contains(&len), "native ELF package size");
    let actual = Command::new("sha256sum").arg(&output).output().expect("run sha256sum");
    let digest = std::str::from_utf8(&actual.stdout).expect("read ELF digest").split_whitespace().next();
    assert!(actual.status.success() && digest == Some(expected), "native ELF hash mismatch");
    println!("cargo::rustc-env={digest_env}={expected}");
}
