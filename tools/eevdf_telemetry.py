#!/usr/bin/env python3
"""Build and capture the finite EEVDF telemetry deployment once, or replay it offline."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys

from ordinary_amc_capture import save, sha
from ordinary_eevdf_replay import Unobserved, replay

ROOT = Path(__file__).resolve().parents[1]
TARGET = "aarch64-unknown-none"
PROFILE = {"JNOX_BOARD": "virt", "JNOX_PROFILE": "eevdf", "JNOX_APP": "telemetry"}


def text(command, cwd=ROOT, env=None):
    return subprocess.check_output(command, cwd=cwd, env=env, text=True).strip()


def score(directory, digest):
    capture = directory / "capture"
    record = {"capture": str(capture), "session_sha256": digest}
    try:
        if not re.fullmatch("[a-f0-9]{64}", digest):
            raise ValueError("expected a lowercase SHA-256 digest")
        result = replay(capture, digest)
    except (Unobserved, FileNotFoundError, ImportError) as exc:
        record.update(verdict="UNOBSERVED", reason=str(exc))
        return 2, record
    except (ValueError, KeyError, OSError, TypeError, IndexError, struct.error) as exc:
        record.update(verdict="INVALID", reason=str(exc))
        return 1, record
    record.update(verdict="QUALIFIED", result=result)
    return 0, record


def run(directory):
    revision = text(["git", "rev-parse", "HEAD"])
    changes = text(["git", "status", "--porcelain=v1", "--untracked-files=all"])
    diff = subprocess.check_output(["git", "diff", "--binary", "HEAD"], cwd=ROOT)
    directory.mkdir(parents=True, exist_ok=False)
    capture = directory / "capture"
    target = directory / "target"
    elf = target / TARGET / "release" / "jnox-kernel"
    # No inherited Cargo feature, target, wrapper, or profile selector may switch this build.
    env = {k: v for k, v in os.environ.items() if not k.startswith(
        ("JNOX_", "CARGO_FEATURE_", "CARGO_BUILD_", "CARGO_PROFILE_", "CARGO_TARGET_"))
        and k not in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
                      "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN")}
    env.update(PROFILE, CARGO_TARGET_DIR=str(target))
    build = [sys.executable, str(ROOT / "tools/build_kernel.py"), "--arch", "aarch64"]
    command = [sys.executable, str(ROOT / "tools/ordinary_eevdf_capture.py"),
               str(capture), str(elf), revision]
    tools = {}
    for name, version in (("rustc", "-vV"), ("cargo", "--version"), ("aarch64-linux-gnu-nm", "--version"),
                          ("aarch64-linux-gnu-objdump", "--version")):
        path = shutil.which(name)
        if not path:
            raise FileNotFoundError(name)
        tools[name] = {"path": path, "sha256": sha(Path(path)),
                       "version": text([path, version], ROOT / "kernel", env)}
    if not shutil.which("qemu-system-aarch64"):
        raise FileNotFoundError("qemu-system-aarch64")
    (directory / "working-tree.patch").write_bytes(diff)
    provenance = dict(revision=revision, working_tree=changes, source_diff_sha256=hashlib.sha256(diff).hexdigest(),
                      environment={**PROFILE, "CARGO_TARGET_DIR": str(target)}, tools=tools,
                      python=sys.version, python_executable=sys.executable, build_argv=build,
                      cargo_argv=["cargo", "build", "--locked", "--release", "-j1", "--target", TARGET],
                      capture_argv=command, source_root=str(ROOT))
    with (directory / "build.log").open("xb") as log:
        status = subprocess.run(build, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT).returncode
    provenance.update(build_exit=status, build_log_sha256=sha(directory / "build.log"))
    if status == 0:
        provenance["elf_sha256"] = sha(elf)
    save(directory / "build.json", provenance)
    if status:
        raise RuntimeError(f"build exited {status}; see {directory / 'build.log'}")

    with (directory / "capture.log").open("xb") as log:
        capture_status = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT).returncode
    if not (capture / "session.json").is_file():
        raise RuntimeError(f"capture produced no session; see {directory / 'capture.log'}")
    digest = sha(capture / "session.json")
    with (directory / "session.sha256").open("x", encoding="ascii") as file:
        print(digest, file=file)
    code, record = score(directory, digest)
    if not code and (capture_status or record["result"]["elf_sha256"] != provenance["elf_sha256"]
                     or record["result"]["source"] != revision):
        code = 1
        record.update(verdict="INVALID", reason="capture exit or built image identity mismatch")
    record.update(capture_exit=capture_status, result_file=str(directory / "result.json"))
    save(directory / "result.json", record)
    print(json.dumps(record, indent=2))
    return code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    commands.add_parser("run").add_argument("--out", required=True, type=Path)
    offline = commands.add_parser("replay")
    offline.add_argument("--run", required=True, type=Path)
    offline.add_argument("--session-sha256", required=True)
    args = parser.parse_args()
    if args.action == "replay":
        code, record = score(args.run.resolve(), args.session_sha256)
        print(json.dumps(record, indent=2))
        return code
    try:
        return run(args.out.resolve())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as exc:
        print(f"ERROR {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
