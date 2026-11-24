"""Replay one frozen CHG-071 launch using its original UART and executed ELF."""
import argparse
from dataclasses import asdict, dataclass
import hashlib
import json
from pathlib import Path
import struct
from typing import Optional

from reservation_handoff import NA, decode, require


@dataclass(frozen=True)
class Launch:
    ordinal: int
    selector: str
    site: str
    elf_sha256: str
    uart_sha256: str
    counter_pc: Optional[int]


ORDER = [
    ("aligned-wake", site) for site in ("B", "E", "F")
] + [
    ("cbs-saturated", site) for site in ("F", "E", "B", "B", "E", "F")
] + [
    ("aligned-wake", site) for site in ("F", "E", "B")
]


def verify_counter(elf: bytes, pc: int) -> None:
    require(type(pc) is int and pc > 0 and pc % 4 == 0, "invalid counter PC")
    require(len(elf) >= 64 and elf[:6] == b"\x7fELF\x02\x01", "expected ELF64 LE")
    require(struct.unpack_from("<H", elf, 18)[0] == 183, "expected AArch64")
    offset = struct.unpack_from("<Q", elf, 32)[0]
    size, count = struct.unpack_from("<HH", elf, 54)
    require(size >= 56 and offset + count * size <= len(elf), "invalid program headers")
    matches = []
    for i in range(count):
        kind, flags, fileoff, vaddr, _, length = struct.unpack_from("<IIQQQQ", elf, offset + i * size)
        if kind == 1 and flags & 1 and vaddr <= pc and pc + 4 <= vaddr + length:
            require(fileoff + length <= len(elf), "truncated executable segment")
            matches.append(struct.unpack_from("<I", elf, fileoff + pc - vaddr)[0])
    require(len(matches) == 1 and matches[0] & ~31 == 0xD53BE020, "expected MRS CNTPCT_EL0 at pinned PC")


def replay(raw: bytes, elf: bytes, launch: Launch) -> dict:
    require(type(launch.ordinal) is int and 1 <= launch.ordinal <= len(ORDER), "invalid ordinal")
    require(ORDER[launch.ordinal - 1] == (launch.selector, launch.site), "wrong launch assignment")
    require(hashlib.sha256(elf).hexdigest() == launch.elf_sha256, "ELF mismatch")
    require(hashlib.sha256(raw).hexdigest() == launch.uart_sha256, "UART mismatch")
    baseline = launch.site == "B"
    header = b"RSRV RAW 3" if baseline else b"RSRV RAW 4"
    lines = raw.splitlines()
    require([line for line in lines if line.startswith(b"RSRV RAW")] == [header], "wrong schema")
    start = lines.index(header)
    records = lines[start + 1:-1]
    require(lines[-1:] == [b"RSRV END"] and len(records) <= 1024, "incomplete or oversized UART")
    rows = []
    for line in records:
        words = line.split()
        require(len(words) == 8 and all(w.isdigit() and len(w) <= 20 for w in words), "invalid row")
        values = list(map(int, words))
        require(max(values) <= NA, "row overflow")
        rows.append(values)

    probe_indices = [i for i, row in enumerate(rows) if row[0] == 38]
    require(len(probe_indices) == int(not baseline), "missing or duplicate diagnostic row")
    tick = None
    descriptor = 1 if launch.selector == "aligned-wake" else 4
    if baseline:
        require(launch.counter_pc is None, "unexpected baseline probe PC")
    else:
        verify_counter(elf, launch.counter_pc)
        i = probe_indices[0]
        tag, stamp, slot, owner, site, selector, present, pc = rows[i]
        require(i + 1 < len(rows) and rows[i + 1][0] in (24, 36), "misplaced diagnostic")
        require((slot, owner, selector) == (0, 2, descriptor), "wrong diagnostic target")
        require(site == (1 if launch.site == "E" else 2) and present in (0, 1), "invalid site/presence")
        if present:
            require(pc == launch.counter_pc, "counter address mismatch")
            tick = stamp
        else:
            require((stamp, pc) == (0, 0), "nonempty absent observation")
        del records[i]
    # Preserve all historic records and validate them with the corrected RAW3 decoder.
    base = decode(b"\n".join([*lines[:start], b"RSRV RAW 3", *records, lines[-1]]))
    configs = [row for row in rows if row[0] == 1]
    if configs:
        require(1 + 2 * configs[0][6] + configs[0][7] == descriptor, "wrong boot configuration")
    flags = base["stages"]
    live = base["live_receipts"]
    def timestamp(tag):
        return live[tag][0][1] if live.get(tag) else None
    source, pre = timestamp(37), timestamp(26)
    if tick is not None:
        require(source is not None and source <= tick <= rows[-1][1], "probe outside opening")
    if pre is not None and not baseline:
        require(tick is not None and tick <= pre, "missing or late observation")
    whole = None if pre is None else pre - source
    prefix = None if tick is None else tick - source
    suffix = None if tick is None or pre is None else pre - tick
    if suffix is not None:
        require(prefix + suffix == whole, "inconsistent split")
    marker = [row for row in rows if row[0] == 10 and row[2:4] == [0, 0]]
    require(len(marker) <= 1, "duplicate first service")
    return dict(launch=asdict(launch), boot=base["boot_anchor"], mask=flags,
                terminal=rows[-1], source=source, probe=tick, pre=pre, whole=whole,
                prefix=prefix, suffix=suffix, original_receipts=live,
                actual_return=base["actual_worker_return"], first_service=marker)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("launch", "uart", "elf"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    try:
        result = replay(args.uart.read_bytes(), args.elf.read_bytes(),
                        Launch(**json.loads(args.launch.read_text(encoding="utf-8"))))
        print(json.dumps(result, indent=2))
    except (OSError, TypeError, ValueError) as exc:
        parser.exit(1, f"invalid capture: {exc}\n")
