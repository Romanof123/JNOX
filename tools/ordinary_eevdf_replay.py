"""Recompute ordinary EEVDF virtual requests and five service windows from counter intervals."""
from fractions import Fraction
from struct import Struct

from ordinary_eevdf_math import MAX_BLOCKS, batch_output, charge, receiver_round

HEADER = Struct("<12Q")
ROW = Struct("<33Q")
FIRST = Struct("<8Q")
CAPACITY = 512
STACK = 16384
FRAME = 832


class Unobserved(ValueError):
    """The capture lacks a required observation."""


def need(condition, reason):
    if not condition:
        raise ValueError(reason)


def distribution(samples):
    ordered = sorted(samples)
    return {"p50": ordered[3], "p95": ordered[7], "max": ordered[-1]}


def evaluate(data, stack_base, check_pc):
    if len(data) < HEADER.size:
        raise Unobserved("missing journal header")
    (
        version,
        state,
        fail,
        freq,
        q,
        origin,
        count,
        wakes,
        responses,
        finished,
        stack,
        capacity,
    ) = HEADER.unpack_from(data)
    need(
        version == 1 and state in range(4) and capacity == CAPACITY and stack == STACK,
        "header identity",
    )
    need(
        count <= capacity and wakes <= 8 and responses <= 8 and finished <= 3,
        "header counts",
    )
    need(state != 3 and not fail, "guest failure " + str(fail))
    size = HEADER.size + capacity * ROW.size + FIRST.size
    need(len(data) <= size, "trailing journal data")
    if len(data) < size or state != 2:
        raise Unobserved("incomplete journal")
    need(
        freq > 0 and q > 0 and freq == 100 * q and q % 2 == 0 and origin > 0,
        "counter configuration",
    )
    need(not any(data[HEADER.size + count * ROW.size : -FIRST.size]), "unused rows")
    first = FIRST.unpack_from(data, size - FIRST.size)
    weights = [2, 1, 1]
    slices = [2 * q, q, q // 2]
    starts = [0] * 3
    used = [0] * 3
    ready = {0, 1, 2}
    saved = {}
    completed = {}
    waits = 0
    wake_at = []
    witnessed = 0
    gaps = []
    preempted = set()
    resumed = set()
    last = origin
    earliest_start = origin
    external_events = []
    windows = [{"actual": [0] * 3, "target": [Fraction(0)] * 3} for _ in range(5)]
    for index in range(count):
        row = ROW.unpack_from(data, HEADER.size + index * ROW.size)
        (
            seq,
            owner,
            mask,
            start,
            compare,
            stop,
            cause,
            fp,
            pc,
            restore_fp,
            restore_pc,
        ) = row[:11]
        x0, x1, x2, x3, wake, stamp, after, refnum, refden, iar = row[11:21]
        runtime = [a + b for a, b in zip(starts, used)]
        deadline = [a + b for a, b in zip(starts, slices)]
        num = sum(runtime[i] for i in ready)
        den = sum(weights[i] for i in ready)
        need(
            seq == index and owner in ready and (refnum, refden) == (num, den),
            f"row {index} identity/reference",
        )
        candidates = [i for i in sorted(ready) if runtime[i] * den <= num * weights[i]]
        need(
            owner == min(candidates, key=lambda i: Fraction(deadline[i], weights[i])),
            f"row {index} selection",
        )
        snapshot = tuple(
            x
            for i in range(3)
            for x in (runtime[i], deadline[i], weights[i], slices[i] - used[i])
        )
        need(
            row[21:] == snapshot and mask == sum(1 << i for i in ready),
            f"row {index} dispatch snapshot",
        )
        for frame in (fp, restore_fp):
            base = stack_base + owner * STACK
            need(
                frame % 16 == 0 and base <= frame <= base + STACK - FRAME,
                "frame outside owner stack",
            )
        in_batch_body = check_pc(owner, cause, pc, restore_pc, owner not in saved)
        if owner not in saved:
            need(
                restore_fp == stack_base + (owner + 1) * STACK - FRAME,
                "initial frame placement",
            )
        if owner in preempted:
            resumed.add(owner)
        if owner in saved:
            need(saved[owner] == (restore_fp, restore_pc), "unmatched restored frame")
        saved[owner] = fp, pc
        external_events.extend(
            [("eret", restore_pc), ("irq" if cause == 1 else "svc", pc)]
        )
        pending = waits > len(wake_at)
        due = origin + waits * 8 * q
        expected = start + min(q, slices[owner] - used[owner])
        if pending:
            expected = min(expected, due)
        need(
            start >= earliest_start and stop > start and compare == expected,
            "counter interval",
        )
        gaps.append(start - last)
        earliest_start = last = stop
        for w, window in enumerate(windows):
            lo = origin + 8 * w * q
            overlap = max(0, min(stop, lo + 32 * q) - max(start, lo))
            window["actual"][owner] += overlap
            for i in ready:
                window["target"][i] += Fraction(overlap * weights[i], den)
        starts[owner], used[owner] = charge(
            starts[owner], used[owner], slices[owner], stop - start
        )
        if (
            witnessed < len(wake_at)
            and owner == 2
            and start <= first[witnessed] <= stop
        ):
            witnessed += 1
        if cause == 1:
            need(iar == 30 and stop >= compare, "timer IRQ")
            if owner in (0, 1) and in_batch_body:
                preempted.add(owner)
        elif cause == 2:
            need(
                iar == 0 and owner == 2 and x0 == waits + 1 <= 8 and witnessed == waits,
                "receiver wait",
            )
            need(stop < origin + (waits + 1) * 8 * q, "late wait")
            waits += 1
            ready.remove(owner)
            if waits == 5:
                n = sum(starts[i] + used[i] for i in ready)
                d = sum(weights[i] for i in ready)
                starts[1] = starts[1] + used[1] + (n + d - 1) // d
                used[1] = 0
                weights[1] = 2
                slices[1] = 2 * q
        elif cause == 3:
            need(iar == 0 and x0 == owner, "finish identity")
            if owner < 2:
                need(
                    x1 > 0
                    and x1 % 64 == 0
                    and 2 in completed
                    and x1 // 64 < MAX_BLOCKS
                    and (x1, x2, x3) == batch_output(owner, x1 // 64),
                    "batch work",
                )
            else:
                outputs = [receiver_round(s) for s in range(1, 9)]
                need(
                    witnessed == 8
                    and (x1, x2, x3)
                    == (
                        128,
                        sum(outputs),
                        sum(s * v for s, v in enumerate(outputs, 1)),
                    ),
                    "receiver work",
                )
            completed[owner] = (x1, x2, x3)
            ready.remove(owner)
        else:
            raise ValueError("unknown exception cause")
        if wake:
            need(
                cause == 1
                and pending
                and wake == waits
                and stop >= due
                and stamp >= stop,
                "wake attribution",
            )
            n = sum(starts[i] + used[i] for i in ready)
            d = sum(weights[i] for i in ready)
            floor = (n * weights[2] + d - 1) // d
            starts[2] += max(0, floor - starts[2] - used[2])
            ready.add(2)
            wake_at.append(stamp)
            earliest_start = stamp
        else:
            need(
                stamp == 0 and not (cause == 1 and pending and stop >= due),
                "missing or stray wake",
            )
        need(after == sum(1 << i for i in ready), "post-event mask")
    need(resumed == {0, 1}, "batch preemption and resumption")
    need(
        not ready
        and len(completed) == finished == 3
        and wakes == responses == waits == witnessed == len(wake_at) == 8,
        "terminal conservation",
    )
    due_lag = [t - (origin + s * 8 * q) for s, t in enumerate(first, 1)]
    wake_lag = [t - a for t, a in zip(first, wake_at)]
    need(min(wake_lag) >= 0 and max(due_lag) <= 4 * q, "due-to-first-work bound")
    for w, window in enumerate(windows):
        window["start"] = origin + 8 * w * q
        window["stop"] = window["start"] + 32 * q
        actual, target = window["actual"], window["target"]
        need(
            sum(actual) >= 28 * q
            and all(abs(a - t) <= 4 * q for a, t in zip(actual, target)),
            "window fairness",
        )
        window["delivered"] = sum(actual)
        window["errors"] = [str(a - t) for a, t in zip(actual, target)]
        window["target"] = list(map(str, target))
    latency = [
        {
            "due": origin + s * 8 * q,
            "wake": wake_at[s - 1],
            "first": first[s - 1],
            "due_to_first": due_lag[s - 1],
            "wake_to_first": wake_lag[s - 1],
        }
        for s in range(1, 9)
    ]
    return {
        "gaps": gaps,
        "latencies": latency,
        "rows": count,
        "frequency": freq,
        "quantum": q,
        "windows": windows,
        "work": completed,
        "due_to_first": distribution(due_lag),
        "wake_to_first": distribution(wake_lag),
    }, external_events


def linked(elf_bytes):
    import io
    from elftools.elf.elffile import ELFFile

    elf = ELFFile(io.BytesIO(elf_bytes))
    need(
        elf.little_endian and elf.elfclass == 64 and elf["e_machine"] == "EM_AARCH64",
        "ELF target",
    )
    table = elf.get_section_by_name(".symtab")
    if table is None:
        raise Unobserved("missing linked symbol table")

    def unique(name):
        hits = table.get_symbol_by_name(name) or []
        need(len(hits) == 1, "missing or ambiguous symbol " + name)
        return hits[0]

    stacks = unique("jnox_eevdf_app_stacks")
    need(
        stacks["st_size"] == 3 * STACK and stacks["st_value"] % 16 == 0, "stack symbol"
    )
    journal = unique("jnox_eevdf_app_journal")
    need(
        journal["st_size"] == HEADER.size + CAPACITY * ROW.size + FIRST.size,
        "journal symbol",
    )
    workers = [unique(name) for name in ("batch_a", "batch_b", "responder")]
    sections = [
        (s["sh_addr"], s.data()) for s in elf.iter_sections() if s["sh_flags"] & 4
    ]
    bodies = [
        (s["st_value"], s["st_value"] + s["st_size"])
        for s in table.iter_symbols()
        if "eevdf_app_tasks" in s.name
        and "batch" in s.name
        and s["st_info"]["type"] == "STT_FUNC"
    ]

    def instruction(pc):
        for base, code in sections:
            if pc % 4 == 0 and base <= pc <= base + len(code) - 4:
                return int.from_bytes(code[pc - base : pc - base + 4], "little")
        raise ValueError("PC outside linked executable sections")

    def check(owner, cause, pc, restored, initial):
        instruction(restored)
        instruction(pc)
        if initial:
            need(restored == workers[owner]["st_value"], "initial entry")
        if cause in (2, 3):
            imm = cause - 1
            need(
                instruction(pc - 4) == 0xD4000001 | imm << 5,
                "SVC immediate at linked saved PC",
            )
            if cause == 2:
                need(instruction(pc) == 0xD5033FDF, "missing task-side ISB after wait")
                need(
                    instruction(pc + 4) & ~31 == 0xD53BE020,
                    "missing task-side CNTPCT sample",
                )
        if owner < 2:
            worker = workers[owner]
            return any(
                lo <= pc < hi
                for lo, hi in bodies
                + [(worker["st_value"], worker["st_value"] + worker["st_size"])]
            )
        return False

    return stacks["st_value"], check


def replay(root, digest):
    import json
    import re
    from pathlib import PurePosixPath
    from ordinary_amc_capture import FLAGS, sha
    from ordinary_amc_replay import bounded, external, obj, Incomplete
    from ordinary_eevdf_capture import MARKERS

    need(sha(root / "session.json") == digest, "session SHA256")
    session = json.loads(bounded(root / "session.json", 65536), object_pairs_hook=obj)
    files = {
        "launch.json",
        "kernel.elf",
        "qemu-version",
        "symbols",
        "disassembly",
        "stderr",
        "uart.bin",
        "exceptions.log",
        "journal.bin",
    }
    hashes = session["hashes"]
    need(set(hashes) <= files, "unexpected artifact in manifest")
    for name, h in hashes.items():
        need(
            re.fullmatch("[0-9a-f]{64}", h) and sha(root / name) == h,
            "artifact digest " + name,
        )
    if session["reason"] == MARKERS[1].decode():
        raise ValueError("guest failure marker")
    if set(hashes) != files or session["reason"] != MARKERS[0].decode():
        raise Unobserved("incomplete capture: " + session["reason"])
    launch = json.loads(bounded(root / "launch.json", 65536), object_pairs_hook=obj)
    need(
        launch["version"] == 1
        and re.fullmatch("[0-9a-f]{40}", launch["revision"])
        and launch["elf_sha256"] == hashes["kernel.elf"]
        and re.fullmatch("[0-9a-f]{64}", launch["qemu_sha256"])
        and launch["timeout_seconds"] == 20
        and session["pid"] > 0,
        "launch identity",
    )
    argv = launch["argv"]
    prefix = [argv[0], *FLAGS, "-qmp", "stdio", "-d", "int", "-D"]
    need(
        len(argv) == len(prefix) + 5 and argv[: len(prefix)] == prefix,
        "QEMU configuration",
    )
    log, serial, uart, kernel, elf = argv[len(prefix) :]
    need(
        serial == "-serial" and kernel == "-kernel" and uart.startswith("file:"),
        "QEMU artifact arguments",
    )
    paths = [PurePosixPath(p.replace(chr(92), "/")) for p in (log, uart[5:], elf)]
    need(
        [p.name for p in paths] == ["exceptions.log", "uart.bin", "kernel.elf"]
        and len({p.parent for p in paths}) == 1,
        "QEMU artifact locations",
    )
    lines = bounded(root / "uart.bin", 2**20).splitlines()
    need(
        [
            line
            for line in lines
            if line.startswith(b"EEVDF APP ")
            and (b"FAIL" in line or b"COMPLETE" in line)
        ]
        == [MARKERS[0]],
        "terminal UART",
    )
    stack, check = linked(bounded(root / "kernel.elf", 128 << 20))
    result, expected = evaluate(
        bounded(root / "journal.bin", HEADER.size + CAPACITY * ROW.size + FIRST.size),
        stack,
        check,
    )
    try:
        observed = [
            (kind, pc)
            for kind, pc, _, _ in external(bounded(root / "exceptions.log", 64 << 20))
        ]
    except Incomplete as exc:
        raise Unobserved(str(exc)) from exc
    if len(observed) < len(expected) and observed == expected[: len(observed)]:
        raise Unobserved("incomplete exception/return history")
    need(observed == expected, "exception/return history")
    result.update(
        source=launch["revision"],
        elf_sha256=hashes["kernel.elf"],
        session_sha256=digest,
    )
    return result


if __name__ == "__main__":
    import argparse
    import json
    from pathlib import Path

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("session_sha256")
    args = parser.parse_args()
    try:
        print(json.dumps(replay(args.root, args.session_sha256), indent=2))
    except (Unobserved, FileNotFoundError, ImportError) as exc:
        parser.exit(2, f"UNOBSERVED {exc}\n")
    except (ValueError, KeyError, OSError, TypeError, IndexError) as exc:
        parser.exit(1, f"INVALID {exc}\n")
