"""Exercise one guest application breakpoint and bounded shared-object reclamation."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

here = Path(__file__).resolve().parent
repo = here.parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("out", type=Path, help="fresh output directory")
out = parser.parse_args().out.resolve()
out.mkdir()
env = dict(os.environ, CARGO_BUILD_JOBS="1", CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(out / "target"))


def run(*cmd):
    subprocess.run(cmd, cwd=repo, env=env, check=True)


app, broker, pkg = out / "probe.elf", out / "broker.elf", out / "package"
run("sh", "sdk/native-aarch64/build.sh", "--local-socket", here / "probe.c", app)
run("sh", "sdk/native-aarch64/build-rust.sh", "--features", "display-five-active",
    "services/socket/Cargo.toml", broker)
run(sys.executable, "tools/build_native_el0.py", "--board", "virt", "--policy", "fifo",
    "--display-five-active", "--elf", app, "--priority", "4",
    "--peer-elf", app, "--peer-priority", "7", "--peer-elf", app, "--peer-priority", "5",
    "--peer-elf", broker, "--peer-priority", "1", "--peer-elf", app, "--peer-priority", "4",
    "--display-service", "0", "--socket-service", "3", "--shared-fd",
    "--ipc-route", "0:3", "--ipc-route", "1:3", "--ipc-route", "2:3", "--ipc-route", "4:3",
    "--out", pkg)
uart = out / "guest.log"
run(sys.executable, "examples/native-pixman/capture.py", pkg / "kernel.elf", uart, "5", "--symbol", "main")
data = uart.read_bytes()
records = re.findall(rb"^NATIVE EL0 RETIRED (\d+) (\d+) (-?\d+)\r?$", data, re.M)
exits = re.findall(rb"^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$", data, re.M)
if records != [(b"1", b"60", b"-5")] or exits != [
        (b"0", b"0"), (b"1", b"-5"), (b"2", b"72"), (b"3", b"0"), (b"4", b"0")]:
    raise SystemExit(f"unexpected retired role or peer status: {records!r}, {exits!r}")
result = {"retired_slot": 1, "exception_class": 60, "slot_exits": [int(code) for _, code in exits],
          "reallocated_objects": 4}
(out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
