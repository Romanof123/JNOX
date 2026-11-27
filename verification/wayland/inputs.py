#!/usr/bin/env python3
"""Extract the pinned Ubuntu packages with dpkg-deb, or check a prepared tree."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from urllib.request import urlopen


manifest = json.loads(Path(__file__).with_name("upstream.json").read_text())


def verify(path, expected):
    with path.open("rb") as stream:
        actual = hashlib.sha256(stream.read()).hexdigest()
    if actual != expected:
        raise SystemExit(f"{path}: SHA-256 {actual}, expected {expected}")


def check(root):
    for name, expected in manifest["files"].items():
        verify(root / name, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare")
    prepare.add_argument("destination", type=Path, help="a new directory")
    prepare.add_argument("--debs", type=Path, help="use existing archives offline")
    commands.add_parser("check").add_argument("sysroot", type=Path)
    args = parser.parse_args()
    if args.command == "check":
        check(args.sysroot)
        return

    args.destination.mkdir()
    root = args.destination / "sysroot"
    root.mkdir()
    archives = args.debs
    if archives is None:
        archives = args.destination / "debs"
        archives.mkdir()
    for uri, expected in manifest["packages"].items():
        path = archives / Path(uri).name
        if args.debs is None:
            with urlopen(manifest["archive"] + uri, timeout=60) as source, path.open("xb") as out:
                shutil.copyfileobj(source, out)
        verify(path, expected)
        subprocess.run(["dpkg-deb", "-x", path, root], check=True)
    check(root)


if __name__ == "__main__":
    main()
