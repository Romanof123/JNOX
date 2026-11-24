"""Reduce one acquisition directory. Output exact rational quantities as strings."""
import argparse
import hashlib
import json
import re
import sys
from fractions import Fraction
from pathlib import Path

NS = 10**9
MAX_BYTES = 4 * 1024**2


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_fields(pairs):
    obj = {}
    for key, value in pairs:
        require(key not in obj, f"duplicate key {key}")
        obj[key] = value
    return obj


def read_file(path):
    require(path.is_file(), f"expected regular file {path.name}")
    with path.open("rb") as stream:
        data = stream.read(MAX_BYTES + 1)
    require(len(data) <= MAX_BYTES, "input exceeds 4 MiB")
    return data, hashlib.sha256(data).hexdigest()


def decode(data):
    return json.loads(data.decode("utf-8"), object_pairs_hook=unique_fields)


def field(obj, name):
    require(isinstance(obj, dict) and name in obj, f"missing {name}")
    return obj[name]


def string(obj, name):
    value = field(obj, name)
    require(isinstance(value, str) and value.strip(), f"invalid {name}")
    return value


def integer(value, name):
    require(type(value) is int and abs(value) < 2**64, f"invalid integer {name}")
    return value


def source(directory, manifest, key):
    ref = field(manifest, key)
    name = string(ref, "file")
    require(re.fullmatch(r"[A-Za-z0-9_-][A-Za-z0-9_.-]*", name), "invalid source filename")
    path = directory / name
    require(path.resolve().parent == directory, "source outside acquisition directory")
    data, digest = read_file(path)
    require(digest == string(ref, "sha256"), f"{key} SHA-256 mismatch")
    return decode(data)


def integrate(points, start, end, gap):
    covered = Fraction(0)
    energy = Fraction(0)
    for (t, p), (u, q) in zip(points, points[1:]):
        left, right = max(start, t), min(end, u)
        if right <= left or u - t > gap:
            continue
        slope = Fraction(q - p, u - t)
        mean = p + slope * ((left - t) + (right - t)) / 2
        covered += right - left
        energy += mean * (right - left) / 10**21  # microvolts times microamps (pW), integrated in ns
    return covered, energy


def reduce_record(path):
    raw, digest = read_file(path)
    manifest = decode(raw)
    producer = string(manifest, "producer")
    kind = string(manifest, "kind")
    require(kind in ("fixture", "board"), "invalid kind")
    directory = path.resolve().parent
    events = source(directory, manifest, "events")
    samples = source(directory, manifest, "samples")
    context = {k: string(events, k) for k in (
        "board", "firmware", "workload", "cpu_opp", "thermal", "shared_load",
        "environment", "idle_state", "instrument", "trigger", "limits")}
    def number(name):
        return integer(field(events, name), name)

    hz, origin, edge = number("counter_hz"), number("trigger_tick"), number("trigger_ns")
    uncertainty, gap = number("sync_uncertainty_ns"), number("max_gap_ns")
    require(hz > 0 and uncertainty >= 0 and gap > 0, "invalid frequency, uncertainty or gap")
    def time(name):
        return edge + Fraction((number(name + "_tick") - origin) * NS, hz)

    release, wake, first, end, deadline = map(time, ("release", "wake", "first", "complete", "deadline"))
    idle_start, idle_end = time("idle_start"), time("idle_end")
    require(release <= wake <= first <= end and release < end and deadline >= release, "invalid event order")
    require(idle_start < idle_end and (idle_end <= release or end <= idle_start), "invalid idle window")
    require(isinstance(samples, list) and len(samples) <= 100000, "invalid samples array or count")
    points = []
    for row in samples:
        require(isinstance(row, list) and len(row) == 3, "expected [ns, uV, uA]")
        t, v, i = (integer(n, "sample") for n in row)
        require(not points or t > points[-1][0], "sample time must strictly increase")
        points.append((t, v * i))

    report = dict(kind=kind, producer=producer, context=context, manifest_sha256=digest,
                  events=manifest["events"], samples=manifest["samples"], counter_hz=hz,
                  sync_uncertainty_ns=uncertainty, max_gap_ns=gap, sample_count=len(points),
                  wake_s=str((first-wake)/NS), response_s=str((end-release)/NS),
                  deadline_margin_s=str((deadline-end)/NS), observed_miss=end > deadline)
    energies = {}
    coverage = {}
    for name, a, b in (("run", release, end), ("idle", idle_start, idle_end)):
        covered, energy = integrate(points, a, b, gap)
        coverage[name] = dict(start_ns=str(a), end_ns=str(b), covered_ns=str(covered),
                              required_ns=str(b-a), complete=covered == b-a)
        energies[name] = energy
    report["coverage"] = coverage
    if all(item["complete"] for item in coverage.values()):
        baseline = energies["idle"] * NS / (idle_end-idle_start)
        report.update(status="complete", run_J=str(energies["run"]), idle_J=str(energies["idle"]),
                      idle_mean_W=str(baseline), adjusted_J=str(energies["run"]-baseline*(end-release)/NS))
    else:
        report["status"] = "incomplete sample coverage"
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    try:
        report = reduce_record(parser.parse_args().manifest)
    except (OSError, ValueError, RecursionError) as error:
        print(f"invalid acquisition: {error}", file=sys.stderr)
        return 2
    print(json.dumps(report, sort_keys=True))
    return 0 if report["status"] == "complete" else 1


if __name__ == "__main__":
    sys.exit(main())
