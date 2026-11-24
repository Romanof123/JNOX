// Host entry point: rustc --edition=2024 --test tools/sched_app_trace_controls.rs
#![allow(dead_code)]

mod multilevel_service {
    pub fn counter() -> u64 {
        panic!("host state controls use explicit record bodies")
    }
}

#[path = "../kernel/src/sched_app_trace.rs"]
mod sched_app_trace;
