#!/usr/bin/env python3
"""Package a native AArch64 application and a kernel containing that exact ELF."""
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
SOURCE = ROOT / "examples/portability/native_startup_probe.c"
TARGET = "aarch64-unknown-none"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def output(args, cwd=ROOT, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="new package directory")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--source", type=Path, help="C application to compile with the native SDK")
    group.add_argument("--elf", type=Path, help="already linked native AArch64 application")
    parser.add_argument("--peer-elf", type=Path, help="independently linked second native application")
    args = parser.parse_args()
    if args.peer_elf and not (args.source or args.elf):
        parser.error("--peer-elf requires an ordinary primary --source or --elf")
    peer_input = args.peer_elf.resolve(strict=True) if args.peer_elf else None
    peer_digest = sha256(peer_input) if peer_input else None
    app_input = (args.elf or args.source or SOURCE).resolve(strict=True)
    input_digest = sha256(app_input)
    app = "process" if args.source or args.elf else "startup-probe"
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    user_elf = out / "user.elf"
    kernel_elf = out / "kernel.elf"
    env = {k: v for k, v in os.environ.items() if not k.startswith(("JNOX_", "CARGO_FEATURE_"))
           and k not in ("CARGO_BUILD_TARGET", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
    commit = output(["git", "rev-parse", "HEAD"])
    dirty = output(["git", "status", "--porcelain", "--untracked-files=no"])
    sdk_files = sorted(p for p in SDK.rglob("*") if p.is_file())
    inputs = {str(p.relative_to(ROOT)): sha256(p) for p in sdk_files if not args.elf}
    input_name = str(app_input.relative_to(ROOT) if app_input.is_relative_to(ROOT) else app_input)
    inputs[input_name] = input_digest
    user_command = None if args.elf else ["sh", str(SDK / "build.sh"), str(app_input), str(user_elf)]
    kernel_command = [sys.executable, "tools/build_kernel.py", "--arch", "aarch64"]
    metadata = {
        "format": 1, "profile": "native", "app": app, "board": "virt",
        "commit": commit, "tracked_changes": dirty, "source_sha256": inputs,
        "input": {"kind": "elf" if args.elf else "c", "path": input_name, "sha256": input_digest},
        "compiler": None if args.elf else output(["clang", "--version"], env=env),
        "linker": None if args.elf else output(["ld.lld", "--version"], env=env),
        "rust": output(["rustc", "-vV"], ROOT / "kernel", env),
        "cargo": output(["cargo", "--version"], ROOT / "kernel", env),
        "sdk_command": user_command, "kernel_command": kernel_command,
    }
    with (out / "build.log").open("w", encoding="utf-8") as log:
        if args.elf:
            shutil.copyfile(app_input, user_elf)
        else:
            subprocess.run(user_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                           check=True)
        if sha256(user_elf if args.elf else app_input) != input_digest:
            raise ValueError("application input changed during packaging")
        if peer_input:
            peer_elf = out / "native_peer.elf"
            shutil.copyfile(peer_input, peer_elf)
            if sha256(peer_elf) != peer_digest:
                raise ValueError("peer ELF changed during packaging")
            peer_name = str(peer_input.relative_to(ROOT) if peer_input.is_relative_to(ROOT) else peer_input)
            inputs[peer_name] = peer_digest
            metadata.update(peer_input={"kind": "elf", "path": peer_name, "sha256": peer_digest},
                            peer_elf_sha256=peer_digest)
            env.update(JNOX_NATIVE_PEER_ELF=str(peer_elf), JNOX_NATIVE_PEER_ELF_SHA256=peer_digest)
        digest = sha256(user_elf)
        env.update(JNOX_BOARD="virt", JNOX_PROFILE="native", JNOX_APP=app,
                   JNOX_NATIVE_ELF=str(user_elf), JNOX_NATIVE_ELF_SHA256=digest,
                   CARGO_TARGET_DIR=str(out / "target"))
        subprocess.run(kernel_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                       check=True)
    shutil.copyfile(out / "target" / TARGET / "release/jnox-kernel", kernel_elf)
    metadata.update(user_elf_sha256=digest, kernel_elf_sha256=sha256(kernel_elf),
                    build_log_sha256=sha256(out / "build.log"))
    (out / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"kernel {metadata['kernel_elf_sha256']}  {kernel_elf}")
    print(f"native {digest}  {user_elf}")
    if peer_input:
        print(f"peer {peer_digest}  {peer_elf}")


if __name__ == "__main__":
    main()
