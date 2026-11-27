"""Verify the static JNOX Xwayland link and record its original ELF digest."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess


def verify(server_dir):
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
    digest = hashlib.sha256(elf.read_bytes()).hexdigest()
    (server_dir / "Xwayland.sha256").write_text(digest + "  " + target + "\n")
    return digest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("server_dir", type=Path)
    print(verify(parser.parse_args().server_dir))
