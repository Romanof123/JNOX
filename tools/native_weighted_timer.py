"""Optional first-cold #184 timer observations for the pinned Ubuntu QEMU."""
import os
from pathlib import Path
import platform
import re
import time

from native_el0_replay import need
from ordinary_amc_capture import save
from qemu_capture import request

PIN = "0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8"
EVENTS = (
    "arm_gt_cval_write",
    "arm_gt_ctl_write",
    "arm_gt_recalc",
    "gic_set_irq",
    "gic_update_set_irq",
    "gic_acknowledge_irq",
)
LINE = re.compile(rb"(\d+)@(\d+)\.(\d{6}):([a-z0-9_]+) (.*)")
CVAL = re.compile(rb"gt_cval_write: timer 0 value (0x[0-9a-f]+)")


class HostObserver:
    def __init__(self, root):
        self.root = root
        self.samples = []
        self.cpu = None
        self.tid = None

    def start(self, pipe, pid):
        cpus = request(pipe, "query-cpus-fast")
        need(len(cpus) == 1 and cpus[0]["cpu-index"] == 0, "one diagnostic vCPU")
        self.cpu = cpus[0]
        self.tid = self.cpu["thread-id"]
        self.task = Path(f"/proc/{pid}/task/{self.tid}")
        self.affinity = sorted(os.sched_getaffinity(self.tid))
        self.sample()
        request(pipe, "cont")

    def sample(self):
        wall0, mono0 = time.time_ns(), time.monotonic_ns()
        try:
            stats = list(map(int, (self.task / "schedstat").read_text().split()))
        except (OSError, ValueError):
            stats = None
        mono1, wall1 = time.monotonic_ns(), time.time_ns()
        self.samples.append([wall0, mono0, stats, mono1, wall1])

    def finish(self):
        save(
            self.root / "host-timing.json",
            dict(
                version=1,
                os=platform.release(),
                cpu=self.cpu,
                affinity=getattr(self, "affinity", None),
                samples=self.samples,
            ),
        )


def split_log(raw):
    plain = []
    records = []
    for index, line in enumerate(raw.splitlines(keepends=True)):
        value = line.rstrip(b"\r\n")
        match = LINE.fullmatch(value)
        if match:
            tid, sec, micro, name, args = match.groups()
            name = name.decode("ascii")
            need(name in EVENTS, "unrequested timestamped QEMU event")
            records.append(
                (index, int(tid), int(sec) * 1_000_000 + int(micro), name, args)
            )
        else:
            plain.append(line)
            if re.fullmatch(rb"Taking exception 5 \[IRQ\](?: on CPU 0)?", value):
                records.append((index, None, None, "irq_entry", b""))
    return b"".join(plain), records


def timing_report(records, grants, host):
    need(host["version"] == 1 and host["cpu"]["cpu-index"] == 0, "vCPU timing identity")
    tid = host["cpu"]["thread-id"]
    starts = []
    for i, event in enumerate(records):
        match = CVAL.fullmatch(event[4]) if event[3] == "arm_gt_cval_write" else None
        if match:
            starts.append((i, int(match[1], 16)))
    need(
        [c for _, c in starts] == [g["compare"] for g in grants],
        "CVAL writes vs all closed grants",
    )
    estimates = []
    for j, ((i, c), g) in enumerate(zip(starts, grants)):
        if g["cause"] != "irq":
            continue
        segment = records[i : starts[j + 1][0] if j + 1 < len(starts) else len(records)]

        def one(name, pattern):
            hits = [r for r in segment if r[3] == name and re.fullmatch(pattern, r[4])]
            return hits[0] if len(hits) == 1 else None

        signal = one("gic_set_irq", rb"irq 30 level 1 cpumask 0x1 target 0x[0-9a-f]+")
        cpu_line = one("gic_update_set_irq", rb"cpu\[0\]: irq = 1")
        recalc = one(
            "arm_gt_recalc", rb"gt recalc: timer 0 irqstate 1 next tick 0x[0-9a-f]+"
        )
        ack = one("gic_acknowledge_irq", rb"cpu 0 acknowledged irq 30")
        interrupt = one("irq_entry", rb"")
        off = one("arm_gt_ctl_write", rb"gt_ctl_write: timer 0 value 0x0")
        points = dict(
            cval=segment[0],
            gic_ppi=signal,
            cpu_irq_line=cpu_line,
            recalc_active=recalc,
            handler_disable=off,
            handler_iar=ack,
        )
        item = dict(
            lease=g["lease"],
            pid=g["pid"],
            guest_R=g["start"],
            guest_C=c,
            guest_I=g["stop"],
            guest_I_minus_C=g["stop"] - c,
            host_points={
                k: ({"tid": e[1], "wall_us": e[2], "log_line": e[0] + 1} if e else None)
                for k, e in points.items()
            },
        )

        def interval(start, end):
            a, b = points[start], points[end]
            if not a or not b or b[0] < a[0] or b[2] < a[2]:
                return None
            delta = b[2] - a[2]
            return [max(0, delta - 1), delta + 1]

        item["cval_to_ppi_us"] = interval("cval", "gic_ppi")
        item["ppi_to_cpu_line_us"] = interval("gic_ppi", "cpu_irq_line")
        item["ppi_to_iar_us"] = interval("gic_ppi", "handler_iar")
        if not (
            signal
            and interrupt
            and off
            and signal[0] < interrupt[0] < off[0]
            and off[1] == tid
        ):
            item["ppi_to_handler_disable_us"] = None
        else:
            item["ppi_to_handler_disable_us"] = interval("gic_ppi", "handler_disable")
        item["irq_log_line"] = interrupt[0] + 1 if interrupt else None
        estimates.append(item)
    samples = host["samples"]
    valid = [s for s in samples if s[2] is not None and len(s[2]) == 3]
    counters = None
    if len(valid) >= 2:
        first, last = valid[0], valid[-1]
        diffs = [b - a for a, b in zip(first[2], last[2])]
        if min(diffs) >= 0:
            counters = dict(
                run_ns=diffs[0],
                runqueue_ns=diffs[1],
                timeslices=diffs[2],
                sample_count=len(valid),
                first_wall_bounds_ns=[first[0], first[4]],
                last_wall_bounds_ns=[last[0], last[4]],
            )
    return dict(
        vcpu_tid=tid,
        host_schedstat=counters,
        irqs=estimates,
        limits="Host wall timestamp intervals include tracing overhead and assume no wall-clock adjustment within each interval. Guest counter and host wall are reported separately. Callback entry, exact CPU acceptance and per-lease off-CPU episodes are unobserved.",
    )
