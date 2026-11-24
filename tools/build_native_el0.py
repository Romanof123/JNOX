#!/usr/bin/env python3
"""Package a native C application and a kernel containing that exact ELF."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SDK = ROOT / "sdk/native-aarch64"
SOURCES = {
    "startup-probe": "native_startup_probe.c",
    "weighted-share": "native_weighted_monitor.c",
    "partition-p": "native_partition_monitor.c",
    "partition-q": "native_partition_monitor.c",
    "weighted-wake": "native_weighted_monitor.c",
}
TARGET = "aarch64-unknown-none"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def output(args, cwd=ROOT, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True).strip()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="new package directory")
    parser.add_argument("--app", choices=SOURCES, default="startup-probe")
    parser.add_argument("--partition-phase", choices=("irq-v1",),
                        help="include native P/Q IRQ stage timestamps in the kernel")
    parser.add_argument("--partition-frames", choices=("retained-v1",),
                        help="retain independent native P/Q save and resume banks")
    args = parser.parse_args(argv)
    if args.partition_frames and args.app not in ("partition-p", "partition-q"):
        parser.error("--partition-frames requires --app partition-p or partition-q")
    if args.partition_phase and args.app not in ("partition-p", "partition-q"):
        parser.error("--partition-phase requires --app partition-p or partition-q")
    source = ROOT / "examples/portability" / SOURCES[args.app]
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    user_elf = out / "user.elf"
    kernel_elf = out / "kernel.elf"
    env = {k: v for k, v in os.environ.items() if not k.startswith(("JNOX_", "CARGO_FEATURE_"))
           and k not in ("CARGO_BUILD_TARGET", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
    commit = output(["git", "rev-parse", "HEAD"])
    dirty = output(["git", "status", "--porcelain", "--untracked-files=no"])
    sdk_files = sorted(p for p in SDK.rglob("*") if p.is_file())
    inputs = {str(p.relative_to(ROOT)): sha256(p) for p in [source, *sdk_files]}
    user_command = ["sh", str(SDK / "build.sh"), str(source), str(user_elf)]
    kernel_command = [sys.executable, "tools/build_kernel.py", "--arch", "aarch64"]
    metadata = {
        "format": 1, "profile": "native", "app": args.app, "board": "virt",
        "commit": commit, "tracked_changes": dirty, "source_sha256": inputs,
        "compiler": output(["clang", "--version"], env=env),
        "linker": output(["ld.lld", "--version"], env=env),
        "rust": output(["rustc", "-vV"], ROOT / "kernel", env),
        "cargo": output(["cargo", "--version"], ROOT / "kernel", env),
        "sdk_command": user_command, "kernel_command": kernel_command,
    }
    with (out / "build.log").open("w", encoding="utf-8") as log:
        subprocess.run(user_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                       check=True)
        digest = sha256(user_elf)
        env.update(JNOX_BOARD="virt", JNOX_PROFILE="native", JNOX_APP=args.app,
                   JNOX_NATIVE_ELF=str(user_elf), JNOX_NATIVE_ELF_SHA256=digest,
                   CARGO_TARGET_DIR=str(out / "target"))
        if args.partition_frames:
            env["JNOX_PARTITION_FRAMES"] = args.partition_frames
            metadata["partition_frames"] = args.partition_frames
        if args.partition_phase:
            env["JNOX_PARTITION_PHASE"] = args.partition_phase
            metadata["partition_phase"] = args.partition_phase
        subprocess.run(kernel_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                       check=True)
    shutil.copyfile(out / "target" / TARGET / "release/jnox-kernel", kernel_elf)
    metadata.update(user_elf_sha256=digest, kernel_elf_sha256=sha256(kernel_elf),
                    build_log_sha256=sha256(out / "build.log"))
    (out / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"kernel {metadata['kernel_elf_sha256']}  {kernel_elf}")
    print(f"native {digest}  {user_elf}")


if __name__ == "__main__":
    main()
