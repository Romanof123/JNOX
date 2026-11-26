#!/usr/bin/env python3
"""Build an unchanged, pinned Linux driver as a native JNOX EL0 service."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
COMPAT = ROOT / "drivers/linux-compat"
SDK = ROOT / "sdk/native-aarch64"
TARGET = "aarch64-none-elf"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def pinned_source(path, upstream):
    data = path.read_bytes()
    git_id = hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()
    if (len(data), sha(data), git_id) != (
        upstream["size"], upstream["sha256"], upstream["git_blob_sha1"]
    ):
        raise ValueError("source length, SHA-256 or Git blob ID differs from the selected profile")
    if data.splitlines()[0] != f'// SPDX-License-Identifier: {upstream["spdx"]}'.encode():
        raise ValueError("SPDX declaration differs from the selected profile")
    return data


def run(argv, cwd):
    return subprocess.check_output(list(map(str, argv)), cwd=cwd, text=True,
                                   env={**os.environ, "LC_ALL": "C"})


def inspect_elf(path, tools):
    def inspect(*args):
        return run([tools["readelf"], "--wide", *args, path.name], path.parent)

    header = inspect("--file-header")
    sections = inspect("--section-headers")
    symbols = inspect("--symbols")
    entries = []
    for line in symbols.splitlines():
        fields = line.split(None, 7)
        if len(fields) == 8 and re.fullmatch(r"\d+:", fields[0]):
            entries.append(dict(value=fields[1], size=int(fields[2]), type=fields[3],
                                bind=fields[4], visibility=fields[5], index=fields[6], name=fields[7]))
    return {
        "filename": path.name, "sha256": sha(path.read_bytes()), "size": path.stat().st_size,
        "header": header, "sections": sections, "program_headers": inspect("--program-headers"),
        "symbols": entries, "undefined": sorted({s["name"] for s in entries if s["index"] == "UND"}),
        "nm": run([tools["nm"], "-P", path.name], path.parent),
    }


def verify_elf(report, elf_type, profile):
    header = report["header"]
    if not (re.search(r"Machine:\s+AArch64", header) and
            re.search(r"Class:\s+ELF64", header) and
            re.search(r"Type:\s+" + elf_type + r"\b", header)):
        raise ValueError(f"{report['filename']} has the wrong ELF architecture or type")
    functions = {s["name"] for s in report["symbols"] if s["type"] == "FUNC" and s["index"] != "UND"}
    missing = set(profile["required_driver_functions"]) - functions
    if missing:
        raise ValueError(f"{report['filename']} missing original driver callbacks: {sorted(missing)}")
    if elf_type == "REL":
        unknown = set(report["undefined"]) - set(profile["allowed_driver_imports"])
        if unknown:
            raise ValueError(f"unsupported Linux imports: {sorted(unknown)}")
    else:
        if report["undefined"] or re.search(r"^\s*(?:INTERP|DYNAMIC)\s", report["program_headers"], re.M):
            raise ValueError("service has unresolved imports or requires a dynamic loader")
        if ".note.jnox.abi" not in report["sections"] or "_start" not in functions:
            raise ValueError("service is missing the native JNOX ABI note or entry")


def build(args, profile_path, profile, data, work):
    tools = {name: shutil.which(getattr(args, name)) for name in ("clang", "ld", "readelf", "nm")}
    if not all(tools.values()):
        raise ValueError(f"missing build tools: {[key for key, path in tools.items() if not path]}")
    commands = []

    def invoke(argv):
        commands.append(list(map(str, argv)))
        run(argv, work)

    flags = ["--target=" + TARGET, "-std=gnu11", "-O2", "-ffreestanding", "-nostdinc",
             "-fno-pic", "-fno-pie", "-fno-stack-protector", "-fno-unwind-tables",
             "-fno-asynchronous-unwind-tables", "-mgeneral-regs-only", "-mno-outline-atomics",
             "-Werror=implicit-function-declaration"]
    includes = ["-I", COMPAT / "include", "-I", SDK / "include"]
    # Compile precisely the verified byte buffer, even if the external file changes later.
    name = Path(profile["upstream"]["path"]).name
    original = work / name
    original.write_bytes(data)
    obj = Path(name).with_suffix(".o").name
    invoke([tools["clang"], *flags, *includes, "-c", name, "-o", obj])
    original.unlink()  # The intake remains external; publish only build products.
    driver = inspect_elf(work / obj, tools)
    verify_elf(driver, "REL", profile)

    objects = [obj]
    inputs = {}
    for rel in [*profile["runtime_sources"], profile["service_source"], "sdk/native-aarch64/io.c",
                "sdk/native-aarch64/start.S"]:
        source = ROOT / rel
        target = rel.replace("/", "_") + ".o"
        inputs[rel] = sha(source.read_bytes())
        search = ["-I", SDK / "include"] if source.parent == SDK else includes
        invoke([tools["clang"], *flags, *search, "-c", source, "-o", target])
        objects.append(target)
    script = SDK / "app.ld"
    invoke([tools["ld"], "-m", "aarch64elf", "-static", "--no-undefined", "--build-id=none",
            "-z", "max-page-size=4096", "-T", script, *objects, "-o", "service.elf"])
    service = inspect_elf(work / "service.elf", tools)
    verify_elf(service, "EXEC", profile)
    for path in [script, profile_path, *sorted((COMPAT / "include").rglob("*.h")),
                 *sorted((SDK / "include").rglob("*.h"))]:
        inputs[path.relative_to(ROOT).as_posix()] = sha(path.read_bytes())
    manifest = {
        "schema": 1, "profile": profile["name"], "target": TARGET,
        "artifact_type": "static native JNOX EL0 service", "upstream": profile["upstream"],
        "source": {"path": str(args.source.resolve()), "translation_unit": name, "size": len(data),
                   "sha256": sha(data), "git_blob_sha1": profile["upstream"]["git_blob_sha1"]},
        "driver_object": driver, "service": service, "profile_inputs_sha256": inputs,
        "commands": commands,
        "tools": {key: {"path": path, "version": run([path, "--version"], work).splitlines()[0]}
                  for key, path in tools.items()},
    }
    (work / "import.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    for intermediate in objects[1:]:
        (work / intermediate).unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    profiles = {path.stem: path for path in (COMPAT / "profiles").glob("*.json")}
    parser.add_argument("--profile", choices=sorted(profiles), required=True)
    parser.add_argument("--source", required=True, type=Path, help="external upstream C file")
    parser.add_argument("--out", required=True, type=Path, help="new output directory")
    for key, default in [("clang", "clang"), ("ld", "ld.lld"), ("readelf", "readelf"), ("nm", "nm")]:
        parser.add_argument("--" + key, default=default)
    args = parser.parse_args()
    profile_path = profiles[args.profile]
    profile = json.loads(profile_path.read_text(encoding="utf-8"))
    data = pinned_source(args.source, profile["upstream"])
    out = args.out.resolve()
    if out.exists():
        parser.error("--out must name a new directory")
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=out.parent) as stage:
        work = Path(stage) / "build"
        work.mkdir()
        build(args, profile_path, profile, data, work)
        work.rename(out)
    print(out / "import.json")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        raise SystemExit(str(exc)) from exc
