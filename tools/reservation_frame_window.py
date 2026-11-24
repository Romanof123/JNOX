"""Read the optional aligned-saturated first-frame interval from one frozen boot."""
import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
from typing import Optional

from reservation_handoff import NA, decode, require
from reservation_stage import verify_counter


@dataclass(frozen=True)
class Launch:
    ordinal: int
    selector: str
    arm: str
    trace: bool
    elf_sha256: str
    uart_sha256: str
    before_pc: Optional[int]
    after_pc: Optional[int]


ORDER = (("B", True), ("P", True), ("P", False), ("B", False))


def replay(raw: bytes, elf: bytes, launch: Launch):
    require(type(launch.ordinal) is int and 1 <= launch.ordinal <= len(ORDER), "wrong ordinal")
    require(type(launch.trace) is bool and ORDER[launch.ordinal - 1] == (launch.arm, launch.trace)
            and launch.selector == "aligned-saturated", "wrong arm, selector or trace mode")
    require(hashlib.sha256(elf).hexdigest() == launch.elf_sha256, "wrong ELF")
    require(hashlib.sha256(raw).hexdigest() == launch.uart_sha256, "wrong UART")
    diagnostic = launch.arm == "P"
    header = b"RSRV RAW 5" if diagnostic else b"RSRV RAW 3"
    lines = raw.splitlines()
    require([line for line in lines if line.startswith(b"RSRV RAW")] == [header], "wrong schema")
    begin = lines.index(header)
    require(lines[-1:] == [b"RSRV END"], "incomplete UART")
    content = lines[begin + 1:-1]
    require(len(content) <= 1024, "too many rows")
    rows = []
    for line in content:
        words = line.split()
        require(len(words) == 8 and all(w.isdigit() and len(w) <= 20 for w in words), "invalid row")
        row = list(map(int, words))
        require(max(row) <= NA and 1 <= row[0] <= 40 and row[0] != 38, "foreign tag or word")
        rows.append(row)
    config = [r for r in rows if r[0] == 1]
    require(len(config) == 1 and config[0][4:] == [62500000, 3125000, 0, 1], "wrong clock/workload")

    pair_indices = [i for i, r in enumerate(rows) if r[0] in (39, 40)]
    pcs = (launch.before_pc, launch.after_pc)
    before = after = pair_epoch = None
    raw3 = raw
    if diagnostic:
        require(len(pair_indices) == 2 and pair_indices[1] == pair_indices[0] + 1, "missing or duplicate pair")
        i, j = pair_indices
        require([rows[i][0], rows[j][0]] == [39, 40] and j + 1 < len(rows)
                and rows[j + 1][0] in (24, 36), "wrong pair placement or order")
        require(pcs[0] != pcs[1], "same counter PC")
        for pc in pcs:
            verify_counter(elf, pc)
        first, last = rows[i], rows[j]
        require(first[2:6] == last[2:6] and first[2:5] == [0, 2, 2]
                and first[5] in (0, 1), "wrong pair identity")
        if first[5]:
            require((first[6], last[6]) == pcs and first[7] == last[7], "foreign PC or epoch")
            before, after, pair_epoch = first[1], last[1], first[7]
        else:
            require(all(r[1] == r[6] == r[7] == 0 for r in (first, last)), "nonempty absent pair")
        raw3 = b"\n".join([*lines[:begin], b"RSRV RAW 3", *content[:i], *content[j + 1:], b"RSRV END", b""])
    else:
        require(not pair_indices and pcs == (None, None), "probe in baseline")

    ordinary = decode(raw3)
    anchor = ordinary["boot_anchor"]
    require(anchor is not None, "no epoch for this measurement")
    epoch = anchor[5]
    live = ordinary["live_receipts"]
    source = live[37][0][1] if live[37] else None
    pre = live[26][0][1] if live[26] else None
    terminal = rows[-1]
    if before is not None:
        require(pair_epoch == epoch and source is not None
                and source <= before <= after <= terminal[1], "pair outside original opening")
    if pre is not None and diagnostic:
        require(after is not None and after <= pre, "precommit without an ordered pair")
    whole = pre - source if pre is not None else None
    pieces = ([before - source, after - before, pre - after]
              if after is not None and pre is not None else None)
    if pieces is not None:
        require(sum(pieces) == whole, "inconsistent partition")
    markers = [r for r in rows if r[0] == 10 and r[2:4] == [0, 0]]
    require(len(markers) <= 1, "extra first worker")
    ledger = [{"slot": r[2], "owner": r[3], "pre_eret": r[4], "first": r[5],
               "trap": r[6], "charged": r[7]} for r in rows if r[0] == 10]
    limit = epoch + config[0][4] // 1000
    return {
        "ordinal": launch.ordinal, "arm": launch.arm, "trace": launch.trace,
        "epoch": epoch, "mask": ordinary["stages"], "terminal": terminal,
        "source": source, "before": before, "after": after, "precommit": pre,
        "whole_ticks": whole, "pieces_ticks": pieces,
        "frame_interval_ticks": after - before if after is not None else None,
        "headroom_ticks": [limit - x if x is not None else None for x in (source, before, after, pre)],
        "first_worker_offset": markers[0][5] - epoch if markers else None,
        "return_to_first_ticks": markers[0][5] - markers[0][4] if markers else None,
        "actual_return": ordinary["actual_worker_return"], "grants": ledger,
        "deadline_charges": [r for r in rows if r[0] == 19],
    }, raw3


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("launch", "uart", "elf"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--raw3", type=Path, help="write the ledger decoder input to a new file")
    args = parser.parse_args()
    try:
        launch = Launch(**json.loads(args.launch.read_text(encoding="utf-8")))
        result, canonical = replay(args.uart.read_bytes(), args.elf.read_bytes(), launch)
        if args.raw3:
            with args.raw3.open("xb") as output:
                output.write(canonical)
        print(json.dumps(result, indent=2))
    except (OSError, TypeError, ValueError) as exc:
        parser.exit(1, f"invalid capture: {exc}\n")
