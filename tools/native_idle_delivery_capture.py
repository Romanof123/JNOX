#!/usr/bin/env python3
"""Capture one archived #340 wake-trace control or one same-image observation.

Run preflight --out NEW_DIR on Linux before either acquisition. Then run
control --kernel ARCHIVE/packages/wake/kernel.elf --out NEW_DIR and
observed --kernel SAME_ELF --out NEW_DIR --plugin PINNED_QEMU_PLUGIN_SPEC.
Use a separately frozen marker plugin configuration. No governor, affinity,
priority, tracing permission or guest configuration changes are made.

Build the observer's SPEC with tools/native_idle_markers.py spec; give its
marker file a fresh path. host.json uses host CLOCK_MONOTONIC nanoseconds.
QMP records are [phase, round, command, args, before_send, after_response,
error]. Sampler rounds are [nominal, began, finished, tasks]; each task read
is [before, after, value, error]. Require both task starttimes to match the
original identity, report sample/drop gaps and do not infer runnable delay
from zero wait counters or from total wall time outside the CPU clock.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import struct
import subprocess
import threading
import time

from ordinary_amc_capture import FLAGS, sha
from qemu_capture import request

KERNEL = "3e5947d2e194e2aab8c83cbe5fdbbb675c2a0b4ab55fb6d484f7f66cc71da0a9"
QEMU = "0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8"
PERIOD_NS = 5_000_000
mono = time.monotonic_ns


def save(path, value):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, separators=(",", ":"))
        stream.write("\n")


def bracket(fn):
    before = mono()
    try:
        value = fn()
    except (OSError, ValueError, IndexError, KeyError) as exc:
        return [before, mono(), None, str(exc)]
    return [before, mono(), value, None]


def identity(task):
    # stat's comm may contain spaces or parentheses. Field 22 is starttime.
    fields = (task / "stat").read_text().rsplit(")", 1)[1].split()
    return int(fields[19])


def switch_counts(task):
    names = ("voluntary_ctxt_switches", "nonvoluntary_ctxt_switches")
    values = {}
    for line in (task / "status").read_text().splitlines():
        key, _, value = line.partition(":")
        if key in names:
            values[key] = int(value)
    return [values[name] for name in names]


def schedstat(task):
    values = [int(v) for v in (task / "schedstat").read_text().split()]
    if len(values) != 3 or min(values) < 0:
        raise ValueError("invalid schedstat triple")
    return values


def sample(pid, tid):
    task = Path(f"/proc/{pid}/task/{tid}")
    return {"tid": tid, "start_before": bracket(lambda: identity(task)),
            "schedstat": bracket(lambda: schedstat(task)),
            "switches": bracket(lambda: switch_counts(task)),
            "start_after": bracket(lambda: identity(task))}


class Bounded:
    def __init__(self, capacity):
        self.capacity = capacity
        self.total = 0
        self.rows = []

    def add(self, row):
        self.total += 1
        if len(self.rows) < self.capacity:
            self.rows.append(row)

    def export(self):
        return {"total": self.total, "dropped": self.total - len(self.rows), "rows": self.rows}


class Observer:
    def __init__(self, pid):
        self.pid = pid
        self.cpu_tid = None
        self.commands = Bounded(8192)
        self.samples = Bounded(24000)
        self.stop = threading.Event()
        self.thread = None
        self.cost_ns = 0
        self.skipped = 0
        self.error = None

    def call(self, pipe, phase, seq, command, **arguments):
        start = mono()
        error = None
        try:
            return request(pipe, command, **arguments)
        except Exception as exc:
            error = str(exc)
            raise
        finally:
            self.commands.add([phase, seq, command, arguments, start, mono(), error])

    def start(self, pipe):
        cpus = self.call(pipe, "setup", None, "query-cpus-fast")
        if len(cpus) != 1 or cpus[0]["cpu-index"] != 0:
            raise ValueError("expected CPU0 only")
        self.cpu_tid = int(cpus[0]["thread-id"])
        self.thread = threading.Thread(target=self.run, name="refill-sampler")
        self.thread.start()

    def run(self):
        cpu_start = time.thread_time_ns()
        deadline = mono()
        try:
            while not self.stop.is_set():
                start = mono()
                per_task = [sample(self.pid, tid) for tid in dict.fromkeys((self.pid, self.cpu_tid))]
                self.samples.add([deadline, start, mono(), per_task])
                deadline += PERIOD_NS
                now = mono()
                if deadline <= now:
                    skipped = (now - deadline) // PERIOD_NS + 1
                    self.skipped += skipped
                    deadline += skipped * PERIOD_NS
                self.stop.wait(max(0, deadline - mono()) / 1e9)
        except Exception as exc:
            self.error = repr(exc)
        finally:
            self.cost_ns = time.thread_time_ns() - cpu_start

    def finish(self, out):
        self.stop.set()
        if self.thread is not None:
            self.thread.join()
        save(out / "host.json", {"version": 1, "clock": "CLOCK_MONOTONIC ns",
             "qemu_pid_main_tid": self.pid, "cpu0_tid": self.cpu_tid, "period_ns": PERIOD_NS,
             "sampler_cpu_ns": self.cost_ns, "skipped_periods": self.skipped, "error": self.error,
             "qmp": self.commands.export(), "samples": self.samples.export()})


def preflight(out):
    pid, tid = os.getpid(), threading.get_native_id()
    widths = [bracket(time.thread_time_ns) for _ in range(64)]
    before = sample(pid, tid)
    # Exercise actual accounted CPU work without changing scheduling policy.
    block = bytes(1 << 20)
    digest = hashlib.sha256()
    for _ in range(32):
        digest.update(block)
    active = sample(pid, tid)
    time.sleep(0.015)
    slept = sample(pid, tid)
    save(out / "preflight.json", {"platform": platform.platform(), "uid": os.getuid(),
         "monotonic": vars(time.get_clock_info("monotonic")), "thread": vars(time.get_clock_info("thread_time")),
         "clock_brackets": widths, "calibration_digest": digest.hexdigest(),
         "before": before, "after_cpu": active, "after_sleep": slept,
         "runqueue_limit": "zero/uncalibrated runnable wait cannot establish absence of contention"})
    snapshots = (before, active, slept)
    if any(row[key][3] for row in snapshots for key in ("start_before", "start_after", "schedstat", "switches")):
        raise ValueError("incomplete proc accounting, see preflight.json")
    if len({row[key][2] for row in snapshots for key in ("start_before", "start_after")}) != 1:
        raise ValueError("unstable task identity")
    if active["schedstat"][2][0] <= before["schedstat"][2][0]:
        raise ValueError("task CPU accounting did not advance")


def capture(args, out):
    kernel = args.kernel.resolve(strict=True)
    qemu = Path("/usr/bin/qemu-system-aarch64")
    if sha(kernel) != KERNEL or sha(qemu) != QEMU:
        raise ValueError("archive kernel or QEMU digest mismatch")
    table = subprocess.check_output(["nm", "-Sn", kernel], text=True)
    (out / "symbols").write_text(table)

    def address(suffix):
        lines = [line for line in table.splitlines() if suffix in line]
        if len(lines) != 1:
            raise ValueError(f"ambiguous or missing symbol {suffix}")
        return int(lines[0].split()[0], 16)

    base, end = address(" _start"), address(" __stack_top")
    spaces, frames = address("6SPACES"), address("6FRAMES")
    ranges = [(spaces + i * 0x415000 + 0x7000, 128) for i in range(3)] + [(frames, 2496)]
    argv = [str(qemu), *FLAGS, "-qmp", "stdio", "-serial", "file:" + str(out / "uart"),
            "-kernel", str(kernel), "-d", "int", "-D", str(out / "exceptions")]
    metadata = {"argv": argv, "qemu": QEMU, "kernel": KERNEL, "mode": args.mode,
                "platform": platform.platform(), "pid": os.getpid(), "cwd": str(Path.cwd())}
    if args.mode == "observed":
        plugin_file = Path(args.plugin.split(",", 1)[0]).resolve(strict=True)
        metadata["plugin_sha256"] = sha(plugin_file)
        argv.extend(["-plugin", args.plugin])
    save(out / "launch.json", metadata)

    with (out / "stderr").open("xb") as err:
        process = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=err, bufsize=0)
        observer = Observer(process.pid) if args.mode == "observed" else None
        # These bound methods retain the archived unbuffered QMP pipe.
        pipe = type("Pipe", (), {"write": process.stdin.write, "readline": process.stdout.readline})

        def call(phase, seq, command, **kw):
            if observer is None:
                return request(pipe, command, **kw)
            return observer.call(pipe, phase, seq, command, **kw)

        result = {"done": False, "rounds": 0}
        # The archived 105 s acquisition gate still governs normal rounds.
        # Bound a broken QMP pipe which never sends a reply, including during
        # terminal extraction, so failure evidence can still be written.
        def watchdog(_signum, _frame):
            raise TimeoutError("outer 150 s capture watchdog")

        prior_handler = signal.signal(signal.SIGALRM, watchdog)
        signal.alarm(150)
        try:
            if "QMP" not in json.loads(process.stdout.readline()):
                raise ValueError("missing QMP greeting")
            call("setup", None, "qmp_capabilities")
            if observer:
                observer.start(pipe)
            start = time.monotonic()
            seq = 0
            while True:
                uart = (out / "uart").read_bytes()
                exits = re.findall(rb"NATIVE EL0 EXIT PID \d+ -?\d+\r?\n", uart)
                done = len(exits) == 3 and b"NATIVE HIERARCHY HEALTH END 1\n" in uart.replace(b"\r", b"")
                if done or time.monotonic() - start > 105 or b"NATIVE EL0 FAULT" in uart:
                    break
                with (out / "samples").open("ab") as raw:
                    raw.write(struct.pack("<d", time.monotonic() - start))
                    for addr, size in ranges:
                        call("running", seq, "pmemsave", val=addr, size=size, filename=str(out / "scratch"))
                        raw.write((out / "scratch").read_bytes())
                seq += 1
                result["rounds"] = seq
                time.sleep(.09)
            call("terminal", None, "stop")
            regs = call("terminal", None, "human-monitor-command", **{"command-line": "info registers"})
            (out / "regs").write_text(regs)
            call("terminal", None, "pmemsave", val=base, size=end-base, filename=str(out / "ram.bin"))
            result.update(done=done, elapsed=time.monotonic() - start)
        except BaseException as exc:
            result["error"] = repr(exc)
            raise
        finally:
            signal.alarm(0)
            signal.signal(signal.SIGALRM, prior_handler)
            try:
                if observer:
                    observer.finish(out)
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                process.stdin.close()
                process.stdout.close()
                save(out / "outcome.json", result)
        if not result["done"]:
            raise ValueError("incomplete guest terminal evidence")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("preflight", "control", "observed"))
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--kernel", type=Path)
    parser.add_argument("--plugin", help="full stock QEMU -plugin argument; observed arm only")
    args = parser.parse_args()
    if platform.system() != "Linux":
        parser.error("requires Linux /proc and POSIX QMP pipes")
    if bool(args.kernel) != (args.mode != "preflight") or bool(args.plugin) != (args.mode == "observed"):
        parser.error("control and observed need --kernel; only observed needs --plugin")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    if args.mode == "preflight":
        preflight(out)
    else:
        capture(args, out)


if __name__ == "__main__":
    main()
