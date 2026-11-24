"""Bounded ordinary batch UART contracts and independent counter/frame replay."""
from fractions import Fraction
import re

MAX_BYTES = 24 * 1024
WORD = rb" [0-9a-f]{16}"
EVENT = re.compile(rb"BATCH EVENT" + WORD * 15)
IDLE = re.compile(rb"BATCH IDLE EMPTY" + WORD * 2)
ORDERS = {
    b"batch-sjf": ("WORK END", "SHORT", "OLDER ONE", "OLDER TWO"),
    b"batch-srtf": ("SHORT", "WORK END", "OLDER ONE", "OLDER TWO"),
    b"batch-hrrn": ("WORK END", "OLDER ONE", "OLDER TWO", "SHORT"),
}


def ordinary(profile):
    labels = ("ESTIMATES REJECTED", "FULL REJECTED", "WORK BEGIN", *ORDERS[profile],
              "STALE REJECTED", "OVERRUN SUBMIT", "OVERRUN RETAINED", "CONTINUATION", "FINISHED")
    return b"JNOX M1 BOOT\nJNOX M1 READY\n" + b"".join(
        b"APP " + label.encode() + b"\n" for label in labels)


def validate(data, profile, board=b"M1"):
    if len(data) > MAX_BYTES or board != b"M1" or profile not in ORDERS:
        return False
    preamble = ordinary(profile)
    if not data.startswith(preamble):
        return False
    lines = data[len(preamble):].split(b"\n")
    if not 5 <= len(lines) <= 67 or lines[-2:] != [b"BATCH TRACE END", b""]:
        return False
    if not IDLE.fullmatch(lines[0]) or any(not EVENT.fullmatch(line) for line in lines[1:-2]):
        return False
    idle_pc, idle_sp = (int(x, 16) for x in lines[0].split()[3:])
    if not idle_pc or idle_pc % 4 or not idle_sp or idle_sp % 16:
        return False
    rows = [tuple(int(x, 16) for x in line.split()[2:]) for line in lines[1:-2]]
    # The first idle dispatch constructs a complete 832-byte Frame directly
    # below its 16 KiB stack top. The sampled idle SP must belong to it.
    idle_entries = [r for r in rows if r[2] != 0 and r[3] == 0]
    if len(idle_entries) != 2:
        return False
    top = idle_entries[0][6] + 832
    if not top - 16384 < idle_sp <= top:
        return False
    return replay(rows, profile)


def replay(rows, profile):
    """Apply admissions/completions, charge each real incumbent and rank every live candidate."""
    jobs = {}
    old_owner = last = compare = sequence = 0
    saved = {}
    accepted = []
    rejected = []
    stale = []
    irqs = []
    for i, row in enumerate(rows):
        kind, now, old, new, src, pc, dst, elr, arg, estimate, delay, status, token, left, armed = row
        if old != old_owner or now < last or not dst or dst % 16 or not elr or elr % 4:
            return False
        if i and (not src or src % 16 or not pc or pc % 4):
            return False
        if not i and (kind, old, src, pc, arg, status, token, delay) != (0, 0, 0, 0, 0, 0, 1, 0):
            return False
        if old:
            jobs[old]["remaining"] = max(1, jobs[old]["remaining"] - (now - last))
        if i:
            saved[old] = (src, pc)
        if kind == 3 or kind == 0 and i == 0:
            expected = 0
            if not 1 <= estimate < 2**32:
                expected = 1
            elif now + delay >= 2**64:
                expected = 4
            elif len(jobs) >= 4:
                expected = 2
            if status != expected:
                return False
            if status:
                rejected.append((arg, estimate, delay, status))
                if token:
                    return False
            else:
                sequence += 1
                if token != sequence:
                    return False
                jobs[token] = dict(due=now + delay, estimate=estimate, remaining=estimate)
                accepted.append((arg, estimate, delay))
        elif kind == 4:
            if (estimate, delay, token) != (0, 0, 0) or status != int(arg != old) or not old:
                return False
            if status:
                stale.append((old, arg))
            else:
                del jobs[old]
                del saved[old]
        elif kind == 1:
            if not compare or now < compare or any((arg, estimate, delay, status, token)):
                return False
            irqs.append((old, new))
        else:
            return False
        ready = [t for t in jobs if jobs[t]["due"] <= now]
        def rank(t):
            j = jobs[t]
            if profile == b"batch-hrrn":
                metric = -Fraction(now - j["due"], j["estimate"])
            elif profile == b"batch-srtf":
                metric = j["remaining"]
            else:
                metric = j["estimate"]
            return metric, t != old, j["due"], t
        expected = (old if old in ready and profile != b"batch-srtf"
                    else min(ready, key=rank, default=0))
        if new != expected or left != (jobs[new]["remaining"] if new else 0):
            return False
        if new in saved and saved[new] != (dst, elr):
            return False
        compare = min((j["due"] for j in jobs.values() if j["due"] > now), default=0)
        if compare != armed:
            return False
        last, old_owner = now, new
    if len(accepted) != 6 or jobs or old_owner:
        return False
    hz = accepted[0][1]
    expected_jobs = [(0, hz, 0), (1, 2*hz, 0), (2, 2*hz, 0), (3, hz//2, hz//10),
                     (0, 1, hz//20), (0, 1, 0)]
    return (accepted == expected_jobs and rejected == [(0, 0, 0, 1), (0, 2**32, 0, 1),
            (0, 1, 2**64-1, 4), (0, 1, 0, 2)] and len(stale) == 2 and stale[0] == (1, 2)
            and stale[1][1] == 1 and irqs == [(1, 4 if profile == b"batch-srtf" else 1), (0, 5)])


def main():
    import argparse
    import json
    from pathlib import Path
    import struct
    from qemu_capture import journal_location

    parser = argparse.ArgumentParser(description="Replay and cross-check a Linux batch QMP capture")
    parser.add_argument("profile", choices=[p.decode() for p in ORDERS])
    parser.add_argument("kernel", type=Path)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    folder = args.directory
    with (folder / "uart.bin").open("rb") as stream:
        uart = stream.read(MAX_BYTES + 1).replace(b"\r\n", b"\n")
    profile = args.profile.encode()
    if not validate(uart, profile):
        parser.error("invalid batch UART replay")
    lines = uart[len(ordinary(profile)):].splitlines()
    rows = [tuple(int(x, 16) for x in line.split()[2:]) for line in lines[1:-1]]
    raw = b"".join(struct.pack("<15Q", *r) for r in rows).ljust(64 * 15 * 8, b"\0")
    if (folder / "audit.bin").read_bytes() != raw:
        parser.error("QMP event memory disagrees with UART")
    pc, length = journal_location(args.kernel, "jnox_batch_idle")
    qmp_pc = re.search(r"\bPC=([0-9a-fA-F]+)\b", (folder / "registers.txt").read_text())
    idle_sample = int(lines[0].split()[3], 16)
    if not qmp_pc or not all(pc <= value < pc + length for value in (int(qmp_pc[1], 16), idle_sample)):
        parser.error("idle PC outside linked symbol")
    elapsed = {}
    for first, second in zip(rows, rows[1:]):
        owner = first[3]
        elapsed[owner] = elapsed.get(owner, 0) + second[1] - first[1]
    print(json.dumps(dict(profile=args.profile, records=len(rows), charged_counter_ticks=elapsed,
                          idle_pc=hex(idle_sample), qmp_pc=qmp_pc[1]), indent=2))


if __name__ == "__main__":
    main()
