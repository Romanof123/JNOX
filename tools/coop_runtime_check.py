#!/usr/bin/env python3
"""Decode application observations from an independently captured linked guest."""
import argparse
import json
from pathlib import Path
import re
import struct

from qemu_boot import TRANSCRIPTS


def require(condition, message):
    if not condition:
        raise ValueError(message)


def decode(profile, raw):
    words = struct.unpack("<26Q", raw)
    dual = profile == "coop-events"
    cap = 2 if dual else 1
    require(words[:7] == (1, cap, cap, cap, 1, 1, 3 if dual else 2), "queue header")
    order = (11, 22, 33, 0) if dual else (11, 33, 0, 0)
    require(words[7:11] == order, "handler completion order")
    require(words[11:14] == (2, 1, 20), "owner responses, timer and peer order")
    rows = [words[14:18], words[18:22]]
    for row, counts in zip(rows, [(1, 0), (2, 2)] if dual else [(2, 2), (0, 0)]):
        calls, total, maximum, overruns = row
        require((calls, overruns) == counts, "per-handler call and overrun counts")
        if calls:
            require(0 < maximum <= total <= calls * maximum, "elapsed tick statistics")
            if overruns:
                require(total >= maximum + 64 and maximum >= 64, "two measured handler intervals")
        else:
            require((total, maximum) == (0, 0), "unused handler row")
    elapsed = words[22:26]
    require(all(elapsed[:words[6]]) and not any(elapsed[words[6]:]), "per-return intervals")
    assignments = [1, 0, 1] if dual else [0, 0]
    for handler, row in enumerate(rows):
        samples = [ticks for h, ticks in zip(assignments, elapsed) if h == handler]
        budget = (1 if handler == assignments[0] else (1 << 64) - 1)
        require(row == (len(samples), sum(samples), max(samples, default=0),
                        sum(ticks > budget for ticks in samples)), "independent counter reconstruction")
    return {"profile": profile, "queue_peak": cap, "order": list(order[:words[6]]),
            "owner_responses": words[11], "handler_statistics": rows, "elapsed": elapsed}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("profile", choices=["coop-events", "coop-single"])
    p.add_argument("capture", type=Path)
    args = p.parse_args()
    root = args.capture
    uart = (root / "uart.bin").read_bytes().replace(b"\r\n", b"\n")
    require(uart == TRANSCRIPTS[args.profile], "unexpected UART")
    trace = (root / "int.trace").read_text()
    exceptions = re.findall(r"Taking exception \d+ \[(IRQ|SVC)\]", trace)
    require(exceptions == ["IRQ", "SVC", "SVC", "SVC", "SVC"], "IRQ/yield/peer returns/owner return")
    irq = trace.split("Taking exception 5 [IRQ]", 1)[1].split("Taking exception", 1)[0]
    incoming = re.search(r"with ELR (0x[0-9a-fA-F]+)", irq)
    outgoing = re.search(r"Exception return from AArch64 EL1 to AArch64 EL1 PC (0x[0-9a-fA-F]+)", irq)
    require(incoming and outgoing and incoming[1] == outgoing[1], "IRQ retained interrupted owner PC")
    services = re.findall(r"Taking exception 2 \[SVC\][^\n]*\n[^\n]*\n\.\.\.with ESR 0x15/(0x[0-9a-fA-F]+)", trace)
    require([int(value, 16) & 0xffff for value in services] == [0, 1, 1, 1], "yield and exit immediates")
    report = decode(args.profile, (root / "audit.bin").read_bytes())
    print(json.dumps(report))


if __name__ == "__main__":
    main()
