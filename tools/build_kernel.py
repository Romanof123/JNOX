#!/usr/bin/env python3
"""Build the kernel for an architecture registered under arch/<name>/target."""

import argparse
import os
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]


def add_kernel_profile(parser):
    parser.add_argument("--kernel-profile", choices=("release", "speed"), default="release",
                        help="Cargo kernel profile: compact release (default) or speed")


def cargo_command(target, profile):
    selection = ["--release"] if profile == "release" else ["--profile", profile]
    return ["cargo", "build", "--locked", *selection, "-j1", "--target", target]


def read_registry(root):
    """Each descriptor contains one built-in Rust target triple."""
    entries = {}
    for path in sorted((root / "arch").glob("*/target")):
        name = path.parent.name
        target = path.read_text(encoding="utf-8").strip()
        if not re.fullmatch(r"[a-z0-9_]+(?:-[a-z0-9_]+)+", target):
            raise ValueError(f"invalid Rust target triple in {path}")
        entries[name] = target
    if not entries:
        raise ValueError(f"no architecture descriptors found under {root / 'arch'}")
    return entries


def main(argv=None, *, root=ROOT, environ=None, run=subprocess.run):
    parser = argparse.ArgumentParser(description=__doc__)
    add_kernel_profile(parser)
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--list-arches", action="store_true",
                           help="list implemented architecture names, one per line")
    selection.add_argument("--arch", metavar="NAME",
                           help="select architecture (default: JNOX_ARCH, then aarch64)")
    args = parser.parse_args(argv)
    try:
        entries = read_registry(root)
    except (OSError, ValueError) as exc:
        parser.exit(2, f"{parser.prog}: {exc}\n")
    if args.list_arches:
        print("\n".join(entries))
        return 0

    env = dict(os.environ if environ is None else environ)
    arch = args.arch if args.arch is not None else env.get("JNOX_ARCH", "aarch64")
    if arch not in entries:
        parser.error(f"unsupported architecture {arch!r}; choose from {', '.join(entries)}")
    target = entries[arch]
    if "CARGO_BUILD_TARGET" in env and env["CARGO_BUILD_TARGET"] != target:
        parser.error(f"CARGO_BUILD_TARGET={env['CARGO_BUILD_TARGET']!r} conflicts with "
                     f"architecture {arch!r}, which requires {target!r}")
    env["JNOX_ARCH"] = arch
    command = cargo_command(target, args.kernel_profile)
    try:
        result = run(command, cwd=root / "kernel", env=env)
    except OSError as exc:
        print(f"{parser.prog}: could not launch Cargo: {exc}", file=sys.stderr)
        return 127 if isinstance(exc, FileNotFoundError) else 1
    return result.returncode if result.returncode >= 0 else 128 - result.returncode


if __name__ == "__main__":
    sys.exit(main())
