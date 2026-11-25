#!/usr/bin/env python3
"""Read terminal M4 telemetry and CRC results from a native two-process capture."""
import argparse
import json
from pathlib import Path
import re
import struct
import zlib

from native_el0_replay import Snapshot, elf_file, need, permissions, symbol, vread
from ordinary_amc_capture import sha


def values(image, mem, root, name, fmt):
    va, size = symbol(image, name)
    need(size == struct.calcsize("<" + fmt), f"unexpected symbol size for {name}")
    permissions(mem, root, va, "user rw")
    return struct.unpack("<" + fmt, vread(mem, root, va, size))


def read_results(package, capture):
    manifest = json.loads((package / "manifest.json").read_text())
    launch = json.loads((capture / "launch.json").read_text())
    session = json.loads((capture / "session.json").read_text())
    need(launch["argument_package"] == manifest, "package differs from captured manifest")
    for name in ("launch.json", "uart.bin", "ram.bin", "kernel.elf", "user.elf"):
        need(sha(capture / name) == session["hashes"][name], f"capture digest mismatch for {name}")
    slots = manifest["slots"]
    need(manifest["policy"] in ("mlq", "mlfq") and len(slots) == 2, "expected two multilevel slots")
    kernel = capture / "kernel.elf"
    need(sha(kernel) == manifest["kernel_elf_sha256"], "kernel identity")
    mem = Snapshot(launch["ram_base"], (capture / "ram.bin").read_bytes())
    need(len(mem.raw) == launch["ram_bytes"], "incomplete RAM snapshot")
    uart = (capture / "uart.bin").read_bytes()
    exits = [(int(pid), int(code)) for pid, code in re.findall(
        rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$", uart)]
    need(sorted(pid for pid, _ in exits) == [0, 1], "missing or duplicate process exit")
    statuses = dict(exits)
    k = elf_file(kernel.read_bytes())
    spaces = [s for s in k.get_section_by_name(".symtab").iter_symbols()
              if "native_el0_mmu" in s.name and "6SPACES" in s.name]
    need(len(spaces) == 1 and spaces[0]["st_size"] % 2 == 0, "private address space layout")
    images = []
    roots = []
    for i, slot in enumerate(slots):
        path = package / ("user.elf" if i == 0 else "native_peer.elf")
        digest = sha(path)
        need(slot["slot"] == i and digest == slot["elf_sha256"], "slot ELF identity")
        prefix = "NATIVE EL0 " + ("PEER " if i else "") + "ELF "
        need((prefix + digest).encode() in uart.splitlines(), "guest ELF identity")
        images.append(elf_file(path.read_bytes()))
        roots.append(spaces[0]["st_value"] + i * (spaces[0]["st_size"] // 2))

    def reporter(name, fmt):
        return values(images[0], mem, roots[0], name, fmt)

    def crc(name, fmt):
        return values(images[1], mem, roots[1], name, fmt)[0]

    bound, = reporter("publication_deadline_ns", "Q")
    fields = {name: reporter(name, fmt) for name, fmt in (
        ("request_ns", "3Q"), ("observed_ns", "3Q"), ("publication_ns", "3Q"),
        ("publication_marker", "3I"), ("publication_verdict", "3I"),
        ("round_count", "3I"), ("cumulative_count", "3I"))}
    rounds = []
    for i in range(3):
        row = {name: data[i] for name, data in fields.items()}
        sampled = row["publication_marker"] == i + 1 and row["publication_ns"] != 0
        delta = row["publication_ns"] - row["request_ns"] if sampled else None
        row.update(round=i + 1, sampled_delay_ns=delta,
                   sampled_margin_ns=bound - delta if delta is not None else None)
        rounds.append(row)
    blocks = crc("private_blocks", "Q")
    need(blocks <= 1 << 20, "CRC block count exceeds application limit")
    ramp = bytes(range(256)) * 16
    expected = 0
    for _ in range(blocks):
        expected = zlib.crc32(ramp, expected)
    actual = crc("private_result", "I")
    size = crc("private_bytes", "Q")
    return dict(policy=manifest["policy"], kernel_sha256=manifest["kernel_elf_sha256"],
                slots=slots, reporter_exit=statuses[0], deadline_ns=bound, rounds=rounds,
                reporter_blocks=reporter("private_blocks", "I")[0],
                reporter_bytes=reporter("private_bytes", "Q")[0],
                reporter_result=reporter("private_result", "I")[0], crc_exit=statuses[1],
                crc_blocks=blocks, crc_bytes=size, crc32=actual, expected_crc32=expected,
                crc_matches=(actual == expected and size == len(ramp) * blocks))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("capture", type=Path)
    args = parser.parse_args()
    print(json.dumps(read_results(args.package, args.capture), indent=2))
