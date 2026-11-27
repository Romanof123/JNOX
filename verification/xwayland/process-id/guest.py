"""Package and boot the JNOX shell parent and Picolibc child identity fixtures."""
import argparse
import ast
import configparser
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

here = Path(__file__).resolve().parent
repo = here.parents[2]
sdk = repo / "sdk/native-aarch64"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("runtime", type=Path, help="verified Picolibc graphics runtime")
parser.add_argument("out", type=Path, help="fresh output directory")
args = parser.parse_args()
runtime, out = args.runtime.resolve(), args.out.resolve()
out.mkdir()
os.environ.update(CARGO_BUILD_JOBS="1", CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(out / "target"))


def run(*cmd):
    subprocess.run(cmd, check=True, cwd=repo)


parent = out / "parent.elf"
run("sh", sdk / "build.sh", "--native-shell", here / "parent.c", parent)
prefix = out / "picolibc"
shutil.copytree(runtime / "target/include", prefix / "include")
(prefix / "lib").mkdir()
(prefix / "lib/libc.a").symlink_to(runtime / "target/lib/libc.a")
run(sys.executable, sdk / "graphics/os/install.py", "--prefix", prefix)
cfg = configparser.ConfigParser(interpolation=None)
cfg.read(runtime / "jnox.ini")
cc = ast.literal_eval(cfg["binaries"]["c"])
opt = cfg["built-in options"]
child = out / "child.elf"
obj = out / "child.o"
run(*cc, *ast.literal_eval(opt["c_args"]), "-iquote", sdk / "include", "-c", here / "child.c", "-o", obj)
run(*cc, obj, prefix / "lib/libjnox-wayland-os.a", *ast.literal_eval(opt["c_link_args"]), "-o", child)
package = out / "package"
run(sys.executable, "tools/build_native_el0.py", "--board", "virt", "--policy", "fifo", "--pipes",
    "--elf", parent, "--priority", "4", "--peer-elf", child, "--peer-priority", "6", "--exec-child", "1:live",
    "--peer-elf", child, "--peer-priority", "6", "--exec-child", "2:orphan", "--out", package)
uart = out / "guest.log"
run(sys.executable, "examples/native-pixman/capture.py", package / "kernel.elf", uart, "3", "--symbol", "main")
data = uart.read_bytes()
rows = re.findall(rb"^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$", data, re.M)
if rows != [(b"0", b"80"), (b"1", b"81"), (b"2", b"82")]:
    raise SystemExit("unexpected identity guest exit records: " + repr(rows))
tasks = re.findall(rb"^NATIVE PIPE TASK 1 2 (\d+) (\d+) \d+ \d+\r?$", data, re.M)
if tasks != [(b"1", b"1")]:
    raise SystemExit("missing the orphan child's one pipe block and wake")
result = {"slot_exits": {int(i): int(value) for i, value in rows}, "orphan_blocks": 1, "orphan_wakes": 1}
(out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
