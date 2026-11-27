"""Check every libX11.a member and execute a public Xlib function in native EL0."""
import argparse
import ast
import configparser
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

here = Path(__file__).resolve().parent
p = argparse.ArgumentParser(description=__doc__)
for option in ("build", "cross", "protocol-prefix", "out", "lock"):
    p.add_argument("--" + option, type=Path, required=True)
p.add_argument("--repo", type=Path, default=here.parents[2])
a = p.parse_args()
out, build, repo = a.out.resolve(), a.build.resolve(), a.repo.resolve()
archive = build / "src/.libs/libX11.a"

with a.lock.open("a") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    out.mkdir()
    os.environ.update(CARGO_BUILD_JOBS="1", CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(out / "cargo-target"))
    def run(*args, **kw):
        return subprocess.run(args, check=True, cwd=repo, **kw)
    members = run("llvm-ar", "t", archive, capture_output=True, text=True).stdout.splitlines()
    desc = run("llvm-readelf", "-h", archive, capture_output=True, text=True).stdout
    machines = re.findall(r"^\s*Machine:\s*(\S+)", desc, re.M)
    if len(machines) != len(members) or not machines or set(machines) != {"AArch64"}:
        sys.exit("archive must contain only AArch64 object files")
    source = re.search(r"^srcdir = (.+)$", (build / "src/Makefile").read_text(), re.M)[1]
    cfg = configparser.ConfigParser(interpolation=None)
    cfg.read(a.cross)
    cc = ast.literal_eval(cfg["binaries"]["c"])
    opt = cfg["built-in options"]
    elf, obj = out / "geometry.elf", out / "geometry.o"
    run(*cc, *ast.literal_eval(opt["c_args"]), "-I" + str(build / "include"),
        "-I" + str(Path(source).parent / "include"), "-I" + str(a.protocol_prefix.resolve() / "include"),
        "-c", here / "xlib-geometry.c", "-o", obj)
    run(*cc, obj, archive, *ast.literal_eval(opt["c_link_args"]), "-o", elf)
    pkg = out / "package"
    run(sys.executable, "tools/build_native_el0.py", "--board", "virt", "--policy", "fifo",
        "--priority", "4", "--elf", elf, "--out", pkg)
    uart = out / "app.log"
    run(sys.executable, "examples/native-pixman/capture.py", pkg / "kernel.elf", uart, "1",
        "--symbol", "XParseGeometry")
    if re.findall(rb"^NATIVE EL0 EXIT (-?\d+)\r?$", uart.read_bytes(), re.M) != [b"0"]:
        sys.exit("Xlib geometry guest failed")
    result = {"archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
              "members": len(members), "machine": "AArch64", "geometry_guest_exit": 0}
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
