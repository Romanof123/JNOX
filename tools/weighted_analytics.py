#!/usr/bin/env python3
"""Build and boot the named CPU0 weighted analytics deployment on Linux/WSL."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PROFILE = Path(__file__).with_name("weighted_analytics_profile.json")
INPUTS = ["sdk/native-aarch64", "examples/native-weighted", "examples/native-weighted-rust",
          "examples/native-weighted-analytics", "examples/native-weighted-analytics-rust"]
SLOT_FILES = ("user.elf", "native_peer.elf", "native_peer2.elf")
QEMU_FLAGS = ["-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
              "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none",
              "-monitor", "none", "-serial", "stdio", "-no-reboot"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def output(command, cwd=ROOT, env=None):
    return subprocess.check_output(command, cwd=cwd, env=env, text=True).strip()


def run_logged(command, cwd, env, log):
    line = shlex.join(list(map(str, command)))
    print(line, flush=True)
    print(line, file=log, flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        log.flush()
        with open(log.name, "rb") as saved:
            saved.seek(max(0, saved.seek(0, os.SEEK_END) - 8192))
            sys.stderr.write(saved.read().decode("utf-8", errors="replace"))
        if "--out" in command:
            inner = Path(command[command.index("--out") + 1]) / "build.log"
            if inner.is_file():
                with inner.open("rb") as saved:
                    saved.seek(max(0, saved.seek(0, os.SEEK_END) - 8192))
                    sys.stderr.write(saved.read().decode("utf-8", errors="replace"))
        result.check_returncode()


def snapshot(work):
    names = output(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard",
                    "--", *INPUTS])
    hashes = {}
    for name in names.split("\0"):
        if not name:
            continue
        source = ROOT / name
        dest = work / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, dest)
        hashes[name] = digest(dest)
    return hashes


def check_package(bundle, profile, name, record):
    package = bundle / name
    package_hash = record["packages"][name]
    require(digest(package / "manifest.json") == package_hash, f"{name}: manifest digest mismatch")
    manifest = read(package / "manifest.json")
    for key, expected in {"app": "process", "board": "virt", "profile": "native", "policy": "weighted",
                          "commit": record["commit"], "quantum_ticks": profile["quantum_ticks"]}.items():
        require(manifest[key] == expected, f"{name}: unexpected {key}")
    clients = profile["variants"][name]
    require(len(manifest["slots"]) == len(clients), f"{name}: unexpected slot count")
    for index, (client, weight) in enumerate(clients):
        slot = manifest["slots"][index]
        expected = record["clients"][client]
        elf_name = SLOT_FILES[index]
        require(slot["slot"] == index and slot["weight"] == weight and slot["elf"] == elf_name,
                f"{name}: slot {index} configuration mismatch")
        require(slot["input"]["kind"] == "elf" and slot["input"]["sha256"] == expected
                and slot["elf_sha256"] == expected and digest(package / elf_name) == expected
                and digest(bundle / "clients" / (client + ".elf")) == expected,
                f"{name}: wrong {client} ELF")
    require(digest(package / "kernel.elf") == manifest["kernel_elf_sha256"], f"{name}: wrong kernel")
    require(digest(package / "build.log") == manifest["build_log_sha256"], f"{name}: wrong build log")
    return package / "kernel.elf"


def build(out, profile):
    if os.path.lexists(out):
        raise FileExistsError(f"choose a new output directory: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith(("JNOX_", "CARGO_FEATURE_"))
           and k not in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET")}
    env.setdefault("RUSTUP_TOOLCHAIN", "1.95.0")
    with tempfile.TemporaryDirectory(prefix=".weighted-analytics-", dir=out.parent) as stage:
        stage = Path(stage)
        work, bundle = stage / "sources", stage / "deployment"
        (bundle / "clients").mkdir(parents=True)
        env["CARGO_TARGET_DIR"] = str(stage / "cargo")
        record = {
            "profile_sha256": digest(PROFILE),
            "commit": output(["git", "rev-parse", "HEAD"]),
            "tracked_changes": output(["git", "status", "--porcelain", "--untracked-files=no"]),
            "inputs": snapshot(work), "clients": {}, "packages": {},
            "tools": {tool: output([tool, "--version"], env=env)
                      for tool in ("clang", "ld.lld", "rustc", "cargo")},
        }
        shutil.copyfile(PROFILE, bundle / "profile.json")
        with (bundle / "build.log").open("w", encoding="utf-8") as log:
            for client, source in profile["sources"].items():
                elf = bundle / "clients" / (client + ".elf")
                script = "build-rust.sh" if source.endswith("Cargo.toml") else "build.sh"
                run_logged(["sh", "-x", "sdk/native-aarch64/" + script, source, elf], work, env, log)
                record["clients"][client] = digest(elf)
            for name, clients in profile["variants"].items():
                package = bundle / name
                command = [sys.executable, str(ROOT / "tools/build_native_el0.py"), "--out", str(package),
                           "--policy", "weighted", "--quantum-ticks", str(profile["quantum_ticks"])]
                for index, (client, weight) in enumerate(clients):
                    prefix = "--peer-" if index else "--"
                    command += [prefix + "elf", str(bundle / "clients" / (client + ".elf")),
                                prefix + "weight", str(weight)]
                run_logged(command, ROOT, env, log)
                record["packages"][name] = digest(package / "manifest.json")
                check_package(bundle, profile, name, record)
                shutil.rmtree(package / "target")
        save(bundle / "build.json", record)
        (bundle / "build.sha256").write_text(digest(bundle / "build.json") + "\n", encoding="ascii")
        # Reserve a fresh destination even if another build finished during compilation.
        out.mkdir()
        try:
            bundle.rename(out / "deployment")
        except OSError:
            out.rmdir()
            raise
    print(out / "deployment" / "build.json")


def boot(out, profile, name):
    bundle = out / "deployment"
    require(digest(bundle / "build.json") == (bundle / "build.sha256").read_text().strip(),
            "build record digest mismatch")
    record = read(bundle / "build.json")
    require(digest(bundle / "profile.json") == record["profile_sha256"] == digest(PROFILE),
            "profile differs from this operator configuration")
    kernel = check_package(bundle, profile, name, record)
    qemu = shutil.which("qemu-system-aarch64")
    if not qemu:
        raise FileNotFoundError("qemu-system-aarch64 is required")
    command = [qemu, *QEMU_FLAGS, "-kernel", str(kernel)]
    launches = out / "launches"
    launches.mkdir(exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=launches, prefix=name + "-", suffix=".json",
                                     delete=False, encoding="utf-8") as log:
        json.dump({"variant": name, "kernel_sha256": digest(kernel), "argv": command,
                   "qemu_version": output([qemu, "--version"]), "qemu_sha256": digest(Path(qemu))},
                  log, indent=2)
        print("Launch record:", log.name, file=sys.stderr, flush=True)
    os.execv(qemu, command)


def main():
    profile = read(PROFILE)
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    for action in ["build", "boot"]:
        sub = actions.add_parser(action)
        sub.add_argument("--out", required=True, type=Path)
        if action == "boot":
            sub.add_argument("--variant", required=True, choices=profile["variants"])
    args = parser.parse_args()
    out = args.out.absolute()
    if args.action == "build":
        build(out, profile)
    else:
        boot(out, profile, args.variant)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        sys.exit(f"weighted-analytics: {error}")
