"""Check the frozen equal-peer receipt fields in a raw admission witness."""
import argparse
from pathlib import Path
import struct


def check(data, refused):
    words = struct.unpack("<30Q", data)
    if words[1] == 0 or words[2:6] != (4_000_000, 4_000_000, 24_000_000, 24_000_000 - int(refused)):
        raise ValueError("invalid frequency, quantum, allowance, bound or deadline")
    if refused:
        if words[0] != 2 or any(words[6:]):
            raise ValueError("refused profile ran a worker or recorded a failure")
        return
    epoch, start, first = words[6:9]
    services = words[9:13]
    gaps = start - epoch - sum(services)
    excess = max(0, services[2] - words[2])
    entry = first - start
    used = gaps + excess + entry
    if not (0 < epoch <= start <= first) or min(gaps, entry) < 0:
        raise ValueError("inconsistent epoch, owner charges or target counters")
    if (not all(0 < s <= 8_000_000 for s in services[:3]) or services[3] != 0
            or services[2] < words[2] or used > words[3] or first - epoch > words[5]):
        raise ValueError("measured demand, quantum or timing exceeds the frozen contract")
    if words[13:18] != (gaps, excess, entry, used, first - epoch):
        raise ValueError("raw counters do not reproduce the published timing decomposition")
    if words[18:20] != (34816, 382976) or words[22:24] != (15, 1):
        raise ValueError("unexpected high products, worker mask or first dispatch count")
    blocks = words[24]
    if words[0] != 4 or words[27] != 0:
        raise ValueError(f"state {words[0]}, failure stage {words[27]}")
    if words[21] != 16 or words[28] != words[21] or words[29] != blocks:
        raise ValueError("target product, published receipt or final block mismatch")
    if not 1 <= blocks <= 2_000_000 or words[20] != 2048 * blocks:
        raise ValueError("equal-peer reduction disagrees with its block count")
    if words[25] != 1 or words[26] < 1:
        raise ValueError("missing timer handoff or peer redispatch")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("witness", type=Path)
    parser.add_argument("--refused", action="store_true")
    args = parser.parse_args()
    check(args.witness.read_bytes(), args.refused)
