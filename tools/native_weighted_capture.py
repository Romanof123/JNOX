#!/usr/bin/env python3
"""Capture one weighted or cyclic-partition boot: ARM CLOCK NEW_DIR KERNEL_ELF USER_ELF PROBE_C SOURCE_REV."""
import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys

from native_el0_capture import command, observe, symbol_address
from ordinary_amc_capture import save, sha

SUCCESS = "END"
TERMINAL = rb"(?m)^NATIVE VRUNTIME (END|FAIL)\n"

ARMS = ("share", "wake")
PARTITIONS = ("partition-p", "partition-q")
CLOCKS = ("default", "icount3")


def qemu_command(qemu, root, clock, timer_observation=False):
    cmd = command(qemu, root)
    if clock == "icount3":
        cmd[1:1] = ["-icount", "shift=3,align=off,sleep=off"]
    if timer_observation:
        from native_weighted_timer import EVENTS

        cmd[cmd.index("-d") + 1] = "int," + ",".join("trace:" + name for name in EVENTS)
        cmd[1:1] = ["-S", "-msg", "timestamp=on"]
    return cmd


def capture(args):
    if not re.fullmatch("[0-9a-f]{40}", args.revision):
        raise ValueError("full source revision required")
    root = args.root.resolve()
    root.mkdir()
    for src, name in (
        (args.kernel, "kernel.elf"),
        (args.user, "user.elf"),
        (args.probe, "probe.c"),
    ):
        shutil.copyfile(src, root / name)
    table = subprocess.check_output(
        [args.nm, "-nS", str(root / "kernel.elf")], text=True
    )
    base = symbol_address(table, "_start")
    limit = symbol_address(table, "__stack_top")
    if base != 0x40200000 or not base < limit <= base + (16 << 20):
        raise ValueError("bounded physical RAM extent")
    qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
    observer = None
    if args.timer_observation:
        from native_weighted_timer import EVENTS, HostObserver, PIN

        if (args.arm, args.clock) != ("share", "default") or sha(Path(qemu)) != PIN:
            raise ValueError("timer observer requires pinned default share QEMU")
        available = subprocess.check_output(
            [qemu, "-trace", "help"], text=True
        ).splitlines()
        if not set(EVENTS).issubset(available):
            raise ValueError("required timer/GIC tracepoints unavailable")
        observer = HostObserver(root)
    cmd = qemu_command(qemu, root, args.clock, args.timer_observation)
    save(
        root / "launch.json",
        dict(
            version=1,
            **({"timer_observation": 1} if args.timer_observation else {}),
            arm=args.arm,
            clock=args.clock,
            argv=cmd,
            qemu_sha256=sha(Path(qemu)),
            qemu_version=subprocess.check_output([qemu, "--version"], text=True),
            revision=args.revision,
            ram_base=base,
            ram_bytes=limit - base,
            kernel_sha256=sha(root / "kernel.elf"),
            user_sha256=sha(root / "user.elf"),
            probe_sha256=sha(root / "probe.c"),
        ),
    )
    terminal = rb"(?m)^NATIVE PARTITION (END|FAIL)\n" if args.arm in PARTITIONS else TERMINAL
    return observe(root, cmd, base, limit, terminal, SUCCESS, observer)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("arm", choices=(*ARMS, *PARTITIONS))
    parser.add_argument("clock", choices=CLOCKS)
    for name in ("root", "kernel", "user", "probe"):
        parser.add_argument(name, type=Path)
    parser.add_argument("revision")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timer-observation", action="store_true")
    parser.add_argument("--nm", default="aarch64-linux-gnu-nm")
    sys.exit(capture(parser.parse_args()))
