"""Exercise the native X11 setup reader, Mosaic and the five-role broker in QEMU."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import re
import selectors
import subprocess
import tempfile
import time


def run_case(kernel, records, seconds):
    command = ["qemu-system-aarch64", "-machine", "virt,gic-version=2", "-cpu", "cortex-a72",
               "-accel", "tcg", "-smp", "1", "-m", "128M", "-nodefaults", "-no-reboot",
               "-display", "none", "-device", "ramfb", "-monitor", "none", "-serial", "stdio",
               "-kernel", kernel]
    for role, path in zip(("server", "client"), records):
        if path is not None:
            command.extend(["-fw_cfg", f"name=opt/jnox/xauth-{role},file={path}"])
    exits = {}
    with subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          stderr=subprocess.DEVNULL) as proc, selectors.DefaultSelector() as ready:
        try:
            ready.register(proc.stdout, selectors.EVENT_READ)
            end = time.monotonic() + seconds
            tail = b""
            while len(exits) < 5 and time.monotonic() < end:
                if not ready.select(0.25):
                    continue
                data = os.read(proc.stdout.fileno(), 65536)
                if not data:
                    break
                lines = (tail + data).split(b"\n")
                tail = lines.pop()[-512:]
                for line in lines:
                    if line.startswith(b"NATIVE EL0 FAULT"):
                        raise RuntimeError("guest reported a native fault")
                    m = re.fullmatch(rb"NATIVE EL0 EXIT PID ([0-4]) (-?[0-9]+)\r?", line)
                    if m:
                        pid, code = map(int, m.groups())
                        if pid in exits:
                            raise ValueError("duplicate terminal exit")
                        exits[pid] = code
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
    return [exits.get(i) for i in range(5)]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("repo", type=Path)
    p.add_argument("kernel", type=Path)
    p.add_argument("out", type=Path)
    p.add_argument("lock", type=Path)
    p.add_argument("--timeout", type=int, default=30)
    args = p.parse_args()
    os.umask(0o077)
    args.out.mkdir()
    with args.lock.open("a") as lock, tempfile.TemporaryDirectory() as private:
        fcntl.flock(lock, fcntl.LOCK_EX)
        folder = Path(private) / "records"
        subprocess.run(["python3", args.repo / "examples/native-x11/local_profile.py",
                        "prepare", folder, "--node", "guest-a"], check=True,
                       stdout=subprocess.DEVNULL)
        cases = [("match", (folder / "server.auth", folder / "client.auth"), [72,70,72,0,104]),
                 ("mismatch", (folder / "server.auth", folder / "wrong.auth"), [72,71,72,0,114]),
                 ("absent", (None, None), [72,91,72,0,116])]
        failures = 0
        for name, records, expected in cases:
            observed = run_case(args.kernel, records, args.timeout)
            result = {"case": name, "observed_exits": observed, "expected_exits": expected,
                      "passed": observed == expected}
            (args.out / (name + ".json")).write_text(json.dumps(result) + "\n")
            print(json.dumps(result), flush=True)
            failures += observed != expected
        return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
