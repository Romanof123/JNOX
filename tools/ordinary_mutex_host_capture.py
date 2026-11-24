"""Capture four cold ordinary images under Linux/QEMU 6.2.0. Output must be new."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import zipfile

PLUGIN_REF = "663b598e5976d8d8578153b4716c677d727f80b8"
ARCHIVE_REF = "317d7365ad966441453eabb38aeab1c78e751f7b"
ELFS = {
    "pi": "fc5be57112e78bb9a4d2de6bb4fc17078331e69c7b26eac10660c9eeea6075ce",
    "ceiling": "172a88efd01ebda34e6cd05bc32ab4318f814cdec53f40c95d98dc9fb852727c",
}
SITES = {
    "pi": ["40201aa0:d5033f5f", "40201aa4:d69f03e0", "40202480:b5000040",
           "40202ba0:c89ffd6f", "40202ba4:d503201f", "40201280:14000161"],
    "ceiling": ["40201aa0:d5033f5f", "40201aa4:d69f03e0", "40202554:b5000040",
                "40202cc8:c89ffd8f", "40202ccc:d503201f", "40201280:14000161"],
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify_sites(image, app):
    phoff, = struct.unpack_from("<Q", image, 32)
    size, count = struct.unpack_from("<HH", image, 54)
    segments = [struct.unpack_from("<II6Q", image, phoff+i*size) for i in range(count)]
    for spec in SITES[app]:
        pc, expected = (int(field, 16) for field in spec.split(":"))
        (segment,) = [s for s in segments if s[0] == 1 and s[1] & 1
                      and s[3] <= pc <= s[3]+s[5]-4]
        actual, = struct.unpack_from("<I", image, segment[2]+pc-segment[3])
        if actual != expected:
            raise ValueError("linked instruction " + spec)


def capture(root):
    if os.getuid() == 0:
        raise ValueError("ordinary user required")
    root.mkdir()
    root = root.resolve()
    version = subprocess.check_output(["qemu-system-aarch64", "--version"], text=True)
    (root / "qemu.version").write_text(version)
    if not version.startswith("QEMU emulator version 6.2.0 "):
        raise ValueError("QEMU version")
    archive = subprocess.check_output(["git", "show", ARCHIVE_REF +
                                       ":docs/evidence/ordinary-timed-chain-integration.zip"])
    if digest(archive) != "fa89510c77757edbcbd70c300c69663f5e7220d31a1303f84015c6207db362fd":
        raise ValueError("archive digest")
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        for app, expected in ELFS.items():
            image = z.read("ordinary-" + app + "/kernel.elf")
            if digest(image) != expected:
                raise ValueError("ELF digest")
            verify_sites(image, app)
            (root / (app + ".elf")).write_bytes(image)
        (root / "control_replay.py").write_bytes(z.read("control_replay.py"))
    source = subprocess.check_output(["git", "show", PLUGIN_REF + ":tools/native_weighted_marker.c"])
    (root / "marker.c").write_bytes(source)
    subprocess.run(["cc", "-shared", "-fPIC", "-O2", "marker.c", "-o", "marker.so"],
                   cwd=root, check=True)
    manifest = []
    for i, (app, observed) in enumerate((app, observed) for app in ELFS for observed in (False, True)):
        label = "observed" if observed else "unobserved"
        d = root / f"0{i}-{app}-{label}"
        d.mkdir()
        cmd = ["timeout", "-k", "2s", "15s", "qemu-system-aarch64", "-machine",
               "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1", "-accel", "tcg",
               "-m", "128M", "-nodefaults", "-display", "none", "-monitor", "none",
               "-serial", "stdio", "-d", "int", "-D", "external.log"]
        if observed:
            markers = [f"site{j}={s}" for j, s in enumerate(SITES[app])]
            cmd += ["-plugin", ",".join(["../marker.so", "out=marker.bin", *markers])]
        cmd += ["-kernel", "../" + app + ".elf"]
        (d / "command.json").write_text(json.dumps(cmd))
        with (d / "uart").open("xb") as uart, (d / "stderr").open("xb") as err:
            rc = subprocess.run(cmd, cwd=d, stdin=subprocess.DEVNULL, stdout=uart, stderr=err).returncode
        (d / "rc").write_text(str(rc))
    for p in sorted(root.rglob("*")):
        if p.is_file():
            manifest.append(f"{digest(p.read_bytes())}  {p.relative_to(root).as_posix()}")
    (root / "SHA256SUMS").write_text("\n".join(manifest) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("new_directory", type=Path)
    capture(parser.parse_args().new_directory)
