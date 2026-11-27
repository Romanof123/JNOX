"""Cross-link the pinned original Xtrans caller and package it with the native broker."""
import argparse
import ast
import configparser
import hashlib
import os
from pathlib import Path
import subprocess
import sys

p = argparse.ArgumentParser()
p.add_argument("cross", type=Path, help="installed Picolibc/Wayland transport jnox.ini")
p.add_argument("protocol_prefix", type=Path, help="staged xorgproto and xtrans prefix")
p.add_argument("out", type=Path)
a = p.parse_args()
here = Path(__file__).resolve().parent
root = here.parents[2]
out = a.out.resolve()
out.mkdir()
cross = configparser.ConfigParser(interpolation=None)
cross.read(a.cross)
options = cross["built-in options"]
cc = ast.literal_eval(cross["binaries"]["c"])
cflags = ast.literal_eval(options["c_args"])
ldflags = ast.literal_eval(options["c_link_args"])
# Compile the actual pinned function with Xtrans's own headers and types.
# The complete transport translation unit is compiled in the separate X11 step.
xtrans = a.protocol_prefix.resolve()/"include/X11/Xtrans/Xtranssock.c"
if hashlib.sha256(xtrans.read_bytes()).hexdigest() != "a195c3aca8a46c60be1a0f510da95b66881b83310bf6473fede4aa177c9a0b18":
    p.error("unexpected Xtrans 1.4.0 transport source")
source = xtrans.read_text()
functions = []
for name in ("SocketBytesReadable", "SocketReadv", "SocketWritev"):
    declaration = f"static int\nTRANS({name}) "
    if source.count(declaration) != 1:
        p.error(f"ambiguous upstream {name}")
    body = source.split(declaration, 1)[1].split("\n}\n", 1)[0]
    functions.append(declaration + body + "\n}\n")
(out/"original-socket.inc").write_text("\n".join(functions))
if not any("libjnox-wayland-os.a" in flag for flag in ldflags):
    p.error("cross file must link the updated transport archive")
env = dict(os.environ, CARGO_BUILD_JOBS="1", CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(out/"target"))

def run(*args):
    subprocess.run(args, cwd=root, env=env, check=True)

obj = out/"caller.o"
elf = out/"caller.elf"
run(*cc, *cflags, "-D_GNU_SOURCE", "-ffunction-sections", "-fdata-sections",
    "-O0", "-I" + str(out), "-I" + str(a.protocol_prefix.resolve()/"include"), "-c", here/"upstream.c", "-o", obj)
run(*cc, obj, *ldflags, "-Wl,--gc-sections", "-o", elf)
run("sh", "sdk/native-aarch64/build-rust.sh", "services/socket/Cargo.toml", out/"broker.elf")
run(sys.executable, "tools/build_native_el0.py", "--board", "virt", "--policy", "fifo",
    "--elf", elf, "--priority", "5", "--peer-elf", out/"broker.elf", "--peer-priority", "1",
    "--peer-arg", "1", "--socket-service", "1", "--ipc-route", "0:1", "--out", out/"package")
