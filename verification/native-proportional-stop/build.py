#!/usr/bin/env python3
"""Build a distinct site-17 diagnostic in a new detached worktree; retain all inputs."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
REL = Path("kernel/src/native_proportional.rs")
FIXTURE = Path("verification/native-proportional-stop/yield.c")
ORIGINAL = "        r.p.try_stop_elapsed(src, dt)"
ADAPTER = """        // Verification-only source tree: offer slot 1 the real elapsed interval
        // when slot 0 issues its ordinary EL0 yield. The policy retains owner 0.
        let caller = if src == 0 && kind == 2 && op == 8 && matches!(action, Action::Yield) {
            1
        } else {
            src
        };
        r.p.try_stop_elapsed(caller, dt)"""


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: build.py NEW-DIRECTORY-OUTSIDE-CHECKOUT")
    out = Path(sys.argv[1]).resolve()
    if out.is_relative_to(ROOT):
        sys.exit("use a fresh output directory outside the checkout")
    out.mkdir(parents=True, exist_ok=False)
    tree = out / "diagnostic-source"
    rev = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    subprocess.run(["git", "worktree", "add", "--detach", str(tree), rev], cwd=ROOT, check=True)
    source = tree / REL
    original_hash = digest(source)
    body = source.read_text(encoding="utf-8")
    if body.count(ORIGINAL) != 1:
        sys.exit("diagnostic adapter requires exactly one matching production close site")
    source.write_text(body.replace(ORIGINAL, ADAPTER), encoding="utf-8")
    patch = subprocess.check_output(["git", "diff", "--", REL.as_posix()], cwd=tree)
    (out / "diagnostic.patch").write_bytes(patch)
    elf = out / "yield.elf"
    subprocess.run(["sh", "sdk/native-aarch64/build.sh", str(tree / FIXTURE), str(elf)], cwd=tree, check=True)
    package = out / "guard-site17-only"
    subprocess.run([sys.executable, "tools/build_native_el0.py", "--elf", str(elf),
                    "--peer-elf", str(elf), "--out", str(package), "--policy", "stride",
                    "--weight", "1", "--peer-weight", "1", "--quantum-ticks", "625000"],
                   cwd=tree, check=True)
    label = dict(identity="verification-only-native-stride-site17-v1", revision=rev,
                 original_runtime=original_hash, diagnostic_runtime=digest(source),
                 patch=digest(out / "diagnostic.patch"), application=digest(elf),
                 kernel=digest(package / "kernel.elf"), manifest=digest(package / "manifest.json"))
    (out / "diagnostic.json").write_text(json.dumps(label, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
