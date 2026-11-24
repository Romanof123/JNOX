"""Check an AArch64 UART transcript in fresh full-system QEMU guests."""

import argparse
from enum import Enum
import hashlib
from importlib import import_module
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


PI4_BOOT_TRANSCRIPT = b"JNOX PI4 BOOT\nJNOX PI4 READY\n"
BOOT_TRANSCRIPT = b"JNOX M1 BOOT\nJNOX M1 READY\n"
SCHEDULER_LINES = (
    b"SCHED LOW RUN\n"
    b"SCHED TIMER IRQ\n"
)
SCHEDULER_START = BOOT_TRANSCRIPT + SCHEDULER_LINES
FIXED_PRIORITY_LINES = (
    b"SCHED HIGH RUN\n"
    b"SCHED LOW RESUMED\n"
    b"SCHED PASS\n"
)
SCHEDULER_TRANSCRIPT = SCHEDULER_START + FIXED_PRIORITY_LINES
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
CBS_TRANSCRIPT = BOOT_TRANSCRIPT + (
    b"CBS A FIRST\n"
    b"CBS B FIRST\n"
    b"CBS IDLE\n"
    b"CBS A RESUMED\n"
    b"CBS B RESUMED\n"
    b"CBS TIMER STOP\n"
    b"CBS PASS\n"
)
PERIODIC_RELEASES = BOOT_TRANSCRIPT + (
    b"RM 0 A\n"
    b"RM 0 B\n"
    b"RM 2 A\n"
    b"RM 3 B\n"
    b"RM 4 A\n"
    b"DM 0 B\n"
    b"DM 0 A\n"
    b"DM 2 A\n"
    b"DM 3 B\n"
    b"DM 4 A\n"
)
PERIODIC_TRANSCRIPT = PERIODIC_RELEASES + b"PERIODIC PASS\n"
PERIODIC_PREEMPT_TRANSCRIPT = PERIODIC_RELEASES + (
    b"RM A START\n"
    b"RM A CONTINUE\n"
    b"RM B RUN\n"
    b"DM A START\n"
    b"DM B RUN\n"
    b"DM A CONTINUE\n"
    b"PREEMPT PASS\n"
)
PARTITION_TRANSCRIPT = BOOT_TRANSCRIPT + (
    b"PART A FIRST\n"
    b"PART IDLE FIRST\n"
    b"PART B READY\n"
    b"PART A RESUMED\n"
    b"PART B FIRST\n"
    b"PART IDLE RESUMED\n"
    b"PART STOP\n"
    b"PART PASS\n"
)
COOP_TRANSCRIPT = BOOT_TRANSCRIPT + (
    b"COOP A FIRST\n"
    b"COOP IRQ B READY\n"
    b"COOP A RETAINED\n"
    b"COOP B FIRST\n"
    b"COOP A RESUMED\n"
    b"COOP B WOKEN\n"
    b"COOP B RESUMED\n"
    b"EVENT 1\n"
    b"EVENT 2\n"
    b"EVENT 3\n"
    b"COOP PASS\n"
)
BATCH_TRANSCRIPT = BOOT_TRANSCRIPT + (
    b"BATCH SJF A FIRST\n"
    b"BATCH SJF IRQ B READY\n"
    b"BATCH SJF A RETAINED\n"
    b"BATCH SJF B FIRST\n"
    b"BATCH SRTF A FIRST\n"
    b"BATCH SRTF IRQ B READY\n"
    b"BATCH SRTF B FIRST\n"
    b"BATCH SRTF A RESUMED\n"
    b"BATCH HRRN A FIRST\n"
    b"BATCH HRRN IRQ C READY\n"
    b"BATCH HRRN A RETAINED\n"
    b"BATCH HRRN B FIRST\n"
    b"BATCH HRRN C FIRST\n"
    b"BATCH PASS\n"
)
DEADLINE_TRANSCRIPT = BOOT_TRANSCRIPT + (
    b"DEADLINE EDF A FIRST\n"
    b"DEADLINE EDF IRQ C READY\n"
    b"DEADLINE EDF C FIRST\n"
    b"DEADLINE EDF A RESUMED\n"
    b"DEADLINE EDF B FIRST\n"
    b"DEADLINE LLF B FIRST\n"
    b"DEADLINE LLF B TIE RETAINED\n"
    b"DEADLINE LLF T6 CROSSOVER\n"
    b"DEADLINE LLF A FIRST\n"
    b"DEADLINE LLF B RESUMED T6\n"
    b"DEADLINE LLF IRQ C READY\n"
    b"DEADLINE LLF C FIRST\n"
    b"DEADLINE LLF B RESUMED T7\n"
    b"DEADLINE PASS\n"
)
APERIODIC_TRANSCRIPT = BOOT_TRANSCRIPT + (
    b"APER BEGIN PS\n"
    b"APER t=0 id=0 budget=0 credits=-\n"
    b"APER t=1 id=0 budget=0 credits=-\n"
    b"APER t=2 id=0 budget=0 credits=-\n"
    b"APER t=3 id=0 budget=0 credits=-\n"
    b"APER t=4 id=1 budget=1 credits=-\n"
    b"APER t=5 id=2 budget=0 credits=-\n"
    b"APER t=6 id=0 budget=0 credits=-\n"
    b"APER t=7 id=0 budget=0 credits=-\n"
    b"APER t=8 id=2 budget=1 credits=-\n"
    b"APER t=9 id=2 budget=0 credits=-\n"
    b"APER END pending=2 charged=4 completed=4\n"
    b"APER BEGIN DS\n"
    b"APER t=0 id=0 budget=2 credits=-\n"
    b"APER t=1 id=1 budget=1 credits=-\n"
    b"APER t=2 id=0 budget=1 credits=-\n"
    b"APER t=3 id=2 budget=0 credits=-\n"
    b"APER t=4 id=2 budget=1 credits=-\n"
    b"APER t=5 id=2 budget=0 credits=-\n"
    b"APER t=6 id=0 budget=0 credits=-\n"
    b"APER t=7 id=0 budget=0 credits=-\n"
    b"APER t=8 id=3 budget=1 credits=-\n"
    b"APER t=9 id=4 budget=0 credits=-\n"
    b"APER END pending=0 charged=6 completed=6\n"
    b"APER BEGIN SS\n"
    b"APER t=0 id=0 budget=2 credits=-\n"
    b"APER t=1 id=1 budget=1 credits=5:1\n"
    b"APER t=2 id=0 budget=1 credits=5:1\n"
    b"APER t=3 id=2 budget=0 credits=5:1,7:1\n"
    b"APER t=4 id=0 budget=0 credits=5:1,7:1\n"
    b"APER t=5 id=2 budget=0 credits=7:1,9:1\n"
    b"APER t=6 id=0 budget=0 credits=7:1,9:1\n"
    b"APER t=7 id=2 budget=0 credits=9:1,11:1\n"
    b"APER t=8 id=0 budget=0 credits=9:1,11:1\n"
    b"APER t=9 id=3 budget=0 credits=11:1,13:1\n"
    b"APER END pending=1 charged=5 completed=5\n"
    b"APER BEGIN SS1\n"
    b"APER t=0 id=0 budget=2 credits=-\n"
    b"APER t=1 id=1 budget=1 credits=5:1\n"
    b"APER t=2 id=0 budget=1 credits=5:1\n"
    b"APER t=3 id=2 budget=0 credits=7:2\n"
    b"APER t=4 id=0 budget=0 credits=7:2\n"
    b"APER t=5 id=0 budget=0 credits=7:2\n"
    b"APER t=6 id=0 budget=0 credits=7:2\n"
    b"APER t=7 id=2 budget=1 credits=11:1\n"
    b"APER t=8 id=2 budget=0 credits=12:2\n"
    b"APER t=9 id=0 budget=0 credits=12:2\n"
    b"APER END pending=2 charged=4 completed=4\n"
    b"APER STOP\n"
    b"APER PASS\n"
)


class DynamicTranscript(Enum):
    AMC = ("AMC", "amc_observer")
    VRUNTIME = ("VR", "vruntime_observer")

    def __init__(self, label, module):
        self.label = label
        self.prefix = label.encode("ascii")
        self.module = module

    def observer(self):
        if __package__:
            return import_module("." + self.module, __package__)
        return import_module(self.module)


TRANSCRIPTS = {
    "priority-resume": BOOT_TRANSCRIPT + b"PRIORITY RESUME BEGIN\nPRIORITY RESUME PASS\n",
    "periodic-overrun": BOOT_TRANSCRIPT + b"OVERRUN RM DONE\nOVERRUN DM DONE\nPERIODIC OVERRUN PASS\n",
    "cbs-wake": BOOT_TRANSCRIPT + b"CBS WAKE TIMER STOP\nCBS WAKE PASS\n",
    "amc": None,  # Counter values vary per boot. Freeze the validated UART before quiet.
    "vruntime": DynamicTranscript.VRUNTIME,
    "proportional": BOOT_TRANSCRIPT + (
        b"LOTTERY SEED A\nLOTTERY SEED B\nSTRIDE SERVICE JOIN\nSTRIDE DELAY CHARGED\nPROPORTIONAL PASS\n"
    ),
    "multilevel": BOOT_TRANSCRIPT + (
        b"MLQ STRICT FOUR\nMLFQ YIELDS DEMOTED\nMLFQ BOOST ORDER\nMULTILEVEL PASS\n"
    ),
    "mutex": BOOT_TRANSCRIPT + (
        b"MUTEX PI TRANSFER\nMUTEX CHAIN TIMEOUT\n"
        b"MUTEX CEILING NESTED\nMUTEX PASS\n"
    ),
    "smp": BOOT_TRANSCRIPT + (
        b"SMP BOTH ONLINE\nSMP PARTITION PASS\nSMP GLOBAL CONTENDED\n"
        b"SMP GLOBAL IRQ\nSMP GLOBAL MIGRATED\nSMP CLUSTER REJECTED\n"
        b"SMP CLUSTER PASS\nSMP PASS\n"
    ),
    "aperiodic": APERIODIC_TRANSCRIPT,
    "deadline": DEADLINE_TRANSCRIPT,
    "batch": BATCH_TRANSCRIPT,
    "coop": COOP_TRANSCRIPT,
    "boot": BOOT_TRANSCRIPT,
    "pi4": PI4_BOOT_TRANSCRIPT,
    "pi4-scheduler": PI4_BOOT_TRANSCRIPT + SCHEDULER_LINES + FIXED_PRIORITY_LINES,
    "eevdf": BOOT_TRANSCRIPT + (
        b"EEVDF BEGIN\nEEVDF EARLY INELIGIBLE\nEEVDF SLICE CHANGE\n"
        b"EEVDF WEIGHT CHANGE\nEEVDF TWO WAKES\nEEVDF STOP\nEEVDF PASS\n"
    ),
    "hierarchy": BOOT_TRANSCRIPT + (
        b"HIERARCHY WAIT\nHIERARCHY THROTTLE\nHIERARCHY REFILL\nHIERARCHY PASS\n"
    ),
    "power": BOOT_TRANSCRIPT + (
        b"POWER BASE\nPOWER THERMAL\nPOWER SHARED\nPOWER DEADLINE MISS\n"
        b"POWER FALLBACK\nPOWER IDLE\nPOWER PASS\n"
    ),
    "extension": BOOT_TRANSCRIPT + (
        b"EXTENSION BEGIN\nEXTENSION PROPOSAL ACCEPTED\nEXTENSION MASK FALLBACK\n"
        b"EXTENSION STALE FALLBACK\nEXTENSION FUEL FALLBACK\nEXTENSION WATCHDOG FALLBACK\n"
        b"EXTENSION EMPTY IDLE\nEXTENSION RESERVED THREE\nEXTENSION PASS\n"
    ),
    "partition": PARTITION_TRANSCRIPT,
    "scheduler": SCHEDULER_TRANSCRIPT,
    "edf": EDF_TRANSCRIPT,
    "fifo": FIFO_TRANSCRIPT,
    "rr": RR_TRANSCRIPT,
    "cbs": CBS_TRANSCRIPT,
    "periodic": PERIODIC_TRANSCRIPT,
    "periodic-preempt": PERIODIC_PREEMPT_TRANSCRIPT,
}
TRANSCRIPTS["smp-single"] = TRANSCRIPTS["smp"]
QUIET_SECONDS = 0.1


def kernel_sha256(path, *, raw=False):
    digest = hashlib.sha256()
    with path.open("rb") as kernel:
        header = kernel.read(4)
        if raw and (not header or header == b"\x7fELF"):
            raise ValueError(f"expected a nonempty raw kernel image at {path}")
        if not raw and header != b"\x7fELF":
            raise ValueError(f"expected an ELF kernel at {path}")
        kernel.seek(0)
        for block in iter(lambda: kernel.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def amc_contract():
    return DynamicTranscript.AMC.observer()


def dynamic_profile(expected):
    if expected is None:  # Existing callers use None for the AMC observer.
        return DynamicTranscript.AMC
    return expected if isinstance(expected, DynamicTranscript) else None


def read_uart(serial, expected):
    # All lines may use CRLF. One extra byte exposes any trailing output.
    serial.seek(0)
    profile = dynamic_profile(expected)
    if profile is not None:
        return serial.read(profile.observer().MAX_BYTES + 1)
    return serial.read(len(expected) + expected.count(b"\n") + 1)


def observe(process, serial, timeout, expected=BOOT_TRANSCRIPT, *, allow_crlf=True, quiet_seconds=QUIET_SECONDS):
    profile = dynamic_profile(expected)
    if profile is not None:
        problem, _ = observe_dynamic(process, serial, timeout, profile, allow_crlf=allow_crlf,
                                     quiet_seconds=quiet_seconds)
        return problem
    deadline = time.monotonic() + timeout
    quiet_until = None
    while True:
        if process.poll() is not None:
            return f"QEMU exited before host stop (status {process.returncode})"
        received = read_uart(serial, expected)
        if allow_crlf:
            received = received.replace(b"\r\n", b"\n")
        # A trailing CR can be the first byte of the next line ending.
        prefix = received[:-1] + b"\n" if allow_crlf and received.endswith(b"\r") else received
        now = time.monotonic()
        if not expected.startswith(prefix):
            return "unexpected UART bytes or marker order"
        if quiet_until is None:
            if now >= deadline:
                return f"UART deadline exceeded ({timeout:g}s)"
            if received == expected:
                quiet_until = now + quiet_seconds
        elif now >= quiet_until:
            return None
        time.sleep(min(0.01, (quiet_until or deadline) - now))


def observe_amc(process, serial, timeout, *, allow_crlf=True, quiet_seconds=QUIET_SECONDS):
    return observe_dynamic(process, serial, timeout, DynamicTranscript.AMC,
                           allow_crlf=allow_crlf, quiet_seconds=quiet_seconds)


def observe_dynamic(process, serial, timeout, profile, *, allow_crlf=True, quiet_seconds=QUIET_SECONDS):
    contract = profile.observer()
    passed = profile.prefix + b" PASS\n"
    failed = profile.prefix + b" FAIL\n"
    stop_line = b"\n" + profile.prefix + b" STOP\n"
    deadline = time.monotonic() + timeout
    quiet_until = None
    accepted = None
    while True:
        if process.poll() is not None:
            return f"QEMU exited before host stop (status {process.returncode})", None
        serial.seek(0)
        raw = serial.read(contract.MAX_BYTES + 1)
        now = time.monotonic()
        if len(raw) > contract.MAX_BYTES:
            return f"{profile.label} UART exceeds byte limit", None
        if accepted is not None:
            if raw != accepted:
                return f"unexpected {profile.label} UART bytes during quiet window", None
            if now >= quiet_until:
                return None, accepted
        else:
            data = raw.replace(b"\r\n", b"\n") if allow_crlf else raw
            # A complete terminal line also exposes extra bytes in the same read.
            # If STOP is followed by another complete line, judge it immediately.
            after_stop = data.partition(stop_line)[2]
            if passed in data or failed in data or b"\n" in after_stop:
                if not data.endswith(b"\n" + passed) or not contract.validate(data):
                    return f"unexpected {profile.label} UART transcript", None
                accepted = raw
                quiet_until = now + quiet_seconds
            if now >= deadline:
                return f"UART deadline exceeded ({timeout:g}s)", None
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


def run_boot(command, timeout, expected=BOOT_TRANSCRIPT, *, allow_crlf=True, quiet_seconds=QUIET_SECONDS):
    profile = dynamic_profile(expected)
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
                    if profile is not None:
                        problem, accepted = observe_dynamic(process, serial, timeout, profile,
                                                            allow_crlf=allow_crlf, quiet_seconds=quiet_seconds)
                    else:
                        problem = observe(process, serial, timeout, expected,
                                          allow_crlf=allow_crlf, quiet_seconds=quiet_seconds)
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
        if profile is not None:
            if uart != accepted:
                raise ValueError(f"unexpected {profile.label} UART bytes during shutdown")
            return
        if allow_crlf:
            uart = uart.replace(b"\r\n", b"\n")
        if uart != expected:
            raise ValueError("unexpected UART bytes during shutdown")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, required=True, help="AArch64 kernel ELF (pi4: raw kernel image)")
    parser.add_argument("--qemu", default="qemu-system-aarch64", help="QEMU executable (default: %(default)s)")
    parser.add_argument("--profile", choices=TRANSCRIPTS, default="boot",
                        help="expected UART transcript (default: %(default)s)")
    parser.add_argument("--runs", type=int, help="fresh sequential QEMU boots (default: boot 2, other profiles 1)")
    parser.add_argument("--timeout", type=float, metavar="SECONDS",
                        help="UART deadline per boot (pi4: 30s plus 1s quiet, others: 5s plus 0.1s quiet)")
    args = parser.parse_args()
    boot = args.profile == "boot"
    pi4 = args.profile in ("pi4", "pi4-scheduler")
    timeout = args.timeout if args.timeout is not None else (30 if pi4 else 5)
    runs = args.runs if args.runs is not None else (2 if boot else 1)
    if runs < 1:
        parser.error("--runs must be positive")
    if not math.isfinite(timeout) or timeout <= 0:
        parser.error("--timeout must be positive and finite")

    try:
        kernel = args.kernel.resolve(strict=True)
        expected_hash = kernel_sha256(kernel, raw=pi4)
        expected = TRANSCRIPTS[args.profile]
        image_type = "raw image" if pi4 else "ELF"
        if pi4:
            command = [args.qemu, "-machine", "raspi4b", "-accel", "tcg"]
        else:
            machine = "virt" if boot else "virt,gic-version=2"
            smp = args.profile in ("smp", "smp-single", "power")
            accelerator = "tcg"
            if smp:
                accelerator = "tcg,thread=single" if args.profile == "smp-single" else "tcg,thread=multi"
                machine += ",secure=off,virtualization=off"
            command = [
                args.qemu, "-machine", machine, "-cpu", "cortex-a72", "-smp", "2" if smp else "1",
                "-accel", accelerator, "-m", "128M", "-nodefaults",
            ]
        command.extend(["-display", "none", "-monitor", "none", "-serial", "stdio", "-kernel", str(kernel)])
        for run in range(1, runs + 1):
            print(f"Run {run}/{runs}  {image_type} SHA-256 = {expected_hash}")
            print(f"  argv = {json.dumps(command, ensure_ascii=True)}", flush=True)
            run_boot(command, timeout, expected, allow_crlf=not pi4,
                     quiet_seconds=1 if pi4 else QUIET_SECONDS)
            if kernel_sha256(kernel, raw=pi4) != expected_hash:
                raise ValueError(f"kernel {image_type} changed during testing")
            print("  PASS (host stopped QEMU after the quiet window)", flush=True)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {runs} cold boot(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
