"""Recalculate cadence and wake lateness from the four frozen #449 timing CSVs."""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import zipfile


ARCHIVE_SHA256 = "98b001ff9f7e38ee7b869df9aaa3fd2f3e7cc8917ae5b6112b74a0f396ed1cf8"
PERIOD_NS = 10_000_000
GAP_LIMIT_NS = 20_000_000
SAMPLES = 4096
CASES = ("restart-quit", "primary-exit", "spare-fault", "headless")


def read_csv(archive, entries, case):
    entry = entries[f"449-build-02/all-samples/{case}.csv"]
    # Each frozen CSV occupies a single nonzero content-addressed block.
    key, = entry["chunks"]
    data = archive.read("objects/" + key)
    if len(data) != entry["size"] or hashlib.sha256(data).hexdigest() != entry["sha256"]:
        raise ValueError(f"{case}: CSV identity mismatch")
    return csv.DictReader(io.StringIO(data.decode("ascii")))


def metrics(indexed):
    values = sorted(value for _, value in indexed)
    maximum = values[-1]
    return {
        "count": len(values),
        "median_ns": values[(len(values) - 1) // 2],
        "p99_ns": values[(99 * len(values) + 99) // 100 - 1],
        "max_ns": maximum,
        "max_indices": [index for index, value in indexed if value == maximum],
    }


def analyze(rows):
    deadlines, wakes, gaps, delays = [], [], [], []
    for i, row in enumerate(rows):
        due, wake = int(row["deadline_ns"]), int(row["wake_ns"])
        delay = wake - due
        if (int(row["index"]) != i or delay < 0 or int(row["lateness_ns"]) != delay or
                (i and due - deadlines[-1] != PERIOD_NS)):
            raise ValueError(f"invalid release or wake at row {i}")
        if i:
            gap = wake - wakes[-1]
            if gap < 0 or gap != int(row["inter_completion_ns"]):
                raise ValueError(f"invalid completion interval at row {i}")
            gaps.append((i, gap))
        elif row["inter_completion_ns"] != "":
            raise ValueError("first wake has no adjacent completion interval")
        deadlines.append(due)
        wakes.append(wake)
        delays.append((i, delay))
    if len(wakes) != SAMPLES:
        raise ValueError(f"expected {SAMPLES} samples, found {len(wakes)}")
    return {
        "adjacent_gap": metrics(gaps),
        "wake_minus_target": metrics(delays),
        "gaps_above_20ms": [
            {"index": i, "previous_wake_ns": wakes[i-1], "target_ns": deadlines[i],
             "wake_ns": wakes[i], "gap_ns": gap, "lateness_ns": wakes[i] - deadlines[i]}
            for i, gap in gaps if gap > GAP_LIMIT_NS
        ],
        "indices_at_least_one_period_late": [i for i, delay in delays if delay >= PERIOD_NS],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="original jnox449-evidence.zip")
    args = parser.parse_args()
    if hashlib.sha256(args.archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
        parser.error("archive SHA-256 differs from the published #449 record")
    with zipfile.ZipFile(args.archive) as archive:
        entries = json.loads(archive.read("manifest.json"))["entries"]
        results = {case: analyze(read_csv(archive, entries, case)) for case in CASES}
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
