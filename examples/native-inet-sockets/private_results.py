"""Read private application results from the retained QMP RAM snapshot."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
from native_amc_roots import linked_roots
from native_el0_replay import Snapshot, elf_file, permissions, symbol, vread


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("package", "capture", "tcp_elf", "udp_elf"):
        p.add_argument(name, type=Path)
    a = p.parse_args()
    kernel = a.package / "kernel.elf"
    meta = json.loads((a.package / "manifest.json").read_text())
    launch = json.loads((a.capture / "launch.json").read_text())
    if hashlib.sha256(kernel.read_bytes()).hexdigest() != launch["kernel_sha256"] or launch["kernel_sha256"] != meta["kernel_elf_sha256"]:
        raise ValueError("sealed kernel identity mismatch")
    roots = linked_roots(kernel, 3)
    mem = Snapshot(0x40000000, (a.capture / "ram.bin").read_bytes())
    results = {}
    for slot, (path, name, n) in enumerate(((a.tcp_elf, "inet_tcp_result", 12),
                                            (a.udp_elf, "inet_udp_result", 16))):
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != meta["slots"][slot]["elf_sha256"]:
            raise ValueError(f"slot {slot} ELF identity mismatch")
        va, size = symbol(elf_file(data), name)
        if size != n * 8:
            raise ValueError(f"wrong {name} extent")
        permissions(mem, roots[slot], va, "user rw")
        values = struct.unpack("<" + "q" * n, vread(mem, roots[slot], va, size))
        results[name] = values
    print(json.dumps(results, indent=2))
    tcp = results["inet_tcp_result"]
    udp = results["inet_udp_result"]
    return 0 if (tcp[:3] == (4,9,13) and 1 <= tcp[3] <= 3 and
                 tcp[4:] == (0,12,14,0,4,-1,104,1) and
                 udp[:10] == (3,3,3,3,1,4,3,0,0x0a000202,43004) and
                 udp[10] > 0 and udp[11:] == (1,0,0,0,0)) else 1


if __name__ == "__main__":
    raise SystemExit(main())
