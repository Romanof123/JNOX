#!/usr/bin/env python3
"""Verify the merged ioctl adapter and resume the pinned AF_UNIX libX11 build."""
import argparse
import ast
import configparser
import fcntl
import hashlib
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys

here = Path(__file__).resolve().parent
repo = here.parents[2]
p = argparse.ArgumentParser()
for key in ("out","lab","runtime","identity-target","protocol-prefix","unix-build","xlib-tar"):
    p.add_argument("--"+key, type=Path, required=True)
a = p.parse_args()
out = a.out.resolve()
out.mkdir()
lock = (a.lab/"run.lock").open("a")
fcntl.flock(lock,fcntl.LOCK_EX)
def run(*args,**kwargs):
    print(shlex.join(map(str,args)),flush=True)
    return subprocess.run(list(map(str,args)),check=True,**kwargs)
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
old = a.identity_target/"lib/libjnox-wayland-os.a"
if sha(old) != "d400169bfa279dad70e7f473a989c2dd3ef413804f59c2a4aa745160beebcb75":
    p.error("expected tested #464 adapter")
prefix = out/"target"
shutil.copytree(a.identity_target,prefix)
run(sys.executable,repo/"sdk/native-aarch64/graphics/os/install.py","--prefix",prefix)
adapter = prefix/"lib/libjnox-wayland-os.a"
(out/"adapter.sha256").write_text(sha(adapter)+"  libjnox-wayland-os.a\n")
# Include the authentic SDK IPv4 layout consumed by Xtransint.h.
for name in ("netinet/in.h","arpa/inet.h"):
    dest = prefix/"include/jnox-wayland"/name
    dest.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(repo/"sdk/native-aarch64/include"/name,dest)
cross = configparser.ConfigParser(interpolation=None)
cross.read(a.runtime/"jnox.ini")
opt = cross["built-in options"]
cargs = ast.literal_eval(opt["c_args"])
ld = ast.literal_eval(opt["c_link_args"])
if not any(Path(x).name == adapter.name for x in ld): p.error("missing transport library in source cross file")
opt["c_args"] = repr(["-I"+str(prefix/"include/jnox-wayland"),*cargs])
opt["c_link_args"] = repr([str(adapter) if Path(x).name==adapter.name else x for x in ld])
with (out/"cross.ini").open("w") as f: cross.write(f)
os.environ["CARGO_BUILD_JOBS"] = "1"
run("rustc","--edition","2021","--test",repo/"services/socket/src/local.rs","-o",out/"broker-tests")
run(out/"broker-tests","--test-threads=1")
run(sys.executable,here/"package.py",out/"cross.ini",a.protocol_prefix,out/"guest")
pkg=out/"guest/package"
uart=out/"guest/uart.log"
symbols = ("_X11TransSocketBytesReadable", "_X11TransSocketReadv", "_X11TransSocketWritev", "ioctl", "readv", "writev")
filters = [arg for symbol in symbols for arg in ("--symbol", symbol)]
run(sys.executable,repo/"examples/native-pixman/capture.py",pkg/"kernel.elf",uart,2,*filters)
if sorted(re.findall(rb"^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$",uart.read_bytes(),re.M)) != [(b"0",b"0"),(b"1",b"0")]:
    sys.exit("unexpected guest status")
# Keep every prior ELF, config and intermediate object intact.
build=out/"X11-build"
build.mkdir()
configured=(a.unix_build/"src/xlibi18n/Makefile").read_text()
original=re.search(r"^CC = (.+)$",configured,re.M)[1]
# The previously configured CC is the Unix-only Xtrans wrapper. Its executable
# contains the real compiler with our last overlay first. Rebuild that wrapper
# with this adapter first, retaining the Unix-only private Xtrans source tree.
old_command = Path(original).read_text()
if old_command.count(" \"$@\"") != 1: p.error("unexpected #464 wrapper")
cc=out/"x11-cc"
args=shlex.split(old_command.splitlines()[1].removeprefix("exec "))
cc.write_text("#!/bin/sh\nexec "+shlex.join([args[0],"-I"+str(prefix/"include/jnox-wayland"),*args[1:-1]])+' "$@"\n')
cc.chmod(0o755)
# Reconfigure in a fresh directory. In particular, configure must discover
# sys/ioctl.h in the new overlay rather than reuse an earlier negative probe.
xlib_src = out/"libX11-1.8.10"
if sha(a.xlib_tar) != "2b3b3dad9347db41dca56beb7db5878f283bde1142f04d9f8e478af435dfdc53":
    p.error("unexpected libX11 distribution")
run("tar", "xf", a.xlib_tar, "-C", out)
if sha(xlib_src/"src/xcb_io.c") != "feefeea12f96cca68a63409f18841d5d55e60feac24d527d6e9539e52c1692d1":
    p.error("unexpected libX11 xcb_io.c")
run("patch", "--fuzz=0", "-p1", "-i", here.parent/"xlib-single-thread-diagnostics.patch", cwd=xlib_src)
prior = subprocess.check_output(["sh", a.unix_build/"config.status", "--config"], text=True)
options = [arg for arg in shlex.split(prior) if not arg.startswith(("--prefix=", "CC="))]
env = dict(os.environ, AR="llvm-ar", RANLIB="llvm-ranlib", NM="llvm-nm", CC_FOR_BUILD="cc",
           CONFIG_SITE="/dev/null", LC_ALL="C")
run(xlib_src/"configure", *options, "--prefix="+str(out/"xlib-prefix"), "CC="+str(cc), cwd=build, env=env)
if "#define HAVE_SYS_IOCTL_H 1" not in (build/"src/config.h").read_text():
    p.error("target configure did not discover sys/ioctl.h")
# src/util/makekeys is a host generator. The configured subdirectory
# Makefiles select their own compiler for host tools and target objects.
run("make","-j1","V=1",cwd=build)
