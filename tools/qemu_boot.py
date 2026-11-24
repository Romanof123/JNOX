"""Check an AArch64 UART transcript in fresh full-system QEMU guests."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


BOOT_TRANSCRIPT = b"JNOX M1 BOOT\nJNOX M1 READY\n"
SCHEDULER_START = BOOT_TRANSCRIPT + (
    b"SCHED LOW RUN\n"
    b"SCHED TIMER IRQ\n"
)
SCHEDULER_TRANSCRIPT = SCHEDULER_START + (
    b"SCHED HIGH RUN\n"
    b"SCHED LOW RESUMED\n"
    b"SCHED PASS\n"
)
EDF_TRANSCRIPT = SCHEDULER_START + (
    b"EDF URGENT RUN\n"
    b"SCHED LOW RESUMED\n"
    b"SCHED PASS\n"
)
FIFO_TRANSCRIPT = SCHEDULER_START + (
    b"FIFO A FIRST\n"
    b"FIFO B FIRST\n"
    b"FIFO A RESUMED\n"
    b"FIFO B RESUMED\n"
    b"SCHED LOW RESUMED\n"
    b"SCHED PASS\n"
)
RR_TRANSCRIPT = BOOT_TRANSCRIPT + (
    b"POLICY RANK PASS\n"
    b"POLICY SERVICE PASS\n"
    b"SCHED LOW RUN\n"
    b"SCHED TIMER IRQ\n"
    b"RR A FIRST\n"
    b"RR B FIRST\n"
    b"RR A RESUMED\n"
    b"RR B RESUMED\n"
    b"SCHED LOW RESUMED\n"
    b"SCHED PASS\n"
)
TRANSCRIPTS = {
    "boot": BOOT_TRANSCRIPT,
    "scheduler": SCHEDULER_TRANSCRIPT,
    "edf": EDF_TRANSCRIPT,
    "fifo": FIFO_TRANSCRIPT,
    "rr": RR_TRANSCRIPT,
}
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


def read_uart(serial, expected):
    # All lines may use CRLF. One extra byte exposes any trailing output.
    serial.seek(0)
    return serial.read(len(expected) + expected.count(b"\n") + 1)


def observe(process, serial, timeout, expected=BOOT_TRANSCRIPT):
    deadline = time.monotonic() + timeout
    quiet_until = None
    while True:
        if process.poll() is not None:
            return f"QEMU exited before host stop (status {process.returncode})"
        received = read_uart(serial, expected).replace(b"\r\n", b"\n")
        # A trailing CR can be the first byte of the next line ending.
        prefix = received[:-1] + b"\n" if received.endswith(b"\r") else received
        now = time.monotonic()
        if not expected.startswith(prefix):
            return "unexpected UART bytes or marker order"
        if quiet_until is None:
            if now >= deadline:
                return f"UART deadline exceeded ({timeout:g}s)"
            if received == expected:
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


def run_boot(command, timeout, expected=BOOT_TRANSCRIPT):
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
                    problem = observe(process, serial, timeout, expected)
            finally:
                shutdown_problem = stop(process)
        with uart_path.open("rb") as serial:
            uart = read_uart(serial, expected)
        uart_size = uart_path.stat().st_size
        uart_suffix = " (truncated)" if len(uart) < uart_size else ""
        print(f"  UART ({uart_size} bytes) = {uart!r}{uart_suffix}")
        with stderr_path.open("rb") as errors:
            stderr = errors.read(4096)
        stderr_suffix = " (truncated)" if len(stderr) < stderr_path.stat().st_size else ""
        print(f"  QEMU stderr = {stderr!r}{stderr_suffix}", flush=True)
        if problem or shutdown_problem:
            raise ValueError(problem or shutdown_problem)
        if uart.replace(b"\r\n", b"\n") != expected:
            raise ValueError("unexpected UART bytes during shutdown")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, required=True, help="AArch64 kernel ELF")
    parser.add_argument("--qemu", default="qemu-system-aarch64", help="QEMU executable (default: %(default)s)")
    parser.add_argument("--profile", choices=TRANSCRIPTS, default="boot",
                        help="expected UART transcript (default: %(default)s)")
    parser.add_argument("--runs", type=int, help="fresh sequential QEMU boots (default: boot 2, other profiles 1)")
    parser.add_argument("--timeout", type=float, default=5, metavar="SECONDS",
                        help="UART deadline per boot, followed by a 0.1s quiet window (default: %(default)s)")
    args = parser.parse_args()
    boot = args.profile == "boot"
    runs = args.runs if args.runs is not None else (2 if boot else 1)
    if runs < 1:
        parser.error("--runs must be positive")
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("--timeout must be positive and finite")

    try:
        kernel = args.kernel.resolve(strict=True)
        expected_hash = kernel_sha256(kernel)
        machine = "virt" if boot else "virt,gic-version=2"
        expected = TRANSCRIPTS[args.profile]
        command = [
            args.qemu, "-machine", machine, "-cpu", "cortex-a72", "-smp", "1",
            "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none",
            "-monitor", "none", "-serial", "stdio", "-kernel", str(kernel),
        ]
        for run in range(1, runs + 1):
            print(f"Run {run}/{runs}  ELF SHA-256 = {expected_hash}")
            print(f"  argv = {json.dumps(command, ensure_ascii=True)}", flush=True)
            run_boot(command, args.timeout, expected)
            if kernel_sha256(kernel) != expected_hash:
                raise ValueError("kernel ELF changed during testing")
            print("  PASS (host stopped QEMU after the quiet window)", flush=True)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {runs} cold boot(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
