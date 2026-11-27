"""Boot the packaged registry workload, observe UART, then close the real compositor."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import subprocess
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def boot(package, out, profile, env_connect=False):
    out.mkdir()
    qemu = Path(shutil.which("qemu-system-aarch64")).resolve()
    command = [str(qemu), "-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
               "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none", "-monitor", "none",
               "-no-reboot", "-serial", "stdio", "-device", "ramfb,id=screen",
               "-kernel", str(package/"kernel.elf")]
    core = {"wl_compositor": 4, "wl_shm": 1, "xdg_wm_base": 1, "wl_seat": 4}
    with_output = {**core, "wl_output": 3}
    transcript = bytearray()
    closing = False
    deadline = time.monotonic() + 90
    with (out/"uart.bin").open("xb") as uart, (out/"qemu.stderr").open("xb") as stderr:
        proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr)
        try:
            with selectors.DefaultSelector() as poll:
                poll.register(proc.stdout, selectors.EVENT_READ)
                while time.monotonic() < deadline:
                    if not poll.select(0.5):
                        continue
                    chunk = os.read(proc.stdout.fileno(), 65536)
                    if not chunk:
                        break
                    uart.write(chunk)
                    uart.flush()
                    transcript.extend(chunk)
                    if not closing and b"JNOX_REGISTRY disconnect-result 0" in transcript:
                        proc.stdin.write(b"q")
                        proc.stdin.flush()
                        closing = True
                    if all(f"NATIVE EL0 EXIT PID {pid} 0".encode() in transcript for pid in range(4)):
                        break
        finally:
            proc.kill()
            proc.wait()
    text = bytes(transcript).decode("ascii", errors="replace").replace("\r", "")
    rows = re.findall(r"JNOX_REGISTRY global (\w+) (\d+) (\d+)\n", text)
    versions = {name: int(version) for name, _, version in rows}
    names = [int(num) for _, num, _ in rows]
    exits = sorted((int(pid), int(code)) for pid, code in re.findall(r"NATIVE EL0 EXIT PID (\d+) (\d+)\b", text))
    registry_matches = versions == with_output or (profile == "core" and versions == core)
    passed = (closing and exits == [(i, 0) for i in range(4)] and registry_matches
              and len(rows) == len(versions) == len(set(names)) and min(names, default=0) > 0
              and "JNOX_REGISTRY disconnect-result 0\n" in text)
    report = dict(accepted=passed, profile=profile, argv=command, qemu_sha256=digest(qemu), uart_sha256=digest(out/"uart.bin"),
                  package={p.name: digest(p) for p in package.glob("*.elf")}, exits=exits,
                  globals=[dict(interface=n, name=int(i), version=int(v)) for n,i,v in rows])
    if env_connect:
        matches = re.findall(r"^JNOX_REGISTRY env-connect (/wayland-0) fd (\d+) absent-errno (\d+)$",
                             text, re.MULTILINE)
        # errno numbering belongs to Picolibc; the guest verifies ENOENT/ECONNREFUSED.
        passed = passed and len(matches) == 1 and int(matches[0][2]) > 0
        report["accepted"] = passed
        if len(matches) == 1:
            path, fd, error = matches[0]
            report["environment_connection"] = dict(path=path, fd=int(fd), missing_path_errno=int(error))
    (out/"registry.json").write_text(json.dumps(report, indent=2)+"\n")
    if not passed:
        raise SystemExit("registry or process exit receipt failed; see uart.bin")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--profile", choices=("core", "output"), default="core")
    parser.add_argument("--env-connect", action="store_true")
    args = parser.parse_args()
    boot(args.package.resolve(), args.output, args.profile, args.env_connect)
