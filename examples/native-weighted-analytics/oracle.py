"""Compute weighted-analytics private products and exits from the frozen inputs."""
import json
import math
import zlib


def expected_products():
    ramps = 524288
    block = bytes(range(256)) * 256
    crc = 0
    for _ in range(ramps // 256):
        crc = zlib.crc32(block, crc)

    blocks = 2097152
    bins = [blocks * math.comb(8, k) for k in range(9)]
    score = sum((k + 1) ** 2 * count for k, count in enumerate(bins))
    frames = 4194304
    energy = frames * sum(i * i for i in range(-128, 128))
    return {
        "checksum": {"progress": ramps * 256, "result": crc, "exit": crc % 251},
        "histogram": {"progress": blocks * 256, "result": bins, "exit": score % 251},
        "energy": {"progress": frames * 256, "result": energy, "exit": energy % 251},
        "reporter": {"progress": 3, "result": 7, "exit": 7, "wait_ns": [40000000] * 3},
        "overflow": {"progress": 3, "result": sum(range(256)), "exit": 61,
                     "overflow_return": 75, "normal_return": 0, "wait_ns": 40000000,
                     "distant_seconds": 160000000000, "distant_nanoseconds": 0},
    }


if __name__ == "__main__":
    print(json.dumps(expected_products(), indent=2))
