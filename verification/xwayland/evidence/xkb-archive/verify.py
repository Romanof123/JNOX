#!/usr/bin/env python3
"""Verify retained XKB records and, when supplied, the original two binaries."""
import argparse
import hashlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
BINARIES = {"libxkbfile.a", "probe-archive.elf"}
RECORDS = {"attempt01.log", "build.log", "probe-archive.map", "artifacts.sha256",
           "xkb-sources.sha256", "source-commit", "probe.headers", "probe.symbols",
           "probe.undefined", "archive.members"}

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binaries", type=Path, help="directory containing the two original build products")
args = parser.parse_args()
entries = {}
for line in (HERE / "SHA256SUMS").read_text().splitlines():
    digest, name = line.split()
    if name in entries:
        raise SystemExit(f"duplicate checksum: {name}")
    entries[name] = digest
if set(entries) != RECORDS | BINARIES:
    raise SystemExit("incomplete or unexpected checksum inventory")
for name, digest in entries.items():
    root = args.binaries if name in BINARIES else HERE
    if root is not None and hashlib.sha256((root / name).read_bytes()).hexdigest() != digest:
        raise SystemExit(f"hash mismatch: {name}")
members = (HERE / "archive.members").read_text().splitlines()
if len(members) != 13 or len(set(members)) != 13:
    raise SystemExit("expected 13 original object files")
if (HERE / "probe.undefined").stat().st_size != 0:
    raise SystemExit("unexpected unresolved probe symbols")
pairs = (("maprules.c.o", "XkbRF_Create"), ("maprules.c.o", "XkbRF_Free"),
         ("xkbtext.c.o", "XkbModIndexText"))
link_map = (HERE / "probe-archive.map").read_text()
symbols = (HERE / "probe.symbols").read_text().splitlines()
for obj, symbol in pairs:
    if f"libxkbfile.a({obj}):(.text.{symbol})" not in link_map or not any(
            line.endswith(" T " + symbol) for line in symbols):
        raise SystemExit(f"missing linked origin for {symbol}")
print("Checked retained XKB build records" + (" and both original binaries." if args.binaries else "."))
