"""Independent WSL serial observer. Run with Python 3 and one ELF per process."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument("elf", type=Path)
parser.add_argument("result", type=Path)
parser.add_argument("--boot-timeout", default=5.0, type=float)
parser.add_argument("--quiet", default=1.0, type=float)
args = parser.parse_args()
qemu = ["qemu-system-aarch64", "-machine", "virt", "-cpu", "cortex-a72",
        "-smp", "1", "-m", "128M", "-nodefaults", "-display", "none",
        "-monitor", "none", "-chardev", "stdio,id=probe,signal=off,mux=off",
        "-serial", "chardev:probe", "-kernel", str(args.elf)]
started = time.monotonic()
proc = subprocess.Popen(qemu, stdin=subprocess.DEVNULL,
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
poller = selectors.DefaultSelector()
for stream, name in [(proc.stdout, "serial"), (proc.stderr, "stderr")]:
    poller.register(stream, selectors.EVENT_READ, name)
data = {"serial": bytearray(), "stderr": bytearray()}
events = []
ready = None
wire = re.compile(rb"JNOX M1 BOOT\r?\nJNOX M1 READY\r?\n")
while True:
    now = time.monotonic()
    end = started + args.boot_timeout if ready is None else ready + args.quiet
    if proc.poll() is not None or now >= end:
        break
    for key, mask in poller.select(min(0.05, end - now)):
        chunk = os.read(key.fileobj.fileno(), 65536)
        if chunk:
            data[key.data].extend(chunk)
            events.append(dict(t=round(time.monotonic()-started, 6),
                               stream=key.data, hex=chunk.hex()))
        else:
            poller.unregister(key.fileobj)
    if ready is None and wire.match(data["serial"]):
        ready = time.monotonic()
alive = proc.poll() is None
ended = time.monotonic()
if alive:
    proc.kill()
tail_serial, tail_error = proc.communicate(timeout=3)
data["serial"].extend(tail_serial)
data["stderr"].extend(tail_error)
passed = (alive and ready is not None and ended >= ready + args.quiet and
          wire.fullmatch(data["serial"]) is not None and not data["stderr"])
record = dict(command=qemu, elf_sha256=hashlib.sha256(args.elf.read_bytes()).hexdigest(),
              serial_hex=data["serial"].hex(), stderr_hex=data["stderr"].hex(),
              boot_timeout_s=args.boot_timeout, requested_quiet_s=args.quiet,
              ready_s=None if ready is None else round(ready-started, 6),
              elapsed_s=round(ended-started, 6), live_at_deadline=alive,
              after_stop_returncode=proc.returncode, passed=passed, events=events)
args.result.write_text(json.dumps(record, indent=2)+"\n")
print(json.dumps({k:v for k,v in record.items() if k != "events"}))
raise SystemExit(0 if passed else 1)
