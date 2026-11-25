use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

#[allow(dead_code)]
#[path = "native/elf.rs"]
mod elf;
use elf::{LoadPlan, Permission};
include!("src/native_v1_note.rs");

const RAM: usize = 126 * 1024 * 1024;
const APP_RECORD: usize = 48;
const TASK_RECORD: usize = 64;
const MAX_LOCAL: usize = 30; // 17 pages per stack and guard, one L3 table.

fn word(row: &[u8], at: usize) -> usize {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap()) as usize
}

fn name(row: &[u8]) -> &[u8] {
    let field = &row[..32];
    let n = field.iter().position(|&b| b == 0).expect("unterminated policy name");
    assert!(n > 0 && (field[0].is_ascii_alphabetic() || field[0] == b'_')
        && field[..n].iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
        && field[n..].iter().all(|b| *b == 0), "invalid policy name");
    &field[..n]
}

fn validate(data: &[u8]) -> (usize, usize, usize) {
    assert!(data.len() >= 16 && &data[..4] == b"JNP1" && word(data, 4) == 1, "invalid policy header");
    let apps = word(data, 8);
    let tasks = word(data, 12);
    assert!((1..=255).contains(&apps) && (apps..=apps * MAX_LOCAL).contains(&tasks),
            "policy count exceeds ASID or stack layout");
    let table = 16 + APP_RECORD * apps;
    let mut offset = table + TASK_RECORD * tasks;
    assert!(offset <= data.len(), "truncated policy tables");
    let mut first = 0;
    let mut max_local = 0;
    let mut app_names = BTreeSet::new();
    for (id, app) in data[16..table].chunks_exact(APP_RECORD).enumerate() {
        assert!(app_names.insert(name(app)), "duplicate policy app");
        let len = word(app, 36);
        let count = word(app, 44);
        assert!(word(app, 32) == offset && len <= data.len() - offset
            && word(app, 40) == first && (1..=MAX_LOCAL).contains(&count)
            && count <= tasks - first, "invalid policy app range");
        let image = &data[offset..offset + len];
        let app_name = std::str::from_utf8(name(app)).expect("validated ASCII app name");
        let plan = elf::plan(image, 0x400000..0x800000)
            .unwrap_or_else(|error| panic!("app {app_name}: invalid ELF layout: {error:?}"));
        let permissions = [Permission::ReadExecute, Permission::Read, Permission::ReadWrite];
        assert!(plan.entry == 0x400000 && plan.segments().iter().map(|s| s.permission).eq(permissions),
                "app {app_name}: native ABI requires entry 0x400000 and RX, R, RW segments");
        assert!(native_v1_note(image, &plan).is_some(),
                "app {app_name}: invalid or missing native v1 ABI note");
        let mut names = BTreeSet::new();
        for slot in 0..count {
            let at = table + (first + slot) * TASK_RECORD;
            let task = &data[at..at + TASK_RECORD];
            assert!(names.insert(name(task)), "duplicate task name in application");
            assert!(word(task, 32) == id && word(task, 36) == slot, "invalid policy task owner");
            let entry = u64::from_le_bytes(task[40..48].try_into().unwrap());
            assert!(entry % 4 == 0 && plan.segments().iter().any(|s| {
                s.permission == elf::Permission::ReadExecute && entry >= s.address
                    && entry.checked_add(4).is_some_and(|end| end <= s.address + s.bytes.len() as u64)
            }), "policy entry is outside executable file data");
            assert!((1..=255).contains(&word(task, 52))
                && (1..=word(task, 48)).contains(&word(task, 56)), "invalid policy priority or deadline");
        }
        first += count;
        offset += len;
        max_local = max_local.max(count);
    }
    assert!(first == tasks && offset == data.len(), "policy contains unclaimed records or bytes");
    let app_storage = 4 * 1024 * 1024 + (5 + 17 * max_local - 1) * 4096;
    assert!(apps * app_storage + 63 * 4096 + data.len() <= RAM, "policy exceeds virt RAM");
    (apps, tasks, max_local)
}

pub fn configure(source: &str, expected: &str) {
    assert!(expected.len() == 64 && expected.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')),
            "expected lowercase policy SHA-256");
    let source = fs::canonicalize(source).expect("locate task policy");
    println!("cargo::rerun-if-changed={}", source.display());
    let file = fs::File::open(&source).expect("open task policy");
    assert!(file.metadata().expect("stat task policy").len() <= RAM as u64, "policy exceeds virt RAM");
    let mut bytes = Vec::new();
    file.take(RAM as u64 + 1).read_to_end(&mut bytes).expect("read task policy");
    assert!(bytes.len() <= RAM, "policy exceeds virt RAM");
    let (apps, tasks, max_local) = validate(&bytes);
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("missing Cargo output directory"));
    let snapshot = out.join("task_policy.bin");
    fs::write(&snapshot, &bytes).expect("write policy snapshot");
    #[cfg(not(windows))]
    let command = Command::new("sha256sum").arg(&snapshot).output();
    #[cfg(windows)]
    let command = Command::new("certutil.exe").arg("-hashfile").arg(&snapshot).arg("SHA256").output();
    let result = command.expect("hash policy snapshot");
    let output = String::from_utf8_lossy(&result.stdout);
    #[cfg(not(windows))]
    let digest = output.split_ascii_whitespace().next();
    #[cfg(windows)]
    let digest = output.lines().map(str::trim).find(|line| line.len() == 64);
    assert!(result.status.success() && digest == Some(expected), "task policy SHA-256 mismatch");
    fs::write(out.join("task_policy_sizes.rs"), format!(
        "pub const APP_COUNT: usize = {apps};\npub const TASK_COUNT: usize = {tasks};\npub const MAX_TASKS_PER_APP: usize = {max_local};\n"
    )).expect("write policy sizes");
    println!("cargo::rustc-env=JNOX_TASK_POLICY_SHA256={expected}");
    println!("cargo::rustc-cfg=jnox_task_policy");
    println!("cargo::rustc-cfg=jnox_native_el0");
}
