"""Qualify the four original ordinary mutex controls in a #236 evidence ZIP."""
import argparse
from collections import Counter, defaultdict
import hashlib
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import zipfile

REPLAY = Path(__file__).resolve().parents[1] / "docs/evidence/ordinary-mutex-first/replay.py"
WIDTH = {"H": 5, "T": 8, "S": 5, "E": 26, "R": 3, "C": 2,
         "B": 3, "HIGH": 3, "A": 2, "M": 3, "K": 9}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_miss_state(rows, ceiling, irq, chunks):
    # stop(16) prints from inside the first IRQ, before returning to HIGH or MEDIUM.
    k = rows["K"][0]
    require(rows["HIGH"][0] == rows["M"][0] == [0, 0, 0] and not any(k[3:]),
            "unreleased worker or handoff has a result or marker")
    prefix = rows["E"][:-1]
    actions = [(1, 0), (1, 1), (3, 0), (2, 1), (2, 0), (4, 0)] if ceiling else [
        (1, 0), (2, 0), (4, 0)]
    require(len(prefix) > ceiling and
            [(e[2], e[3]) for e in prefix] == actions[:len(prefix)] and
            all(e[4] == 0 for e in prefix), "unexpected pre-release worker journal")
    for e in prefix:
        count, working = {1: (0, 0), 3: (128, 1), 2: (256, 0), 4: (256, 0)}[e[2]]
        require(e[20:22] == [count, working], "work progress contradicts service entry")

    final_lock = int(ceiling)
    first_unlock = 3 if ceiling else 1
    unlocked = len(prefix) > first_unlock
    yielded = ceiling and len(prefix) > 2
    work = irq[21]
    require(0 <= chunks <= 256 and work in (0, 1) and
            (chunks in (0, 256) or work) and
            (not ceiling or (chunks >= 128 if yielded else chunks <= 128)) and
            (not unlocked or (chunks == 256 and not work)), "impossible IRQ work state")

    # Each marker belongs between a return to LOW and the next relevant exception.
    def marker_between(value, after, before):
        if value:
            require(rows["R"][after + 1][2] <= value <= rows["E"][min(before, len(prefix))][0],
                    "LOW marker outside its execution interval")

    marker_between(k[0], final_lock, final_lock + 1)
    marker_between(k[1], 2 if yielded else final_lock, first_unlock)
    require(not (chunks or work) or k[0], "missing LOW post-acquire marker")
    batch = tuple(rows["B"][0])
    writes = [(0, 0, 0), (0, 65536, 0), (0, 65536, 557056), (1, 65536, 557056)]
    require(batch in (writes if chunks == 256 and not work else writes[:1]),
            "LOW batch contradicts recorded progress")
    require((not k[1] or batch == writes[-1] and k[0] <= k[1]) and
            (not unlocked or k[1]), "LOW commit marker contradicts batch or unlock")

    audit = rows["A"][0]
    if ceiling and unlocked:
        # The two-field volatile audit may be interrupted before stamp(2).
        require(audit[0] in (0, 1) and audit[1] in (0, 256), "invalid ceiling audit")
        marker_between(k[2], first_unlock, first_unlock + 1)
        require((not k[2] or audit == [1, 256]) and
                (len(prefix) <= first_unlock + 1 or k[2]), "inconsistent audit marker")
    else:
        require(audit == [0, 0] and not k[2], "audit before ceiling unlock")


def inspect_uart(raw, ceiling):
    lines = raw.decode("ascii").splitlines()
    require(lines[:3] == ["JNOX M1 BOOT", "JNOX M1 READY", "MUTEX RAW 1"]
            and lines[-1] == "MUTEX END", "incomplete UART envelope")
    rows = defaultdict(list)
    for line in lines[3:-1]:
        fields = line.split()
        require(fields and WIDTH.get(fields[0]) == len(fields) - 1
                and all(re.fullmatch("[0-9a-f]{16}", f) for f in fields[1:]),
                "invalid UART row")
        rows[fields[0]].append([int(f, 16) for f in fields[1:]])
    for tag in ("H", "T", "B", "HIGH", "A", "M", "K"):
        require(len(rows[tag]) == 1, f"expected one {tag} row")
    require([s[0] for s in rows["S"]] == list(range(4)), "invalid stack roster")
    protocol, reason, events, restores, chunks = rows["H"][0]
    require(protocol == ceiling, "UART protocol does not match the position")
    require(events == restores == len(rows["E"]) == len(rows["R"])
            and chunks == len(rows["C"]), "incomplete event/restore/chunk ledger")
    interrupts = [e for e in rows["E"] if e[2] == 5]
    require(len(interrupts) == 1, "expected one IRQ")
    irq = interrupts[0]
    in_window = (irq[4] == 0 and irq[21] == 1 and irq[0] >= rows["T"][0][2]
                 and irq[14 + ceiling] == 0 and (not ceiling or irq[20] < 128))
    if reason == 16:
        require(not in_window and rows["E"][-1] == irq and irq[20] == chunks,
                "reason 16 contradicts the IRQ evidence")
        check_miss_state(rows, ceiling, irq, chunks)
        require(rows["C"] == [[256*k, 2176*k] for k in range(1, chunks+1)],
                "invalid partial progress")
    else:
        require(reason == 0 and in_window, "unexpected reason or IRQ outside work")
    return reason


def read_manifest(archive):
    members = Counter(archive.namelist())
    require(all(count == 1 for count in members.values()), "repeated ZIP member")
    manifest = {}
    for line in archive.read("SHA256SUMS").decode("ascii").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (?:\./)?(.+)", line)
        require(match is not None, "malformed SHA256SUMS")
        digest, name = match.groups()
        require(name not in manifest, f"repeated manifest entry {name}")
        manifest[name] = digest
    return manifest


def qualify(path):
    with zipfile.ZipFile(path) as archive, tempfile.TemporaryDirectory() as temp:
        stage = Path(temp)
        manifest = read_manifest(archive)
        staged = []
        results = []
        seen = set()

        def copy(source, target):
            data = archive.read(source)
            digest = hashlib.sha256(data).hexdigest()
            require(manifest.get(source) == digest, f"missing or incorrect hash for {source}")
            require(digest not in seen, f"repeated evidence at {source}")
            seen.add(digest)
            target.write_bytes(data)
            staged.append(f"{digest}  {target.relative_to(stage).as_posix()}")
            return data

        for ceiling, protocol in enumerate(("PI", "ceiling")):
            base = "ordinary-" + protocol.lower()
            for mode in ("plain", "traced"):
                label = f"{protocol}/{mode}"
                position = stage / f"0{len(results)+1}-{protocol}-{mode}"
                position.mkdir()
                uart = "trace.uart" if mode == "traced" else "plain.uart"
                try:
                    raw = copy(f"{base}/{uart}", position / "uart")
                    if mode == "traced":
                        copy(f"{base}/exceptions.log", position / "external.log")
                    reason = inspect_uart(raw, ceiling)
                except (ValueError, KeyError) as error:
                    raise ValueError(f"{label}: {error}") from error
                results.append((label, reason))

        (stage / "SHA256SUMS").write_text("\n".join(staged), encoding="ascii")
        # Isolated Python ignores PYTHONOPTIMIZE. The historical oracle uses assertions.
        replay = subprocess.run([sys.executable, "-I", str(REPLAY), str(stage)],
                                capture_output=True, text=True)
        require(replay.returncode == 0, f"historical replay failed: {replay.stderr.strip()}")
        return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cohort", type=Path, metavar="COHORT.zip")
    try:
        results = qualify(parser.parse_args().cohort)
    except (OSError, KeyError, ValueError, RuntimeError, zipfile.BadZipFile) as error:
        print(f"invalid evidence: {error}", file=sys.stderr)
        return 2
    for label, reason in results:
        print(label, f"UNQUALIFIED reason={reason}" if reason else "QUALIFIED")
    score = sum(reason == 0 for _, reason in results)
    print(f"qualified={score}/4")
    return 0 if score == 4 else 1


if __name__ == "__main__":
    sys.exit(main())
