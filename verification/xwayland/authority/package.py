"""Package Mosaic and the pinned Xwayland authorization subset in five native slots."""
import argparse
import ast
import configparser
import fcntl
import hashlib
import os
from pathlib import Path
import subprocess
import sys

here = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
for option in ("xwayland", "cross", "xcb", "protocol", "os-prefix", "out", "lock"):
    parser.add_argument("--" + option, type=Path, required=True)
parser.add_argument("--repo", type=Path, default=here.parents[2])
args = parser.parse_args()
repo, out = args.repo.resolve(), args.out.resolve()
os.umask(0o077)


def pinned(name, digest):
    data = (args.xwayland / "os" / name).read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        parser.error("unexpected source for " + name)
    return data.decode()


def section(source, start, end):
    first = source.index(start)
    return source[first:source.index(end, first)]


def run(*command):
    subprocess.run(command, cwd=repo, check=True)


with args.lock.open("a") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    out.mkdir()
    os.environ.update(CARGO_BUILD_JOBS="1", MAKEFLAGS="-j1", CARGO_NET_OFFLINE="true")
    os.environ["CARGO_TARGET_DIR"] = str(out / "cargo-target")
    auth = pinned("auth.c", "5e7b404044e2c7f7614b650984bc316ebe5819a6ffa90a01d9a8d91fb57659e0")
    mit = pinned("mitauth.c", "5a7e2368313496be037ef9135479a984befc92c659be0f12a2e3b470715c8f00")
    timing = pinned("timingsafe_memcmp.c", "4acc1f254c6c967d745ff3fff71bfd7ac8e5acda20cd5e7616710f06400aa03f")
    includes = """#include <X11/X.h>
#include <X11/Xauth.h>
#include <jnox/xauthority_stream.h>
#include <sys/utsname.h>
#include <limits.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
typedef void *ClientPtr;
typedef int Bool;
#define TRUE 1
#define FALSE 0
static const char *display = "0";
static XID FakeClientID(int slot) { (void)slot; return 1; }
static void FatalError(const char *format) { (void)format; _exit(91); }
static const struct { unsigned short name_length; const char *name; }
    protocols[] = {{18, "MIT-MAGIC-COOKIE-1"}};
"""
    (out / "original.c").write_text(includes + timing[timing.index("int\ntimingsafe_memcmp"):]
        + section(mit, "static struct auth {", "int\nMitResetCookie")
        + section(auth, "static const char *authorization_file", "#ifdef XDMCP\n/*\n * XdmcpInit")
        + section(auth, "XID\nCheckAuthorization", "void\nResetAuthorization"))
    cfg = configparser.ConfigParser(interpolation=None)
    cfg.read(args.cross)
    cc = ast.literal_eval(cfg["binaries"]["c"])
    opt = cfg["built-in options"]
    flags = ["-I" + str(args.os_prefix.resolve() / "include/jnox-wayland"),
             "-I" + str(args.xcb.resolve() / "include"), "-I" + str(args.protocol.resolve() / "include"),
             "-D_GNU_SOURCE", *(v for v in ast.literal_eval(opt["c_args"]) if not v.endswith("/include/jnox-wayland"))]
    lib = args.os_prefix.resolve() / "lib"
    ld = ast.literal_eval(opt["c_link_args"])
    if sum(Path(v).name == "libjnox-wayland-os.a" for v in ld) != 1:
        parser.error("cross file must name exactly one JNOX graphics transport archive")
    ld = [str(lib / Path(v).name) if Path(v).name == "libjnox-wayland-os.a" else v for v in ld]
    objects = []
    for src, extra in [(out / "original.c", []), (here / "listener.c", []),
                       (here / "stream-probe.c", ["-Dmain=stream_probe"])]:
        obj = out / (src.stem + ".o")
        run(*cc, *flags, *extra, "-c", src, "-o", obj)
        objects.append(obj)
    server, broker, mosaic, denied = [out / (name + ".elf") for name in ("server", "broker", "mosaic", "denied")]
    run(*cc, *objects, args.xcb.resolve() / "lib/libXau.a", lib / "libjnox-xauthority.a",
        *ld, "-o", server)
    run("sh", "sdk/native-aarch64/build.sh", here / "denied.c", denied)
    run("sh", "sdk/native-aarch64/build.sh", "--local-socket", "examples/native-x11/client.c", mosaic)
    run("sh", "sdk/native-aarch64/build-rust.sh", "--features", "display-five-active",
        "services/socket/Cargo.toml", broker)
    run(sys.executable, "tools/build_native_el0.py", "--board", "virt", "--policy", "fifo",
        "--display-five-active", "--nodename", "guest-a", "--elf", denied, "--priority", "7",
        "--peer-elf", server, "--peer-priority", "5", "--peer-elf", denied, "--peer-priority", "5",
        "--peer-elf", broker, "--peer-priority", "1", "--peer-elf", mosaic, "--peer-priority", "5",
        "--peer4-arg", "unix", "--display-service", "0", "--socket-service", "3", "--shared-fd",
        "--ipc-route", "0:3", "--ipc-route", "1:3", "--ipc-route", "2:3", "--ipc-route", "4:3",
        "--out", out / "package")
