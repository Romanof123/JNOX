"""Check the static engineering reference ceilings for the AArch64 virt linker layout."""
import argparse
import json
import sys
from pathlib import Path

import scheduling_footprint as elf

BASE = 0x40200000
LIMIT = 0x48000000
CODE_LIMIT = 256 * 1024
RAM_LIMIT = 2 * 1024 * 1024
STACK_SIZE = 16 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def profile(data):
    # Reuse the existing ELF bounds, table, string table and union checks.
    generic = elf.measure(data)
    header = elf.HEADER.unpack_from(data)
    require(generic["machine"] == "aarch64" and header[4] == BASE,
            "expected AArch64 virt executable entry at 0x40200000")
    programs = list(elf.entries(data, header[5], header[10], header[9], elf.PROGRAM, "program"))
    require(len(programs) == 2 and [p[:2] for p in programs] == [(1, 5), (1, 6)],
            "expected the virt linker RX and RW PT_LOAD headers in order")
    rx, rw = programs
    for _, _, off, va, pa, filesz, memsz, _ in programs:
        require(va == pa and BASE <= va < va + memsz <= LIMIT,
                "load region is outside the identity-mapped virt RAM window")
        require(off <= len(data), "load file offset outside ELF")
    require(rx[3] == BASE and rx[5] == rx[6] and rx[3] + rx[6] <= rw[3]
            and rw[3] % 4096 == 0, "invalid RX extent or writable boundary")
    require(not rw[5] or rx[2] + rx[5] <= rw[2] or rw[2] + rw[5] <= rx[2],
            "overlapping load file ranges")

    table = list(elf.entries(data, header[6], header[12], header[11], elf.SECTION, "section"))
    _, _, _, _, names, names_size, *_ = table[header[13]]
    section_info = {}
    specs = {".text": (rx, 1, 6), ".rodata": (rx, 1, 2), ".data": (rw, 1, 3),
             ".bss": (rw, 8, 3), ".stack": (rw, 8, 3)}
    for name_idx, kind, flags, address, off, length, _, _, alignment, _ in table:
        if not (flags & 2 and length):
            continue
        name_start = names + name_idx
        name_end = data.find(b"\0", name_start, names + names_size)
        require(name_end >= name_start, "unterminated section name")
        name = data[name_start:name_end].decode("ascii")
        require(name in specs and name not in section_info, "unknown or duplicate allocated section: " + name)
        segment, expected_kind, expected_flags = specs[name]
        require(kind == expected_kind and flags & ~0x30 == expected_flags,
                "unexpected section type or flags: " + name)
        require(alignment in (0, 1) or (alignment & (alignment - 1) == 0 and address % alignment == 0),
                "invalid section alignment: " + name)
        offset = address - segment[3]
        require(0 <= offset <= segment[6] - length, "section outside its prescribed segment: " + name)
        if kind == 1:
            require(offset + length <= segment[5] and off == segment[2] + offset,
                    "incorrect file backing for " + name)
        else:
            require(offset >= segment[5], "NOBITS overlaps file-backed load data: " + name)
        section_info[name] = {"address": address, "bytes": length}

    last = BASE
    for name in specs:
        if name in section_info:
            item = section_info[name]
            require(item["address"] >= last, "overlapping or reordered section: " + name)
            last = item["address"] + item["bytes"]
    require(section_info.get(".text", {}).get("address") == BASE, "missing linked entry text")
    stack = section_info.get(".stack", {})
    require(stack.get("bytes") == STACK_SIZE and stack["address"] % 16 == 0
            and last == rw[3] + rw[6], "expected terminal aligned 16 KiB linker stack")

    def section_bytes(name):
        return section_info.get(name, {}).get("bytes", 0)

    metrics = dict(immutable_span=rx[5], writable_span=rw[6],
                   segment_gap=rw[3] - (BASE + rx[6]), ram_span=last - BASE,
                   immutable_padding=rx[5] - section_bytes(".text") - section_bytes(".rodata"),
                   writable_padding=rw[6] - section_bytes(".data") - section_bytes(".bss") - STACK_SIZE,
                   linker_stack=STACK_SIZE, load_file=generic["bytes"]["load_file"],
                   load_memory=generic["bytes"]["load_memory"], elf_file=len(data))
    segments = {name: dict(address=s[3], file_offset=s[2], filesz=s[5], memsz=s[6])
                for name, s in zip(("rx", "rw"), programs)}
    return dict(sha256=generic["sha256"], bytes=metrics, sections=section_info, segments=segments)


def compare(minimal, fixed_rr):
    def read(path):
        try:
            return profile(path.read_bytes())
        except (OSError, ValueError) as exc:
            raise ValueError(f"{path}: {exc}") from exc

    baseline, comparison = read(minimal), read(fixed_rr)
    values = baseline["bytes"]
    limits = dict(immutable_span=CODE_LIMIT, ram_span=RAM_LIMIT)
    passed = {name: values[name] <= limit for name, limit in limits.items()}
    return dict(schema_version=1, minimal=baseline, fixed_rr=comparison,
                delta_bytes={k: v - values[k] for k, v in comparison["bytes"].items()},
                minimal_limits_bytes=limits, minimal_within_limit=passed,
                minimal_meets_reference=all(passed.values()))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--minimal", type=Path, required=True)
    parser.add_argument("--fixed-rr", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        result = compare(args.minimal, args.fixed_rr)
    except ValueError as exc:
        parser.error(str(exc))
    print(json.dumps(result, indent=2))
    return 0 if result["minimal_meets_reference"] else 1


if __name__ == "__main__":
    sys.exit(main())
