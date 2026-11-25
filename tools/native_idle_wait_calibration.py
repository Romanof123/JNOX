#!/usr/bin/env python3
"""Optional #343 host calibration: python3 tools/native_idle_wait_calibration.py --out NEW_DIR.

Only two disposable calibration children are pinned, to one CPU already allowed
to the invoking process. The parent, QEMU and applications retain their own
settings. Run once before acquisitions; keep failures and insufficient records.
Supply NEW_DIR/wait-calibration.json to native_idle_delivery_report.py
--wait-calibration. Positive evidence qualifies these Linux accounting fields
on this boot. It cannot date when an individual queued wait began, distinguish
Windows host suspension, or give a per-IRQ upper bound on post-deadline delay.
"""
import argparse
import hashlib
import multiprocessing as mp
import os
from pathlib import Path
import platform
import signal
import time

from native_idle_delivery_capture import bracket, sample, save

MS = 1_000_000


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def snapshot():
    pid = os.getpid()
    return {"cpu_before": bracket(time.thread_time_ns), "proc": sample(pid, pid),
            "cpu_after": bracket(time.thread_time_ns)}


def actor(pipe, cpu):
    try:
        # Enforce an additional CPU-time ceiling even if the work loop fails.
        signal.signal(signal.SIGPROF, signal.SIG_DFL)
        signal.setitimer(signal.ITIMER_PROF, 0.50)
        original = sorted(os.sched_getaffinity(0))
        os.sched_setaffinity(0, {cpu})
        record = {"pid": os.getpid(), "original": original, "pinned": sorted(os.sched_getaffinity(0)),
                  "nice": os.getpriority(os.PRIO_PROCESS, 0), "policy": os.sched_getscheduler(0)}
        record["blocked_start"] = snapshot()
        time.sleep(0.040)
        record["blocked_end"] = snapshot()
        pipe.send(("ready", record))
        if pipe.recv() != "run":
            return
        record["active_start"] = snapshot()
        begin = time.thread_time_ns()
        digest = hashlib.sha256()
        block = bytes(16384)
        batches = 0
        while time.thread_time_ns() - begin < 60 * MS:
            digest.update(block)
            batches += 1
        record["active_end"] = snapshot()
        record.update(batches=batches, digest=digest.hexdigest())
        pipe.send(("done", record))
    except Exception as exc:
        pipe.send(("error", repr(exc)))
    finally:
        pipe.close()


def launch(cpu, records):
    ctx = mp.get_context("fork")
    children = []
    try:
        for _ in range(2):
            parent, child = ctx.Pipe()
            process = ctx.Process(target=actor, args=(child, cpu))
            process.start()
            child.close()
            children.append((process, parent))
        for phase in ("ready", "done"):
            for index, (process, pipe) in enumerate(children):
                require(pipe.poll(3), f"actor {index} timeout at {phase}")
                tag, record = pipe.recv()
                require(tag == phase, f"actor {index}: {record}")
                require(record["pid"] == process.pid, "child identity mismatch")
                if phase == "ready":
                    records.append(record)
                else:
                    records[index] = record
            if phase == "ready":
                for _, pipe in children:
                    pipe.send("run")
    finally:
        for process, pipe in children:
            pipe.close()
            process.join(timeout=0.2)
            if process.is_alive():
                process.kill()
                process.join()


def inspect(snapshot, pid):
    p = snapshot["proc"]
    require(p["tid"] == pid, "wrong measured TID")
    fields = [snapshot["cpu_before"], p["start_before"], p["schedstat"], p["switches"],
              p["start_after"], snapshot["cpu_after"]]
    for field in fields:
        require(isinstance(field, list) and len(field) == 4 and field[3] is None
                and type(field[0]) is int and type(field[1]) is int and 0 <= field[0] <= field[1],
                "failed/unordered read")
    require(all(a[1] <= b[0] for a, b in zip(fields, fields[1:])), "overlapping reads")
    identity = fields[1][2]
    require(type(identity) is int and identity > 0 and identity == fields[4][2], "reused TID")
    counters, switches = fields[2][2], fields[3][2]
    require(len(counters) == 3 and len(switches) == 2 and
            all(type(v) is int and v >= 0 for v in [*counters, *switches]), "bad /proc sample")
    c0, c1 = fields[0][2], fields[-1][2]
    require(type(c0) is int and type(c1) is int and 0 <= c0 <= c1, "bad thread CPU clock")
    return identity, fields[0][0], fields[-1][1], c0, c1, counters, switches


def interval(a, b):
    require(a[0] == b[0] and a[2] <= b[1] and a[4] <= b[3], "bad interval chronology")
    wall_lo, wall_hi = b[1] - a[2], b[2] - a[1]
    cpu_lo, cpu_hi = b[3] - a[4], b[4] - a[3]
    deltas = [end - start for start, end in zip(a[5] + a[6], b[5] + b[6])]
    require(min(deltas) >= 0 and cpu_hi <= wall_hi + MS, "counter discontinuity")
    return {"wall_ns": [wall_lo, wall_hi], "cpu_ns": [cpu_lo, cpu_hi],
            "outside_cpu_ns": [max(0, wall_lo-cpu_hi), max(0, wall_hi-cpu_lo)],
            "exec_ns": deltas[0], "wait_ns": deltas[1], "slices": deltas[2],
            "voluntary": deltas[3], "involuntary": deltas[4]}


def validate_wait_calibration(packet, platform_name):
    """Derive both control decisions exclusively from ordered raw samples."""
    require(packet["version"] == 1 and packet["platform"] == platform_name and
            packet["error"] is None, "unavailable or different platform")
    allowed = packet["allowed_before"]
    require(allowed and len(set(allowed)) == len(allowed) and allowed == packet["allowed_after"] and
            len(allowed) == packet["allowed_count"] and packet["cpu"] in allowed, "invalid parent affinity")
    records = packet["actors"]
    require(len(records) == 2 and len({r["pid"] for r in records}) == 2, "need two distinct actors")
    interior = []
    results = []
    for r in records:
        require(r["pid"] != packet["parent_pid"] and r["original"] == allowed and r["pinned"] == [packet["cpu"]]
                and r["nice"] == packet["nice"] and r["policy"] == packet["policy"], "wrong actor settings")
        require(r["batches"] > 0, "no work")
        a, b, c, d = [inspect(r[k], r["pid"]) for k in ("blocked_start", "blocked_end", "active_start", "active_end")]
        require(len({a[0], b[0], c[0], d[0]}) == 1 and b[2] <= c[1], "incarnation/stage mismatch")
        blocked, active = interval(a, b), interval(c, d)
        require(blocked["voluntary"] >= 1 and blocked["outside_cpu_ns"][0] >= 30 * MS
                and blocked["wait_ns"] <= blocked["outside_cpu_ns"][0] // 4, "sleep control inconclusive")
        require(active["cpu_ns"][0] >= 50 * MS and active["exec_ns"] >= 50 * MS and active["involuntary"] >= 1
                and active["wait_ns"] >= 10 * MS and active["slices"] >= 2, "runnable control inconclusive")
        require(active["wait_ns"] <= active["outside_cpu_ns"][1] + MS, "wait exceeds outside-CPU envelope")
        interior.append((c[2], d[1]))
        results.append({"pid": r["pid"], "starttime": a[0], "blocked": blocked, "runnable": active})
    overlap = min(v[1] for v in interior) - max(v[0] for v in interior)
    require(overlap >= 20 * MS, "insufficient simultaneous measurement")
    return {"actors": results, "minimum_overlap_ns": overlap}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    out = parser.parse_args().out
    out.mkdir(parents=True, exist_ok=False)
    record = {"version": 1, "platform": platform.platform(), "parent_pid": os.getpid(),
              "uid": os.getuid(), "actors": [], "error": None, "status": "unavailable",
              "logical_cpus": os.cpu_count()}
    try:
        record["boot_id"] = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
        record["nice"] = os.getpriority(os.PRIO_PROCESS, 0)
        record["policy"] = os.sched_getscheduler(0)
        allowed = sorted(os.sched_getaffinity(0))
        record.update(allowed_before=allowed, allowed_count=len(allowed), cpu=min(allowed))
        launch(record["cpu"], record["actors"])
        record["allowed_after"] = sorted(os.sched_getaffinity(0))
    except (OSError, ValueError, EOFError) as exc:
        record["error"] = repr(exc)
    if record["error"] is None:
        try:
            record["derived"] = validate_wait_calibration(record, record["platform"])
            record["status"] = "qualified"
        except (KeyError, IndexError, TypeError, ValueError) as exc:
            record.update(status="insufficient", reason=str(exc))
    save(out / "wait-calibration.json", record)
    return 0 if record["status"] == "qualified" else 1


if __name__ == "__main__":
    raise SystemExit(main())
