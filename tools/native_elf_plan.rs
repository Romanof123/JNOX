//! Inspect the layout of one static AArch64 native SDK executable.
#[path = "../kernel/native/elf.rs"]
mod elf;
use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: native_elf_plan IMAGE");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("usage: native_elf_plan IMAGE");
        return ExitCode::from(2);
    }
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read: {e}");
            return ExitCode::FAILURE;
        }
    };
    match elf::plan(&bytes, 0x0040_0000..0x0080_0000) {
        Ok(plan) => {
            println!("entry={:#x} segments={}", plan.entry, plan.segments().len());
            for s in plan.segments() {
                println!(
                    "{:?} va={:#x} file_offset={:#x} copy={} bss={} page_end={:#x}",
                    s.permission,
                    s.address,
                    s.file_offset,
                    s.bytes.len(),
                    s.memory_size - s.bytes.len() as u64,
                    s.page_end
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("layout rejected: {e:?}");
            ExitCode::FAILURE
        }
    }
}
