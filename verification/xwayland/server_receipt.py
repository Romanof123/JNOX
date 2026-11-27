"""Verify the static JNOX Xwayland link and record its original ELF digest."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess


def verify(server_dir, *, resident=False):
    target = "hw/xwayland/Xwayland"
    elf = server_dir / "xwayland-build" / target
    headers = subprocess.check_output(
        ["llvm-readelf", "--file-header", "--program-headers", "--relocations", elf], text=True)
    if (not re.search(r"^\s*Machine:\s+AArch64$", headers, re.M)
            or not re.search(r"^\s*Type:\s+EXEC\b", headers, re.M)
            or re.search(r"^\s*(INTERP|DYNAMIC|TLS)\s", headers, re.M)
            or "There are no relocations in this file." not in headers):
        raise ValueError("expected a fully relocated static AArch64 Xwayland executable")
    undefined = subprocess.check_output(["llvm-nm", "--undefined-only", elf], text=True)
    # Picolibc 1.8.10 strerror.c checks the weak address for NULL before calling
    # this optional application extension for an unknown errno.
    for line in undefined.splitlines():
        if line.split() != ["w", "_user_strerror"]:
            raise ValueError("unresolved Xwayland symbol: " + line.strip())
    if resident:
        symbols = subprocess.check_output(["llvm-nm", "-P", "--defined-only", elf], text=True)
        bounds = {name: int(address, 16) for name, address in
                  re.findall(r"^(__heap_start|__heap_end) B ([0-9a-fA-F]+)\b", symbols, re.M)}
        if set(bounds) != {"__heap_start", "__heap_end"} or not (
                0x400000 <= bounds["__heap_start"] < bounds["__heap_end"] <= 0x800000
                and bounds["__heap_end"] - bounds["__heap_start"] == 2 * 1024 * 1024):
            raise ValueError("resident Xwayland requires a 2 MiB heap in its native image window")
    digest = hashlib.sha256(elf.read_bytes()).hexdigest()
    (server_dir / "Xwayland.sha256").write_text(digest + "  " + target + "\n")
    return digest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("server_dir", type=Path)
    parser.add_argument("--resident", action="store_true", help="require the resident input memory profile")
    args = parser.parse_args()
    print(verify(args.server_dir, resident=args.resident))
