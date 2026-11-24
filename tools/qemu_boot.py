"""Check the M1 AArch64 boot transcript across fresh full-system QEMU boots."""

import argparse
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


ENDINGS = (b"\n", b"\r\n")
TRANSCRIPTS = tuple(
    b"JNOX M1 BOOT" + first + b"JNOX M1 READY" + second
    for first, second in itertools.product(ENDINGS, repeat=2)
)
READ_LIMIT = max(map(len, TRANSCRIPTS)) + 1
QUIET_SECONDS = 0.1


def kernel_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as kernel:
        if kernel.read(4) != b"\x7fELF":
            raise ValueError(f"expected an ELF kernel at {path}")
        kernel.seek(0)
        for block in iter(lambda: kernel.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def observe(process, serial, timeout):
    deadline = time.monotonic() + timeout
    quiet_until = None
    while True:
        if process.poll() is not None:
            return f"QEMU exited before host stop (status {process.returncode})"
        serial.seek(0)
        received = serial.read(READ_LIMIT)
        now = time.monotonic()
        if not any(candidate.startswith(received) for candidate in TRANSCRIPTS):
            return "unexpected UART bytes or marker order"
        if quiet_until is None:
            if now >= deadline:
                return f"UART deadline exceeded ({timeout:g}s)"
            if received in TRANSCRIPTS:
                quiet_until = now + QUIET_SECONDS
        elif now >= quiet_until:
            return None
        time.sleep(min(0.01, (quiet_until or deadline) - now))


def stop(process):
    status = process.poll()
    if status is not None:
        return f"QEMU exited before host stop (status {status})"
    process.terminate()
    try:
        status = process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=2)
        return "QEMU required forced termination"
    # QEMU may handle SIGTERM and exit cleanly. Windows uses TerminateProcess.
    expected = (0, 1) if os.name == "nt" else (0, -signal.SIGTERM)
    if status not in expected:
        return f"unexpected QEMU exit status {status}"
    return None


def run_boot(command, timeout):
    with tempfile.TemporaryDirectory(prefix="jnox-qemu-") as folder:
        uart_path = Path(folder) / "uart"
        stderr_path = Path(folder) / "stderr"
        with uart_path.open("wb") as output, stderr_path.open("wb") as errors:
            process = subprocess.Popen(
                command, stdin=subprocess.DEVNULL, stdout=output, stderr=errors
            )
            try:
                print(f"  QEMU PID = {process.pid}", flush=True)
                with uart_path.open("rb") as serial:
                    problem = observe(process, serial, timeout)
            finally:
                shutdown_problem = stop(process)
        uart = uart_path.read_bytes()
        print(f"  UART ({len(uart)} bytes) = {uart!r}")
        print(f"  QEMU stderr = {stderr_path.read_bytes()!r}", flush=True)
        if problem or shutdown_problem:
            raise ValueError(problem or shutdown_problem)
        if uart not in TRANSCRIPTS:
            raise ValueError("unexpected UART bytes during shutdown")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, required=True, help="AArch64 kernel ELF")
    parser.add_argument("--qemu", default="qemu-system-aarch64", help="QEMU executable (default: %(default)s)")
    parser.add_argument("--runs", type=int, default=2, help="fresh QEMU boots (default: %(default)s)")
    parser.add_argument("--timeout", type=float, default=5, metavar="SECONDS",
                        help="UART deadline per boot, followed by a 0.1s quiet window (default: %(default)s)")
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("--timeout must be positive and finite")

    try:
        kernel = args.kernel.resolve(strict=True)
        expected_hash = kernel_sha256(kernel)
        command = [
            args.qemu, "-machine", "virt", "-cpu", "cortex-a72", "-smp", "1",
            "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none",
            "-monitor", "none", "-serial", "stdio", "-kernel", str(kernel),
        ]
        for run in range(1, args.runs + 1):
            print(f"Run {run}/{args.runs}  ELF SHA-256 = {expected_hash}")
            print(f"  argv = {json.dumps(command, ensure_ascii=True)}", flush=True)
            run_boot(command, args.timeout)
            if kernel_sha256(kernel) != expected_hash:
                raise ValueError("kernel ELF changed during testing")
            print("  PASS (host stopped QEMU after the quiet window)", flush=True)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.runs} cold boot(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
