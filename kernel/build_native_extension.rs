use std::{env, fs, path::PathBuf, process::Command};

fn number(raw: &str, max: u64) -> u64 {
    assert!(!raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()), "invalid extension operand");
    let value: u64 = raw.parse().expect("extension operand exceeds u64");
    assert!(value <= max, "extension operand out of range");
    value
}

pub fn configure(path: &str, digest: &str) {
    assert!(digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "expected lowercase extension program SHA-256");
    let input = fs::canonicalize(path).expect("locate extension program");
    println!("cargo::rerun-if-changed={}", input.display());
    let source = fs::read_to_string(input).expect("read UTF-8 extension program");
    let lines: Vec<_> = source.split('\n').map(|line| line.split_whitespace().collect::<Vec<_>>())
        .filter(|fields| !fields.is_empty()).collect();
    assert!((1..=16).contains(&lines.len()), "extension requires 1..16 operations");
    let target = |s: &str| number(s, lines.len() as u64 - 1);
    let mut canonical = String::new();
    let mut generated = String::from("pub(super) const NATIVE_EXTENSION_PROGRAM: &[extension_policy::Op] = &[\n");
    for fields in &lines {
        let (text, op) = match fields.as_slice() {
            ["ready", id, yes, no] => {
                let (id, yes, no) = (number(id, 2), target(yes), target(no));
                (format!("ready {id} {yes} {no}"), format!("IfReady {{ task: {id}, yes: {yes}, no: {no} }}"))
            }
            ["jump", pc] => {
                let pc = target(pc);
                (format!("jump {pc}"), format!("Jump({pc})"))
            }
            ["propose", id, generation, cpu, ticks] => {
                let (id, generation) = (number(id, u32::MAX.into()), number(generation, u64::MAX));
                let (cpu, ticks) = (number(cpu, u8::MAX.into()), number(ticks, u64::MAX));
                (format!("propose {id} {generation} {cpu} {ticks}"),
                 format!("Propose(group_policy::Proposal {{ task: {id}, generation: {generation}, cpu: {cpu}, ticks: {ticks} }})"))
            }
            ["yield"] => ("yield".to_owned(), "Yield".to_owned()),
            _ => panic!("invalid extension operation or operand count"),
        };
        canonical.push_str(&text);
        canonical.push('\n');
        generated.push_str(&format!("    extension_policy::Op::{op},\n"));
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let copy = out.join("native_extension_canonical.txt");
    fs::write(&copy, canonical).expect("snapshot canonical extension");
    let result = Command::new("sha256sum").arg(copy).output().expect("run sha256sum");
    assert!(result.status.success() && std::str::from_utf8(&result.stdout).unwrap().split_whitespace().next() == Some(digest),
        "canonical extension SHA-256 mismatch");
    generated.push_str(&format!("];\npub(super) const NATIVE_EXTENSION_GENERATIONS: [u64; 3] = [7, 11, 17];\npub(super) const NATIVE_EXTENSION_SHA256: &str = \"{digest}\";\n"));
    fs::write(out.join("native_extension_ops.rs"), generated).expect("write typed native extension");
    println!("cargo::rustc-cfg=jnox_native_extension");
}
