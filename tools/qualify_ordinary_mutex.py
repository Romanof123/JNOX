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
