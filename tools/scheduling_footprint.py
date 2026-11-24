"""Compare static byte footprints of named release ELF executables."""

import argparse
import hashlib
import json
import mmap
import os
import struct
import sys


HEADER = struct.Struct("<16sHHIQQQIHHHHHH")
PROGRAM = struct.Struct("<IIQQQQQQ")
SECTION = struct.Struct("<IIQQQQIIQQ")
MACHINES = {183: "aarch64", 62: "x86_64"}


def span(start, length, limit, label):
    end = start + length
    if end > limit:
        raise ValueError(f"{label} exceeds its address or file bounds")
    return start, end


def union_length(ranges):
    total = end = 0
    for start, stop in sorted(ranges):
        if stop > end:
            total += stop - max(end, start)
            end = stop
    return total


def entries(data, offset, count, stride, layout, label):
    if not count or stride != layout.size or offset < HEADER.size:
        raise ValueError(f"missing or unsupported {label} table")
    span(offset, count * stride, len(data), label)
    return (layout.unpack_from(data, offset + i * stride) for i in range(count))


def measure(data):
    if len(data) < HEADER.size or data[:4] != b"\x7fELF":
        raise ValueError("missing or truncated ELF64 header")
    if data[4:7] != bytes((2, 1, 1)):
        raise ValueError("requires ELF64 little-endian version 1")
    (_, kind, machine, version, _, phoff, shoff, _, size, phsize, phnum,
     shsize, shnum, names_index) = HEADER.unpack_from(data)
    if kind != 2 or machine not in MACHINES or version != 1 or size != HEADER.size:
        raise ValueError("requires a supported AArch64 or x86-64 ET_EXEC header")
    if phnum == 0xffff or not 0 < names_index < shnum:
        raise ValueError("missing or unsupported program/section numbering")
    memory, backed, payload = [], [], []
    for ptype, _, off, addr, _, filesz, memsz, align in entries(
            data, phoff, phnum, phsize, PROGRAM, "program"):
        if ptype != 1:
            continue
        if filesz > memsz:
            raise ValueError("PT_LOAD file size exceeds memory size")
        if align > 1 and (align & (align - 1) or (addr - off) % align):
            raise ValueError("invalid PT_LOAD alignment")
        if filesz:
            payload.append(span(off, filesz, len(data), "PT_LOAD payload"))
            backed.append(span(addr, filesz, 1 << 64, "PT_LOAD backed mapping"))
        memory.append(span(addr, memsz, 1 << 64, "PT_LOAD memory"))
    memlen = union_length(memory)
    if not memlen:
        raise ValueError("no nonempty PT_LOAD memory")
    sections = entries(data, shoff, shnum, shsize, SECTION, "section")
    _, stype, _, _, names, names_size, *_ = SECTION.unpack_from(
        data, shoff + names_index * shsize)
    _, names_end = span(names, names_size, len(data), "section names")
    if stype != 3 or not names_size or data[names] or data[names_end - 1]:
        raise ValueError("invalid section name string table")
    allocated, bss, stack = [], [], []
    for name, stype, flags, addr, offset, length, *_ in sections:
        if name >= names_size:
            raise ValueError("invalid section name index")
        if stype not in (0, 8) and length:
            span(offset, length, len(data), "section payload")
        if not (flags & 2 and length):
            continue
        interval = span(addr, length, 1 << 64, "allocated section")
        allocated.append(interval)
        pos = names + name
        # Only compare the bounded prefixes needed for the linker naming rule.
        prefix = data[pos:min(pos + 7, names_end)]
        if stype == 8 and prefix.startswith((b".bss\0", b".bss.")):
            bss.append(interval)
        if prefix.startswith((b".stack\0", b".stack.")):
            stack.append(interval)
    if union_length(memory + allocated) != memlen:
        raise ValueError("allocated section extends outside PT_LOAD memory")
    alloclen = union_length(allocated)
    backedlen = union_length(backed)
    return {
        "machine": MACHINES[machine],
        "sha256": hashlib.sha256(data).hexdigest(),
        "bytes": dict(file=len(data), load_file=union_length(payload),
                      load_memory=memlen, load_backed=backedlen,
                      zero_fill=memlen - backedlen, alloc=alloclen,
                      padding=memlen - alloclen, bss=union_length(bss),
                      linked_stack=union_length(stack)),
    }


def artifact(path):
    try:
        with open(path, "rb") as stream:
            if not os.fstat(stream.fileno()).st_size:
                raise ValueError("empty file")
            with mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ) as data:
                result = measure(data)
    except (OSError, ValueError, OverflowError) as exc:
        raise ValueError(f"{path}: {exc}") from exc
    return dict(path=path, **result)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--feature", action="append", required=True, metavar="NAME=PATH")
    args = parser.parse_args(argv)
    try:
        baseline = artifact(args.baseline)
        features = {}
        for item in args.feature:
            name, _, path = item.partition("=")
            if not name.strip() or not path or name in features:
                raise ValueError("features require a unique nonempty NAME and PATH")
            feature = artifact(path)
            if baseline["machine"] != feature["machine"]:
                raise ValueError(f"{name}: machine differs from baseline")
            feature["delta_bytes"] = {
                key: value - baseline["bytes"][key]
                for key, value in feature["bytes"].items()
            }
            features[name] = feature
    except ValueError as exc:
        parser.error(str(exc))
    print(json.dumps(dict(schema_version=1, baseline=baseline, features=features),
                     indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
