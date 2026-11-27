#!/usr/bin/env python3
"""Independent straight-line integer reference for the native Pixman probe."""
import argparse
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


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("uart", type=Path)
    p.add_argument("--assertion", action="store_true")
    a = p.parse_args()
    text = a.uart.read_text(errors="replace").replace("\r", "")
    require("NATIVE EL0 FAULT" not in text, "unexpected native fault")
    exits = sorted(re.findall(r"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)$", text))
    if a.assertion:
        require(exits == [("0", "134")], "assertion must terminate via native exit 134")
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
