"""Read IMP-054 evidence; outcomes here never grant positive operational admission."""
import argparse
import json
from pathlib import Path

OFF_MODE = "characterization"
OPS_MODE = "admission"


def exact_gap(first, second, required):
    lo, hi = second[0] - first[1], second[1] - first[0]
    if first[0] > first[1] or second[0] > second[1]:
        return "reversed"
    if hi < required:
        return "early"
    if lo > required:
        return "late"
    return "exact" if lo == hi == required else "straddling"


def coverage(rows):
    streams = [[r for r in rows if r[0] == tag] for tag in (80, 81, 82)]
    if not streams[0] or len({len(v) for v in streams}) != 1:
        raise ValueError("incomplete actuator write coverage")
    expected = []
    for ordinal, (request, before, after) in enumerate(zip(*streams), 1):
        if [r[2] for r in (request, before, after)] != [ordinal] * 3 or not request[
            1
        ] <= before[1] <= after[1]:
            raise ValueError("actuator ordinal or timestamp mismatch")
        if before[3] != 0 or after[3] != 0 or request[3] not in (0, 1):
            raise ValueError("device ON or malformed clamp witness")
        expected.extend([request, before, after])
    if [r for r in rows if r[0] in (80, 81, 82)] != expected:
        raise ValueError("actuator records out of order")
    return len(streams[0])


def summarize(path):
    lines = path.read_bytes().splitlines()
    if any(b"CONTAINMENT FAIL" in line or b"CONTROL INVALID" in line for line in lines):
        raise ValueError("guest containment/trace failure")
    if (
        lines.count(b"CONTROL RAW v1") != 1
        or lines.count(b"CONTROL END") != 1
        or lines[-1] != b"CONTROL END"
    ):
        raise ValueError("incomplete or concatenated UART")
    rows = []
    for line in lines[lines.index(b"CONTROL RAW v1") + 1 : -1]:
        words = line.split()
        if len(words) != 4 or not all(w.isdigit() and len(w) <= 20 for w in words):
            raise ValueError("malformed UART row")
        row = list(map(int, words))
        if any(n >= 1 << 64 for n in row):
            raise ValueError("word outside u64")
        rows.append(row)

    def events(tag):
        return [r for r in rows if r[0] == tag]

    def one(tag):
        found = events(tag)
        if len(found) != 1:
            raise ValueError(f"expected one row {tag}")
        return found[0]

    _, _, arm, hz = one(64)
    _, _, admitted, missing = one(65)
    _, _, off, evidence = one(83)
    ident = one(32)
    if arm not in (1, 2) or off not in (0, 1) or ident[2] not in (0, 1) or hz == 0:
        raise ValueError("unexpected arm, mode, clock or policy")
    if one(19)[2] != 0 or one(18)[2] != 0 or rows[-1][0] != 18:
        raise ValueError("initial/final readback is missing or ON")
    result = {
        "arm": f"A{arm}",
        "policy": ["RM", "DM"][ident[2]],
        "frequency": hz,
        "mode": OFF_MODE if off else OPS_MODE,
        "positive_operational_credit": False,
        "admission_mask": missing,
        "evidence_id": evidence,
    }
    if not off:
        if (
            admitted
            or not missing
            or any(events(tag) for tag in (1, 2, 3, 4, 11, 17, 20, 74, 76, 93))
        ):
            raise ValueError(
                "unrecognized operational outcome or activity after refusal"
            )
        return dict(result, outcome="refused-before-launch")
    if events(14) or events(20):
        raise ValueError("actual edge or operational rearm in characterization")
    commits = events(74)
    attempts = events(71)
    if [r[3] for r in commits] != list(range(1, 1 + len(commits))) or any(
        r[2] < r[1] for r in commits
    ):
        raise ValueError("duplicate/reordered publication or reversed bracket")
    if len(attempts) > 7 or len(commits) > len(attempts):
        raise ValueError("publication without a scheduled opportunity")
    writes = coverage(rows)
    terminal = one(89)
    if terminal[2:] != [writes, 0] or one(90)[2] + one(90)[3] != 7:
        raise ValueError("observer terminal or opportunity summary mismatch")
    gaps = [exact_gap(a[1:3], b[1:3], hz // 100) for a, b in zip(commits, commits[1:])]
    return dict(
        result,
        outcome="observed-inhibited",
        publication_intervals=[r[1:] for r in commits],
        gaps=gaps,
        stops=[r[1:] for r in events(66)],
        command_ingress=[r[1:] for r in events(76)],
        receipt_observations=[r[1:] for r in events(78)],
        write_attempts=writes,
        unattempted=one(90)[3],
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("uart", type=Path, nargs="?")
    parser.add_argument("--synthetic-observer", action="store_true")
    args = parser.parse_args()
    if args.synthetic_observer:
        try:
            coverage([[80, 1, 1, 0], [81, 2, 1, 1], [82, 3, 1, 0]])
        except ValueError as exc:
            print(json.dumps({"scope": "synthetic transient ON", "detected": str(exc)}))
            raise SystemExit(0)
        raise SystemExit("synthetic transient ON was missed")
    if args.uart is None:
        parser.error("supply a UART file")
    try:
        print(json.dumps(summarize(args.uart), indent=2))
    except (ValueError, OSError) as exc:
        print(
            json.dumps(
                {
                    "outcome": "invalid-or-incomplete",
                    "reason": str(exc),
                    "positive_operational_credit": False,
                }
            )
        )
        raise SystemExit(1)
