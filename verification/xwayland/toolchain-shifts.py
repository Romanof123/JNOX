"""Select the two AArch64 integer shift helpers used by Picolibc's Ryu code."""

import hashlib
import os
from pathlib import Path
import subprocess
import sys


MEMBERS = {
    "_ashldi3.o": ("__ashlti3", "dba7a046c71221b69d953307b1ced377092a68ab7a1494ae2b22d1170ce8aecd"),
    "_lshrdi3.o": ("__lshrti3", "4659dcf30fc80117ce58838c39362c95829c2d9ddfa8f2819183d55b12e129e0"),
}


def build(output):
    provider = os.environ.get("JNOX_AARCH64_LIBGCC")
    if not provider:
        provider = subprocess.check_output(
            ["aarch64-linux-gnu-gcc-11", "-print-libgcc-file-name"], text=True
        ).strip()
    if not Path(provider).is_file():
        raise ValueError("target libgcc archive is missing")
    output.mkdir()
    objects = []
    for member, (symbol, digest) in MEMBERS.items():
        data = subprocess.check_output(["ar", "p", provider, member])
        if hashlib.sha256(data).hexdigest() != digest:
            raise ValueError(f"target libgcc member hash mismatch for {member}")
        obj = output / member
        obj.write_bytes(data)
        exports = subprocess.check_output(["nm", "-g", "-P", obj], text=True).splitlines()
        if [line.split()[:2] for line in exports] != [[symbol, "T"]]:
            raise ValueError(f"unexpected symbols in {member}")
        objects.append(obj)
    archive = output / "libjnox-shifts.a"
    subprocess.run(["ar", "rcsD", archive, *objects], check=True)
    return archive


if __name__ == "__main__":
    print(build(Path(sys.argv[1]).resolve()))
