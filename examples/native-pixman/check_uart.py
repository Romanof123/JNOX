#!/usr/bin/env python3
"""Independent straight-line integer reference for the native Pixman probe."""
import argparse
import json
from pathlib import Path
import re


def expected(src, dst):
    # Premultiplied Porter-Duff OVER, nearest integer channel rounding.
    alpha = src >> 24
    result = 0
    for shift in (0, 8, 16, 24):
        channel = (src >> shift & 255) + ((dst >> shift & 255) * (255 - alpha) + 127) // 255
        result |= min(255, channel) << shift
    return result


def require(value, context):
    if not value:
        raise ValueError(context)


def check_trace(log, assertion):
    names = json.loads(log.with_suffix(".symbols.json").read_text())
    lines = log.with_suffix(".trace").read_bytes().splitlines()
    pattern = re.compile(rb"^Trace 0: 0x[0-9a-f]+ \[[0-9a-f]+/([0-9a-f]+)/", re.I)
    entries = []
    for i,line in enumerate(lines):
        match = pattern.match(line)
        if match and (i+1 == len(lines) or not lines[i+1].startswith(b"Stopped execution of TB chain before")):
            entries.append(int(match[1],16))
    expected = {"pixman_constructor": 1 if assertion else 2, "graphics_fp_yield": 0 if assertion else 64,
                "pixman_image_composite32": 0 if assertion else 2, "abort": 1 if assertion else 0}
    for name, count in expected.items():
        require(entries.count(names[name][0]) == count, "unexpected executed entry count for "+name)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("uart", type=Path)
    mode = p.add_mutually_exclusive_group()
    mode.add_argument("--assertion", action="store_true")
    mode.add_argument("--console-grant", action="store_true")
    p.add_argument("--trace", action="store_true")
    a = p.parse_args()
    if a.trace and not a.console_grant: check_trace(a.uart, a.assertion)
    text = a.uart.read_text(errors="replace").replace("\r", "")
    require("NATIVE EL0 FAULT" not in text, "unexpected native fault")
    exits = sorted(re.findall(r"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)$", text))
    if a.console_grant:
        require(exits == [("0","73"),("1","74")], "granted vs denied native SVC outcomes")
        require(text.splitlines().count("JNOX CONSOLE GRANT OK") == 1 and "JNOX CONSOLE LEAK" not in text,
                "console data must only be emitted by the granted slot")
        return
    if a.assertion:
        require(exits == [("0", "134")] or (not exits and re.findall(r"(?m)^NATIVE EL0 EXIT (-?\d+)$", text) == ["134"]), "assertion must terminate via native exit 134")
        require("JNOX GRAPHICS ABORT\n" in text and "role != 'a'" in text, "missing real assertion diagnostic")
        return
    require(exits == [("0","71"),("1","72")], "missing native process exits")
    begins, ends = [], []
    for role, first in (("p", 0x80402010), ("q", 0x80201040)):
        sources = (first, 0, 0xff1267ab, 0x400f1001)
        dest = (0xff204060, 0xff123456, 0xff987654, 0xff346789)
        words = [expected(s,d) for s,d in zip(sources,dest)]
        line = "JNOX PIXMAN " + role + " " + " ".join(f"{v:08x}" for v in words)
        require(text.splitlines().count(line) == 1, "pixel mismatch in " + role)
        begin = text.index("JNOX PIXMAN FP BEGIN "+role+"\n")
        end = text.index("JNOX PIXMAN FP END "+role+"\n")
        fini = text.index("JNOX PIXMAN FINI "+role+"\n")
        require(begin < end < text.index(line) < fini, "startup / FP / pixels / finalization order")
        begins.append(begin)
        ends.append(end)
    require(max(begins) < min(ends), "no observed overlap of FP register workloads")


if __name__ == "__main__":
    main()
