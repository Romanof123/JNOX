use std::{env, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let sdk = root.join("../../../sdk/native-aarch64/include");
    let object = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("control1.o");
    println!("cargo:rerun-if-changed=../control1.c");
    println!("cargo:rerun-if-changed=../sample.h");
    println!("cargo:rerun-if-changed={}", sdk.display());
    println!("cargo:rerun-if-changed=../../native-amc/ring.h");
    let status = Command::new("clang")
        .args(["--target=aarch64-none-elf", "-std=c11", "-O2", "-ffreestanding",
               "-nostdinc", "-fno-pic", "-fno-stack-protector", "-fno-unwind-tables",
               "-fno-asynchronous-unwind-tables", "-mgeneral-regs-only", "-mno-outline-atomics",
               "-I"])
        .arg(sdk).arg("-c").arg(root.join("../control1.c")).arg("-o").arg(&object)
        .status().expect("start native C compiler");
    assert!(status.success(), "compile ../control1.c");
    println!("cargo:rustc-link-arg={}", object.display());
}
