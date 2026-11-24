#!/usr/bin/env python3
"""Measure a supplied AArch64 instruction path in a paired GNU objdump listing.

Usage: python tools/pi4_checkpoint_cost.py pair.json > report.json
Paths in JSON are relative to pair.json. Each "baseline" and "candidate" object
has elf, raw, raw_base (physical address of image byte zero), and path. Provide
a common "selection" describing the workload, source revision, iteration, lease,
and the starting and ending source expressions. Each path is an ordered list
of objects with first, last (inclusive instruction addresses), and optionally
why (source location, operand values and predicate for the outgoing conditional
edge). Repeat ranges for each loop traversal. Specify stop (exclusive next PC)
in each side. Use hex strings for addresses. Supply GNU AArch64 objdump via
--objdump. The listing must use GNU's single 8-hex-digit instruction words.

The report checks encoded edges and includes the selected ranges for audit.
A human must establish source correspondence, predicates, stack aliases, and
TLS/digest data flow. Stack counts cover instructions addressing SP directly.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


def number(value):
    return int(value, 0) if isinstance(value, str) else int(value)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def measure(side, directory, objdump):
    elf = directory / side["elf"]
    raw = (directory / side["raw"]).read_bytes()
    base = number(side["raw_base"])

    def dump(*args):
        return subprocess.check_output([objdump, *args, str(elf)], text=True)

    headers = dump("-h").splitlines()
    sections = []
    data = elf.read_bytes()
    for i, line in enumerate(headers[:-1]):
        fields = line.split()
        if len(fields) < 7 or not fields[0].isdigit():
            continue
        flags = headers[i + 1].split(",")
        if not {"CODE", "LOAD", "CONTENTS"} <= {flag.strip() for flag in flags}:
            continue
        size, vma, lma, offset = (int(x, 16) for x in fields[2:6])
        pos = lma - base
        require(0 <= pos <= len(raw) - size and 0 <= offset <= len(data) - size
                and raw[pos:pos + size] == data[offset:offset + size],
                f"ELF/raw mismatch in {fields[1]}")
        sections.append((fields[1], vma, size, offset))
    require(any(name == ".text" and size for name, _, size, _ in sections),
            "no loaded .text section")

    listing = {}
    for line in dump("-d", "-z").splitlines():
        m = re.match(r"\s*([0-9a-f]+):\s+([0-9a-f]{8})\s+(\S+)\s*(.*)", line)
        if m:
            addr, word, op, operands = m.groups()
            listing[int(addr, 16)] = (int(word, 16), op, operands)

    counts = dict(instructions=0, calls=0, returns=0, conditional=0,
                  sp_reads=0, sp_writes=0)
    frames = []
    ranges = side["path"]
    require(bool(ranges), "empty path")
    for index, span in enumerate(ranges):
        first, last = map(number, (span["first"], span["last"]))
        require(first <= last and first % 4 == last % 4 == 0, "invalid range")
        for pc in range(first, last + 1, 4):
            word, op, args = listing[pc]
            matches = [offset + pc - start for _, start, size, offset in sections
                       if start <= pc <= start + size - 4]
            require(len(matches) == 1 and
                    data[matches[0]:matches[0]+4] == word.to_bytes(4, "little"),
                    f"unmapped opcode at {pc:#x}")
            following = pc + 4
            if pc == last:
                following = (number(ranges[index + 1]["first"]) if index + 1 < len(ranges)
                             else number(side["stop"]))
            counts["instructions"] += 1
            if re.search(r"\[sp(?:,|\])", args):
                if op.startswith(("ld", "st")):
                    counts["sp_reads" if op.startswith("ld") else "sp_writes"] += 1
                else:
                    raise ValueError(f"unclassified SP access at {pc:#x}")
            if op == "ret":
                require(args.strip() in ("", "x30") and frames and frames.pop() == following,
                        f"unmapped return at {pc:#x}")
                counts["returns"] += 1
            elif op in ("b", "bl", "cbz", "cbnz", "tbz", "tbnz") or op.startswith("b."):
                target = int(args.split("<")[0].split(",")[-1].strip(), 16)
                if op in ("b", "bl"):
                    require(following == target, f"wrong direct edge at {pc:#x}")
                    if op == "bl":
                        frames.append(pc + 4)
                        counts["calls"] += 1
                else:
                    require(pc == last and str(span.get("why", "")).strip()
                            and following in (pc + 4, target),
                            f"conditional edge needs an endpoint and source evidence at {pc:#x}")
                    counts["conditional"] += 1
            else:
                require(op not in ("br", "blr", "eret", "svc", "hvc", "smc", "brk", "hlt")
                        and not op.startswith((".","bc.","bra","blr","ret","dcps","udf"))
                        and following == pc + 4, f"unmapped edge at {pc:#x}")
    require(not frames, "path ends inside a selected callee")
    return dict(elf_sha256=hashlib.sha256(data).hexdigest(),
                raw_sha256=hashlib.sha256(raw).hexdigest(), raw_bytes=len(raw),
                loaded_code_bytes={n: s for n, _, s, _ in sections},
                counts=counts, path=ranges, stop=side["stop"])


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("pair", type=Path)
    parser.add_argument("--objdump", default="aarch64-linux-gnu-objdump")
    options = parser.parse_args()
    spec = json.loads(options.pair.read_text(encoding="utf-8"))
    require(isinstance(spec["selection"], str) and spec["selection"].strip(),
            "missing source selection")
    report = {key: measure(spec[key], options.pair.parent, options.objdump)
              for key in ("baseline", "candidate")}
    report["selection"] = spec["selection"]
    report["delta"] = {k: v - report["baseline"]["counts"][k]
                       for k, v in report["candidate"]["counts"].items()}
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (KeyError, IndexError, TypeError, ValueError, OSError, subprocess.SubprocessError) as exc:
        sys.exit(f"cost unestablished: {exc}")
