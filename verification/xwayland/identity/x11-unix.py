#!/usr/bin/env python3
"""Reconfigure the actual libX11 tree for JNOX AF_UNIX only."""
import argparse, fcntl, os, re, shlex, shutil, subprocess
from pathlib import Path
p = argparse.ArgumentParser()
for name in ("lab","configured","identity-run","out"):
    p.add_argument("--"+name,type=Path,required=True)
a = p.parse_args()
out = a.out.resolve()
out.mkdir()
lock = (a.lab/"run.lock").open("a")
fcntl.flock(lock, fcntl.LOCK_EX)
inc = out/"include"
shutil.copytree(a.identity_run/"xtrans-include",inc)
subprocess.run(["patch","--fuzz=0","-p1","-i",Path(__file__).resolve().with_name("xtrans-unix-only.patch")],
               cwd=inc, check=True)
args = shlex.split(subprocess.check_output(["sh","./config.status","--config"],cwd=a.configured,text=True))
source = re.search(r"^abs_top_srcdir = (.+)$",(a.configured/"Makefile").read_text(),re.M)[1]
for i,item in enumerate(args):
    if item.startswith("CFLAGS="):
        args[i] = ("CFLAGS=-DJNOX_XTRANS_LOCAL=1 -DNO_TCP_H=1 -I"+str(inc)+" -I"+
                   str(a.identity_run/"target/include/jnox-wayland")+" "+item[7:])
    elif item.startswith("CC="):
        # Automake places protocol-package include arguments before CFLAGS.
        # Put the private, patched Xtrans tree first at the actual CC entry.
        wrapper = out/"x11-cc"
        wrapper.write_text("#!/bin/sh\nexec "+item[3:]+" -I"+shlex.quote(str(inc))+" -I"+
                           shlex.quote(str(a.identity_run/"target/include/jnox-wayland"))+" \"$@\"\n")
        wrapper.chmod(0o755)
        args[i] = "CC="+str(wrapper)
    elif item.startswith("--prefix="): args[i] = "--prefix="+str(out/"target")
build = out/"build"
build.mkdir()
args += ["--enable-unix-transport","--disable-tcp-transport","--disable-local-transport"]
env = os.environ.copy()
env["PKG_CONFIG_PATH"] = ""
for item in args:
    if item.startswith("PKG_CONFIG_LIBDIR="): env["PKG_CONFIG_LIBDIR"] = item.split("=",1)[1]
for cmd in (["sh",str(Path(source)/"configure"),*args],["make","-j1","V=1"]):
    print(shlex.join(cmd),flush=True)
    subprocess.run(cmd,cwd=build,env=env,check=True)
