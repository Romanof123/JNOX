#!/usr/bin/env python3
"""Package up to three native AArch64 applications into a virt kernel."""
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


class SinglePath(argparse.Action):
    def __call__(self, parser, namespace, value, option_string=None):
        if getattr(namespace, self.dest) is not None:
            parser.error(f"{option_string} may appear only once")
        setattr(namespace, self.dest, value)


def edf_job(value):
    fields = value.split(",")
    if len(fields) != 3 or any(not (field.isascii() and field.isdecimal()) for field in fields):
        raise argparse.ArgumentTypeError("expected release,deadline,cost in unsigned decimal grid units")
    release, deadline, cost = map(int, fields)
    if not (release < deadline <= 16 and 0 < cost < 2**64):
        raise argparse.ArgumentTypeError("require 0 <= release < deadline <= 16 and positive u64 cost")
    return release, deadline, cost


def native_schedule(parser, args):
    if len(args.peer_elf) > 2:
        parser.error("virt supports at most two --peer-elf arguments")
    if (args.peer_elf or args.policy) and not (args.source or args.elf):
        parser.error("peers and scheduling policies require an ordinary --source or --elf")
    for name in ("policy", "priority", "weight", "job", "quantum_ticks"):
        values = getattr(args, name)
        if len(values) > 1:
            parser.error(f"--{name.replace('_', '-')} may appear only once")
    policy = args.policy[0] if args.policy else None
    if policy == "edf":
        if not args.job or len(args.peer_job) != len(args.peer_elf):
            parser.error("EDF requires exactly one job tuple per supplied ELF")
    elif args.job or args.peer_job:
        parser.error("job tuples require --policy edf")
    if policy in ("fifo", "rr"):
        if not args.priority or len(args.peer_priority) != len(args.peer_elf):
            parser.error("fixed scheduling requires exactly one priority per supplied ELF")
    elif args.priority or args.peer_priority:
        parser.error("application priorities require --policy fifo or rr")
    priorities = args.priority + args.peer_priority
    if any(value < 0 or value > 255 for value in priorities):
        parser.error("priorities must be in 0..255")
    weights = args.weight + args.peer_weight
    if policy == "weighted":
        if not args.weight or len(args.peer_weight) != len(args.peer_elf):
            parser.error("weighted scheduling requires exactly one weight per supplied ELF")
        if any(not 1 <= value <= 2**32 - 1 for value in weights):
            parser.error("weights must be in 1..2^32-1")
    elif weights:
        parser.error("application weights require --policy weighted")
    if bool(args.quantum_ticks) != (policy in ("rr", "weighted")):
        parser.error("--quantum-ticks is required exclusively with --policy rr or weighted")
    quantum = args.quantum_ticks[0] if args.quantum_ticks else None
    if quantum is not None and not 1 <= quantum <= 2**63 - 1:
        parser.error("--quantum-ticks must be in 1..2^63-1")
    return policy, priorities, weights, quantum


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, action=SinglePath, help="new package directory")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--source", type=Path, action=SinglePath, help="C application to compile with the native SDK")
    group.add_argument("--elf", type=Path, action=SinglePath, help="already linked native AArch64 application")
    parser.add_argument("--peer-elf", type=Path, action="append", default=[],
                        help="independently linked peer in slot order (up to two)")
    parser.add_argument("--policy", choices=["fifo", "rr", "weighted", "edf"], action="append", default=[])
    parser.add_argument("--priority", type=int, action="append", default=[], help="primary priority")
    parser.add_argument("--peer-priority", type=int, action="append", default=[], help="in peer order")
    parser.add_argument("--weight", type=int, action="append", default=[], help="primary CPU service weight")
    parser.add_argument("--peer-weight", type=int, action="append", default=[], help="CPU service weights in peer order")
    parser.add_argument("--quantum-ticks", type=int, action="append", default=[], help="common RR or weighted grant in physical counter ticks")
    parser.add_argument("--job", type=edf_job, action="append", default=[], metavar="R,D,C",
                        help="EDF primary release, absolute deadline and cost in 100 ms units")
    parser.add_argument("--peer-job", type=edf_job, action="append", default=[], metavar="R,D,C",
                        help="EDF job tuple for each peer, in peer order")
    args = parser.parse_args()
    policy, priorities, weights, quantum = native_schedule(parser, args)
    peer_inputs = [path.resolve(strict=True) for path in args.peer_elf]
    peer_digests = [sha256(path) for path in peer_inputs]
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
        digest = sha256(user_elf)
        slots = [{"slot": 0, "elf": user_elf.name, "elf_sha256": digest, "input": metadata["input"]}]
        for slot, (peer_input, peer_digest) in enumerate(zip(peer_inputs, peer_digests), 1):
            suffix = "" if slot == 1 else str(slot)
            peer_elf = out / f"native_peer{suffix}.elf"
            shutil.copyfile(peer_input, peer_elf)
            if sha256(peer_elf) != peer_digest:
                raise ValueError(f"peer ELF in slot {slot} changed during packaging")
            peer_name = str(peer_input.relative_to(ROOT) if peer_input.is_relative_to(ROOT) else peer_input)
            inputs[peer_name] = peer_digest
            peer = {"kind": "elf", "path": peer_name, "sha256": peer_digest}
            slots.append({"slot": slot, "elf": peer_elf.name, "elf_sha256": peer_digest, "input": peer})
            metadata[f"peer{suffix}_input"] = peer
            metadata[f"peer{suffix}_elf_sha256"] = peer_digest
            var = f"JNOX_NATIVE_PEER{suffix}_ELF"
            env[var], env[var + "_SHA256"] = str(peer_elf), peer_digest
        jobs = args.job + args.peer_job
        for slot in slots:
            slot["priority"] = priorities[slot["slot"]] if priorities else None
            if weights:
                slot["weight"] = weights[slot["slot"]]
            if jobs:
                release, deadline, cost = jobs[slot["slot"]]
                slot["job"] = {"release": release, "deadline": deadline, "cost": cost}
        if jobs:
            metadata["edf_profile"] = {"name": "finite", "grid_ms": 100, "horizon_grids": 16}
            env["JNOX_NATIVE_EDF_JOBS"] = ";".join(",".join(map(str, job)) for job in jobs)
        metadata.update(policy=policy or "cooperative", quantum_ticks=quantum, slots=slots)
        if policy:
            env["JNOX_NATIVE_POLICY"] = policy
        if priorities:
            env["JNOX_NATIVE_PRIORITIES"] = ",".join(map(str, priorities))
        if weights:
            env["JNOX_NATIVE_WEIGHTS"] = ",".join(map(str, weights))
        if quantum is not None:
            env["JNOX_NATIVE_QUANTUM_TICKS"] = str(quantum)
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
    for slot in slots[1:]:
        print(f"peer {slot['elf_sha256']}  {out / slot['elf']}")


if __name__ == "__main__":
    main()
