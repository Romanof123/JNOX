"""Capture/replay the #184 single-CPU host fixture with the Python standard library."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time


CASES = [f"empty{i}" for i in range(32)] + ["cpu", "blocked", "contended"]
MS = 1_000_000


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")


def capture(fixture, out):
    if platform.system() != "Linux" or os.getuid() == 0 or os.geteuid() == 0:
        raise ValueError("requires a non-root Linux process")
    if os.sched_getscheduler(0) != os.SCHED_OTHER or os.getpriority(os.PRIO_PROCESS, 0) != 0:
        raise ValueError("requires inherited SCHED_OTHER, nice 0")
    allowed = sorted(os.sched_getaffinity(0))
    cpu = allowed[0]
    taskset = shutil.which("taskset")
    if not taskset:
        raise ValueError("taskset is required")
    fixture = fixture.resolve(strict=True)
    argv = [taskset, "--cpu-list", str(cpu), str(fixture), str(cpu)]
    out.mkdir(parents=True, exist_ok=False)
    manifest = dict(schema=1, argv=argv, allowed=allowed, cpu=cpu, uid=os.getuid(),
                    kernel=platform.release(), machine=platform.machine(), python=sys.version,
                    fixture_sha256=digest(fixture), taskset_sha256=digest(Path(taskset)),
                    watchdog_seconds=5, utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    write_json(out / "manifest.json", manifest)
    manifest["begin_mono_ns"] = time.monotonic_ns()
    with (out / "stdout.txt").open("wb") as stdout, (out / "stderr.txt").open("wb") as stderr:
        try:
            proc = subprocess.run(argv, timeout=5, stdout=stdout, stderr=stderr, check=False)
            manifest.update(returncode=proc.returncode, timed_out=False)
        except subprocess.TimeoutExpired:
            manifest.update(returncode=None, timed_out=True)
    manifest.update(end_mono_ns=time.monotonic_ns(), stdout_sha256=digest(out / "stdout.txt"),
                    stderr_sha256=digest(out / "stderr.txt"))
    write_json(out / "manifest.json", manifest)


def parse(raw, cpu):
    lines = iter(raw.splitlines())

    def row(tag, fields):
        words = next(lines, "").split()
        if len(words) != fields + 1 or words[0] != tag:
            raise ValueError(f"expected {tag} with {fields} fields: {words}")
        return words[1:]

    def thread(role):
        label, tid, affinity, policy, nice = row("thread", 5)
        if label != role or int(tid) <= 0 or affinity != str(cpu) or (policy, nice) != ("0", "0"):
            raise ValueError(f"thread identity mismatch: {label} {tid} {affinity} {policy} {nice}")
        return int(tid)

    tid = thread("measured")
    rw, rc = map(int, row("resolution", 2))
    if thread("companion") == tid:
        raise ValueError("duplicate thread ID")
    pairs = []
    for name in CASES:
        ends = []
        for side in ("start", "end"):
            label, boundary, w0, c0, run, wait, slices, c1, w1 = row("sample", 9)
            if (label, boundary) != (name, side):
                raise ValueError("sample identity mismatch")
            fields = (run, wait, slices)
            sched = None if fields == ("NA", "NA", "NA") else tuple(map(int, fields))
            ends.append((int(w0), int(c0), sched, int(c1), int(w1)))
        label, v0, v1, i0, i1, progress = row("activity", 6)
        if label != name:
            raise ValueError("activity identity mismatch")
        activity = list(map(int, (v0, v1, i0, i1, progress)))
        if min(activity) < 0 or activity[1] < activity[0] or activity[3] < activity[2]:
            raise ValueError("invalid activity counters")
        pairs.append((name, ends[0], ends[1], activity))
    row("complete", 0)
    if list(lines):
        raise ValueError("unexpected trailing rows")
    return rw, rc, pairs


def score(raw, cpu):
    rw, rc, pairs = parse(raw, cpu)
    clock_errors, queue_errors, intervals = [], [], {}
    if not (0 < rw <= MS // 10 and 0 < rc <= MS // 10):
        clock_errors.append("clock resolution")
    last_wall = last_cpu = 0
    last_sched = None
    for name, a, b, (v0, v1, i0, i1, progress) in pairs:
        for w0, c0, sched, c1, w1 in (a, b):
            if not (last_wall <= w0 <= w1 and last_cpu <= c0 <= c1):
                clock_errors.append(name + " nonmonotonic sample")
            if max(w1 - w0, c1 - c0) > MS:
                clock_errors.append(name + " boundary exceeds 1 ms")
            last_wall, last_cpu = w1, c1
            if sched is None:
                queue_errors.append(name + " unavailable schedstat")
            elif min(sched) < 0 or (last_sched and any(x < y for x, y in zip(sched, last_sched))):
                queue_errors.append(name + " invalid schedstat")
            last_sched = sched
        wall = [max(0, b[0] - a[4] - 2 * rw), b[4] - a[0] + 2 * rw]
        active = [max(0, b[1] - a[3] - 2 * rc), b[3] - a[1] + 2 * rc]
        rate = (wall[1] + 999) // 1000
        off = [max(0, wall[0] - active[1] - rate), max(0, wall[1] - active[0] + rate)]
        if active[0] > wall[1] + rate:
            clock_errors.append(name + " inconsistent clocks")
        deltas = [y - x for x, y in zip(a[2], b[2])] if a[2] and b[2] else None
        intervals[name] = dict(wall_ns=wall, cpu_ns=active, outside_cpu_ns=off,
                               schedstat_delta=deltas, voluntary=v1-v0, involuntary=i1-i0,
                               peer_batches=progress)
        if name.startswith("empty"):
            if wall[1] > 2 * MS:
                clock_errors.append(name + " empty-pair overhead")
            continue
        if name in ("cpu", "contended") and active[0] < 39 * MS:
            clock_errors.append(name + " CPU below 39 ms")
        if name == "blocked" and (off[0] < 70 * MS or active[1] > 5 * MS or v1 <= v0):
            clock_errors.append("voluntary blocking control")
        if name == "contended" and (off[0] < 10 * MS or i1 <= i0 or progress <= 0):
            clock_errors.append("runnable contention control")
        if deltas is not None:
            runtime, wait, slices = deltas
            if not (active[0] - MS <= runtime <= active[1] + MS):
                queue_errors.append(name + " stale or inconsistent proc runtime")
            if wait > off[1] + MS:
                queue_errors.append(name + " proc wait above outside-CPU envelope")
            if name == "blocked" and wait > 5 * MS:
                queue_errors.append("blocked proc wait exceeds 5 ms")
            if name == "contended" and (wait < 10 * MS or slices < 1):
                queue_errors.append("contention proc wait or slice count insufficient")
    return dict(clock_controls_pass=not clock_errors, queue_controls_pass=not (clock_errors or queue_errors),
                clock_findings=clock_errors, queue_findings=queue_errors, clock_resolution_ns=[rw, rc],
                max_empty_wall_upper_ns=max(intervals[n]["wall_ns"][1] for n in CASES[:32]),
                intervals=intervals)


def replay(out):
    manifest = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
    for channel in ("stdout", "stderr"):
        if digest(out / (channel + ".txt")) != manifest[channel + "_sha256"]:
            raise ValueError(channel + " digest mismatch")
    if manifest["timed_out"] or manifest["returncode"] != 0:
        raise ValueError(f"incomplete capture: timeout={manifest['timed_out']} status={manifest['returncode']}")
    return score((out / "stdout.txt").read_text(encoding="utf-8"), manifest["cpu"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_subparsers(dest="mode", required=True)
    run = modes.add_parser("capture")
    run.add_argument("fixture", type=Path)
    run.add_argument("out", type=Path)
    modes.add_parser("replay").add_argument("out", type=Path)
    args = parser.parse_args()
    try:
        if args.mode == "capture":
            capture(args.fixture, args.out)
        result = replay(args.out)
        if args.mode == "capture":
            write_json(args.out / "result.json", result)
        print(json.dumps(result, indent=2))
        return 0 if result["queue_controls_pass"] else 1
    except (OSError, ValueError, KeyError) as exc:
        print(f"unqualified capture: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
