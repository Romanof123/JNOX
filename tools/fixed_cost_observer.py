"""Decode stopped fixed-cost capture. Operation numbers match SVC immediates; 3 is IRQ."""
import argparse
import json
import struct
from pathlib import Path

STACK = 16384
FRAME = 832
OPS = ("yield", "exit", "sleep", "irq")


def require(test, message):
    if not test:
        raise ValueError(message)


def decode(profile, records, stacks, base):
    require(len(records) == (5 + 128 * 7) * 8, "journal size")
    hz, first, second, count, dropped = struct.unpack_from("<5Q", records)
    require(hz > 0 and first <= second, "counter metadata")
    require(0 < count <= 128 and dropped == 0, "empty or overflowing journal")
    require(len(stacks) == STACK * 5 and base % 16 == 0, "stack geometry")
    used = {}
    for slot in range(5):
        data = stacks[slot * STACK:(slot + 1) * STACK]
        if slot == 3:
            require(not any(data), "unconfigured stack touched")
            continue
        depth = len(data.lstrip(b"\xa5"))
        require(FRAME <= depth < STACK, "missing frame or exhausted paint")
        used[slot] = {"base": base + slot * STACK, "bytes": STACK, "changed_depth": depth}

    groups = {}
    previous = second
    for op, incoming, outgoing, start, before, after, end in struct.iter_unpack(
            "<7Q", records[40:40 + count * 56]):
        require(op < len(OPS) and previous <= start <= before <= after <= end, "event interval")
        previous = end
        slots = []
        for address in (incoming, outgoing):
            slot, offset = divmod(address - base, STACK)
            require(slot in used and offset <= STACK - FRAME and address % 16 == 0, "frame bounds")
            slots.append(slot)
        require((slots[0] == slots[1]) == (incoming == outgoing), "frame identity")
        key = OPS[op] + ("_switch" if incoming != outgoing else "_return")
        group = groups.setdefault(key, {"samples": 0, "handler_max_ticks": 0, "event_max_ticks": 0})
        group["samples"] += 1
        group["handler_max_ticks"] = max(group["handler_max_ticks"], end - start)
        group["event_max_ticks"] = max(group["event_max_ticks"], after - before)
    require("irq_switch" in groups and "exit_switch" in groups, "missing IRQ or exit switch")
    if profile == "fixed-fifo":
        require({"yield_switch", "sleep_switch", "sleep_return"} <= groups.keys(), "FIFO coverage")
    return dict(frequency_hz=hz, calibration_ticks=second-first, groups=groups, stacks=used)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", choices=("fixed-fifo", "fixed-rr"))
    parser.add_argument("capture", type=Path)
    args = parser.parse_args()
    directory = args.capture
    symbols = json.loads((directory / "launch.json").read_text())["extra_symbols"]
    qmp = json.loads((directory / "qmp.json").read_text())
    require(len(qmp["cpus"]) == 1 and qmp["status"]["running"], "CPU shape")
    raw = {}
    for name in ("fixed_cost_stacks", "fixed_cost_records"):
        raw[name] = (directory / (name + ".bin")).read_bytes()
        require(len(raw[name]) == symbols[name][1], "linked symbol size")
    print(json.dumps(decode(args.profile, raw["fixed_cost_records"], raw["fixed_cost_stacks"],
                            symbols["fixed_cost_stacks"][0]), indent=2))


if __name__ == "__main__":
    main()
