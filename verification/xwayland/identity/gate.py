#!/usr/bin/env python3
"""Prepare and run the #464 gate in a fresh ext4 directory."""
import argparse
import ast
import configparser
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import fcntl

here = Path(__file__).resolve().parent
repo = here.parents[2]
def run(*cmd, **opts):
    print(shlex.join(map(str,cmd)), flush=True)
    return subprocess.run(list(map(str,cmd)), check=True, **opts)

p = argparse.ArgumentParser()
for key in ("lab", "out", "runtime", "xtrans-include", "x11-build"):
    p.add_argument("--"+key, type=Path, required=True)
a = p.parse_args()
out = a.out.resolve()
out.mkdir(parents=True, exist_ok=False)
lock = (a.lab / "run.lock").open("a")
fcntl.flock(lock, fcntl.LOCK_EX)
git = ["git","-C",repo]
rev = run(*git,"rev-parse","HEAD",capture_output=True,text=True).stdout.strip()
if run(*git,"status","--porcelain","--untracked-files=no",capture_output=True,text=True).stdout:
    sys.exit("use a clean checkout to identify the tested source")
(out/"source-commit").write_text(rev+"\n")
os.environ["CARGO_BUILD_JOBS"] = "1"

# Work on a private runtime and private X.Org includes.
rt = a.runtime.resolve()
prefix = out/"target"
shutil.copytree(rt/"target",prefix)
run(sys.executable,repo/"sdk/native-aarch64/graphics/os/install.py","--prefix",prefix)
overlay = prefix/"include/jnox-wayland"
inc = out/"xtrans-include"
shutil.copytree(a.xtrans_include/"X11/Xtrans",inc/"X11/Xtrans")
src = inc/"X11/Xtrans/Xtrans.c"
if hashlib.sha256(src.read_bytes()).hexdigest() != "bba1bbccec1ea44213cb67d2a77628ee1790c3fdbadbb755721927da52785f41":
    sys.exit("unexpected Xtrans.c input")
run("patch","--fuzz=0","-p1","-i",here/"xtrans-1.4.0-uname.patch",cwd=inc)
# The probe links the exact patched upstream function. The full LibX11
# translation unit is built separately below against the same include tree.
body = src.read_text().split("int TRANS(GetHostname) (char *buf, int maxlen)")
if len(body) != 2 or not body[1].strip().endswith("}"):
    sys.exit("unexpected Xtrans GetHostname source")
caller = out/"hostname.c"
caller.write_text("#include <string.h>\n#include <sys/utsname.h>\n#define NEED_UTSNAME 1\n"
                  "#define TRANS(func) _XimXTrans##func\nint TRANS(GetHostname) (char *buf, int maxlen)" + body[1])
conf = configparser.ConfigParser(interpolation=None)
conf.read(rt/"jnox.ini")
def setting(section,key):
    return ast.literal_eval(conf[section][key])
cc = setting("binaries","c")
flags = setting("built-in options","c_args")
ld = setting("built-in options","c_link_args")
ld = [str(prefix/"lib/libjnox-wayland-os.a") if Path(x).name=="libjnox-wayland-os.a" else x for x in ld]
if str(prefix/"lib/libjnox-wayland-os.a") not in ld:
    ld.insert(0,str(prefix/"lib/libjnox-wayland-os.a"))
elf = out/"identity.elf"
run(*cc, "-I"+str(overlay), *flags, here/"probe.c", caller, *ld, "-o",elf)
for index,label in enumerate(("", "a"*64, "-leading", "trailing-")):
    bad = out/("invalid-"+str(index))
    result = subprocess.run([sys.executable,str(repo/"tools/build_native_el0.py"),"--board","virt",
                             "--policy","fifo","--elf",str(elf),"--priority","4","--nodename="+label,
                             "--out",str(bad)],capture_output=True,text=True)
    (out/("invalid-"+str(index)+".log")).write_text(result.stderr)
    if result.returncode != 2 or "--nodename requires" not in result.stderr or bad.exists():
        sys.exit("invalid label accepted or failed for an unrelated reason")
for case in ("guest-a","guest-b","absent"):
    pkg = out/case
    args = ["--board","virt","--policy","fifo","--elf",elf,"--priority","4","--arg",case,
            "--peer-elf",elf,"--peer-priority","4","--peer-arg",case,"--out",pkg]
    if case != "absent": args += ["--nodename",case]
    run(sys.executable,repo/"tools/build_native_el0.py",*args)
    meta = json.loads((pkg/"manifest.json").read_text())
    if meta["commit"] != rev or meta["tracked_changes"] or meta.get("nodename") != (None if case=="absent" else case):
        sys.exit("package metadata mismatch")
    uart = pkg/"uart.log"
    run(sys.executable,repo/"examples/native-pixman/capture.py",pkg/"kernel.elf",uart,2,
        "--symbol","_XimXTransGetHostname","--symbol","uname","--symbol","gethostname")
    exits = sorted(re.findall(rb"^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$",uart.read_bytes(),re.M))
    if exits != [(b"0",b"0"),(b"1",b"0")]: sys.exit("unexpected native exit statuses")
    symbols = json.loads(uart.with_suffix(".symbols.json").read_text())
    lines = uart.with_suffix(".trace").read_text().splitlines()
    hits = set()
    for i,line in enumerate(lines):
        entry = re.match(r"Trace 0: 0x[0-9a-f]+ \[[0-9a-f]+/([0-9a-f]+)/",line,re.I)
        if entry and not (i+1 < len(lines) and lines[i+1].startswith("Stopped execution of TB chain before")):
            hits.add(int(entry[1],16))
    if any(pc not in hits for pc,_ in symbols.values()): sys.exit("missing original function entry")

# Preserve the configured build and compiler command. Change only include
# precedence when compiling the same genuine xim_trans.c and remaining X11.
x11 = out/"X11-build"
shutil.copytree(a.x11_build,x11,symlinks=True)
mk = x11/"src/xlibi18n/Makefile"
original = re.search(r"^CC = (.+)$",mk.read_text(),re.M)
if not original: sys.exit("missing configured libX11 compiler")
wrapper = out/"x11-cc"
wrapper.write_text("#!/bin/sh\nexec "+original[1]+" -I"+shlex.quote(str(inc))+" -I"+
                   shlex.quote(str(overlay))+" \"$@\"\n")
wrapper.chmod(0o755)
run("make","-j1","V=1","CC="+str(wrapper),"xim_trans.lo",cwd=mk.parent)
run("make","-j1","V=1","CC="+str(wrapper),cwd=x11)
