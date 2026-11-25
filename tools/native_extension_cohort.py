#!/usr/bin/env python3
"""Build one #349 source cohort. Linux; pins.json maps cargo/rustc/clang/rust-lld to {path,sha256}."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TARGET = "aarch64-unknown-none"
APPS = ("user.elf", "native_peer.elf", "native_peer2.elf")
HASHES = ("5a2e06c7865ea29f88e0856803e9a75be29188471f878157992170135e81f1f5",
          "5a5e88cf7c66ec8cc65842220e03285a42976483925f8367a8357a13743e8814",
          "28cbf4b5d4842c3b2b3e29ed68ac57fb6965358bb8c0dea7769516e82c745210")
RULE = b"ready 2 1 2\npropose 2 17 0 1\nyield\n"


def digest(path):
    with path.open("rb") as f:
        h = hashlib.sha256()
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def require(ok, message):
    if not ok:
        raise ValueError(message)


def save(path, data):
    with path.open("x", encoding="utf-8") as f:
        json.dump(data, f, indent=2, sort_keys=True)
        f.write("\n")


def run_logged(argv, stem, **kwargs):
    save(stem.with_suffix(".command.json"), argv)
    with stem.with_suffix(".stdout").open("xb") as out, stem.with_suffix(".stderr").open("xb") as err:
        result = subprocess.run(argv, stdout=out, stderr=err, **kwargs)
    save(stem.with_suffix(".exit.json"), result.returncode)
    result.check_returncode()


def build(args, root):
    pins = json.loads(args.pins.read_text())
    require(set(pins) == {"cargo", "rustc", "rust-lld", "clang"}, "four tool pins required")
    bindir = root/"bin"
    bindir.mkdir()
    for name, pin in pins.items():
        path = Path(pin["path"]).resolve(strict=True)
        require(path.name != "rustup" and digest(path) == pin["sha256"],
                f"{name}: supply pinned actual tool executable")
        pin["path"] = str(path)
        if name != "rust-lld":
            (bindir/name).symlink_to(path)
    # rustc recognizes this basename as its GNU-flavor LLD driver.
    wrapper = ROOT/"tools/native_extension_cohort_link.py"
    (bindir/"rust-lld").symlink_to(wrapper)
    ref = args.accepted.resolve(strict=True)
    require([digest(ref/name) for name in APPS] == list(HASHES), "integrated accepted application bytes")
    rule = ref/"extension_program.txt"
    require(rule.read_bytes() == RULE, "integrated accepted rule")
    spec = dict(position=args.position, tools=pins, root=str(root), wrapper_sha256=digest(wrapper),
                driver_sha256=digest(Path(__file__)), source=str(ROOT),
                entry_source_sha256=digest(ROOT/"kernel/src/native_extension_entry.S"))
    save(root/"spec.json", spec)
    order = (0, 1, 2) if args.position == "a" else (2, 1, 0)
    cmd = [sys.executable, str(ROOT/"tools/build_native_el0.py"), "--out", str(root/"ordinary"),
           "--board", "virt", "--policy", "extension", "--extension-program", str(rule),
           "--elf", str(ref/APPS[order[0]]), "--peer-elf", str(ref/APPS[order[1]]),
           "--peer-elf", str(ref/APPS[order[2]])]
    if args.position == "b":
        cmd += ["--arg", "four-batches"]
    env = {k: v for k, v in os.environ.items() if not k.startswith(("CARGO_", "RUST", "JNOX_", "COHORT349_"))}
    env.update(PATH=str(bindir) + os.pathsep + env["PATH"], LC_ALL="C", RUSTC=pins["rustc"]["path"],
               CARGO_TARGET_AARCH64_UNKNOWN_NONE_LINKER=str(bindir/"rust-lld"), COHORT349_ROOT=str(root))
    run_logged(cmd, root/"packager", cwd=ROOT, env=env)
    ordinary = root/"ordinary"
    template = json.loads((ordinary/"manifest.json").read_text())
    require([s["elf_sha256"] for s in template["slots"]] == [HASHES[i] for i in order]
            and template["kernel_elf_sha256"] == digest(ordinary/"kernel.elf")
            and not template.get("tracked_changes"), "clean ordinary package identity")
    manifests = {}
    for variant in ("ordinary", "dormant", "active"):
        link = json.loads((root/"link"/f"{variant}.json").read_text())
        pkg = root/variant
        if variant != "ordinary":
            pkg.mkdir()
            for name in (*APPS, "extension_program.txt"):
                shutil.copyfile(ordinary/name, pkg/name)
            shutil.copyfile(root/"link"/f"{variant}.elf", pkg/"kernel.elf")
            with (pkg/"build.log").open("xb") as f:
                for source in (ordinary/"build.log", root/"link"/f"{variant}.json"):
                    with source.open("rb") as inp:
                        shutil.copyfileobj(inp, f)
            cells = ([] if variant == "dormant" else [1, 3] if args.position == "a" else [9])
            save(pkg/"manifest.json", dict(template, kernel_elf_sha256=link["output_sha256"],
                 build_log_sha256=digest(pkg/"build.log"),
                 extension_stop_bench=dict(position=args.position+"-"+variant, cells=cells)))
        require(digest(pkg/"kernel.elf") == link["output_sha256"], "packaged link result")
        manifests[variant] = digest(pkg/"manifest.json")
    save(root/"cohort.json", dict(spec_sha256=digest(root/"spec.json"), manifests=manifests,
                                  inputs_sha256=digest(root/"link/inputs.json"),
                                  links={v: digest(root/"link"/f"{v}.json") for v in manifests}))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("position", choices=("a", "b"))
    p.add_argument("accepted", type=Path)
    p.add_argument("out", type=Path)
    p.add_argument("--pins", required=True, type=Path)
    args = p.parse_args()
    root = args.out.absolute()
    require(root.parent.resolve(strict=True) == root.parent, "fresh directory under physical parent required")
    root.mkdir(mode=0o700)
    try:
        build(args, root)
    except Exception as exc:
        save(root/"failure.json", dict(type=type(exc).__name__, message=str(exc)))
        raise


if __name__ == "__main__":
    main()
