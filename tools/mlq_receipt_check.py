"""Check the frozen equal-peer receipt fields in a raw admission witness."""
import argparse
from pathlib import Path
import struct


def check(data, refused):
    words = struct.unpack("<30Q", data)
    if refused:
        if words[0] != 2 or any(words[6:]):
            raise ValueError("refused profile ran a worker or recorded a failure")
        return
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
