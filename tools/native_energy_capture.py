"""Capture an externally hash-pinned #357 package or the original malformed CLI refusal."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from ordinary_amc_capture import save, sha
from pi4_native_smp_capture import capture

ROOT = Path(__file__).resolve().parents[1]
CASES = ("base", "thermal", "shared", "fallback", "empty", "deadline", "zero-mask")


def expected(case):
    profile = json.loads((ROOT / "examples/native-energy-placement/base.json").read_bytes())
    if case == "thermal":
        profile["limits"][0]["thermal_capacity"] = 0
    elif case == "shared":
        profile["limits"][0]["shared_load"] = 3
    elif case in ("fallback", "empty"):
        profile["fallback"] = profile["points"][1] if case == "fallback" else None
        profile["points"] = []
    elif case == "deadline":
        for limit in profile["limits"]:
            limit["thermal_capacity"] = 1
    elif case == "zero-mask":
        profile["allowed_cpus"] = 0
    elif case == "missing-limit":
        profile["limits"].pop()
    elif case != "base":
        raise ValueError("unknown placement case")
    return profile


def prepare(args, root, manifest):
    if sha(root / "manifest.json") != args.manifest_sha256:
        raise ValueError("prelaunch manifest digest mismatch")
    if (manifest["policy"], manifest["board"], len(manifest["slots"])) != ("energy-placement", "pi4", 1):
        raise ValueError("expected singleton modeled Pi 4 package")
    source, wire = manifest["placement_input"], manifest["placement_canonical"]
    if (source["file"], wire["file"]) != ("placement-source.json", "placement-v1.json"):
        raise ValueError("unexpected packaged profile paths")
    profile = json.loads((args.package / wire["file"]).read_bytes())
    if profile != expected(args.case) or manifest["slots"][0]["elf_sha256"] != args.elf_sha256:
        raise ValueError("case or prelaunch application identity mismatch")
    save(root / "position.json", dict(case=args.case, manifest_sha256=args.manifest_sha256,
                                      elf_sha256=args.elf_sha256, trace=args.trace))
    return {source["file"]: source["sha256"], wire["file"]: wire["sha256"]}


def refusal(args):
    root = args.out.resolve()
    root.mkdir()
    source = root / "placement-source.json"
    source.write_bytes(args.profile.read_bytes())
    elf = args.elf.resolve(strict=True)
    if json.loads(source.read_bytes()) != expected("missing-limit") or sha(elf) != args.elf_sha256:
        raise ValueError("incorrect refusal inputs")
    target = root / "package"
    command = [sys.executable, str(ROOT / "tools/build_native_el0.py"), "--board", "pi4",
               "--policy", "energy-placement", "--elf", str(elf), "--placement-profile",
               str(source), "--out", str(target)]
    result = subprocess.run(command, capture_output=True)
    (root / "stdout").write_bytes(result.stdout)
    (root / "stderr").write_bytes(result.stderr)
    save(root / "position.json", dict(case="missing-limit", command=command, status=result.returncode,
                                      elf_sha256=args.elf_sha256, package_created=target.exists()))
    save(root / "session.json", dict(reason="cli-return", files={p.name: sha(p) for p in root.iterdir() if p.is_file()}))
    print(sha(root / "session.json"))
    return int(result.returncode != 2 or target.exists() or b"limits requires exactly CPU0 and CPU1" not in result.stderr)


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    sub = cli.add_subparsers(dest="action", required=True)
    boot = sub.add_parser("boot")
    boot.add_argument("case", choices=CASES)
    boot.add_argument("package", type=Path)
    boot.add_argument("--manifest-sha256", required=True)
    boot.add_argument("--trace", action="store_true")
    boot.add_argument("--qemu", default="qemu-system-aarch64")
    boot.add_argument("--nm", default="aarch64-linux-gnu-nm")
    boot.add_argument("--objdump", default="aarch64-linux-gnu-objdump")
    bad = sub.add_parser("refusal")
    bad.add_argument("profile", type=Path)
    bad.add_argument("elf", type=Path)
    for parser in (boot, bad):
        parser.add_argument("out", type=Path)
        parser.add_argument("--elf-sha256", required=True)
    args = cli.parse_args()
    if args.action == "refusal":
        raise SystemExit(refusal(args))
    raise SystemExit(capture(args, lambda root, manifest: prepare(args, root, manifest)))
