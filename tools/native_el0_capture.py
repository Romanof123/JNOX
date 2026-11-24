#!/usr/bin/env python3
"""Capture one cold native-process boot: NEW_DIR KERNEL_ELF USER_ELF PROBE_C SOURCE_REV."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import threading
import time

from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import request


def symbol_address(symbols, name):
    matches = [
        line.split() for line in symbols.splitlines() if line.split()[-1:] == [name]
    ]
    if len(matches) != 1:
        raise ValueError(f"missing or ambiguous symbol {name}")
    return int(matches[0][0], 16)


def command(qemu, root):
    return [
        qemu,
        *FLAGS,
        "-qmp",
        "stdio",
        "-serial",
        "file:" + str(root / "uart.bin"),
        "-d",
        "int",
        "-D",
        str(root / "exceptions.log"),
        "-kernel",
        str(root / "kernel.elf"),
    ]


def capture(args):
    if not re.fullmatch("[0-9a-f]{40}", args.revision):
        raise ValueError("full source revision required")
    root = args.root.resolve()
    root.mkdir()
    for src, name in [
        (args.kernel, "kernel.elf"),
        (args.user, "user.elf"),
        (args.probe, "probe.c"),
    ]:
        shutil.copyfile(src, root / name)
    symbols = subprocess.check_output(
        [args.nm, "-nS", str(root / "kernel.elf")], text=True
    )
    base = symbol_address(symbols, "_start")
    limit = symbol_address(symbols, "__stack_top")
    if base != 0x40200000 or not base < limit <= base + (16 << 20):
        raise ValueError("unexpected linked kernel RAM extent")
    qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
    cmd = command(qemu, root)
    save(
        root / "launch.json",
        dict(
            version=1,
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
    return observe(
        root,
        cmd,
        base,
        limit,
        rb"(?m)^NATIVE EL0 (EXIT -?[0-9]+|FAULT [0-9]+)\r?\n",
        "EXIT 37",
    )


def observe(root, cmd, base, limit, terminal_pattern, success, observer=None):
    """Retain one bounded QEMU process, its UART, exception trace and physical RAM."""
    marker = "unobserved"
    proc = None
    try:
        with (root / "stderr").open("xb") as err:
            proc = subprocess.Popen(
                cmd,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=err,
                bufsize=0,
            )
            timer = threading.Timer(20, proc.kill)
            timer.start()
            try:
                if "QMP" not in json.loads(proc.stdout.readline()):
                    raise ValueError("missing QMP greeting")

                class Pipe:
                    write = proc.stdin.write
                    readline = proc.stdout.readline

                pipe = Pipe()
                request(pipe, "qmp_capabilities")
                if observer is not None:
                    observer.start(pipe, proc.pid)
                while proc.poll() is None:
                    if observer is not None:
                        observer.sample()
                    uart = root / "uart.bin"
                    if uart.exists():
                        if uart.stat().st_size > 1 << 20:
                            raise ValueError("oversize UART")
                        terminal = re.search(
                            terminal_pattern,
                            uart.read_bytes(),
                        )
                        if terminal:
                            request(pipe, "stop")
                            request(
                                pipe,
                                "pmemsave",
                                val=base,
                                size=limit - base,
                                filename=str(root / "ram.bin"),
                            )
                            marker = terminal[1].decode("ascii")
                            break
                    time.sleep(0.005)
            finally:
                timer.cancel()
                timer.join()
    except (ValueError, RuntimeError, OSError) as exc:
        marker = str(exc)
    finally:
        if proc is not None:
            if proc.poll() is None:
                proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        if observer is not None:
            observer.finish()
        save(
            root / "session.json",
            dict(
                marker=marker,
                pid=proc.pid if proc else None,
                hashes={p.name: sha(p) for p in sorted(root.iterdir()) if p.is_file()},
            ),
        )
        print(sha(root / "session.json"))
    return int(marker != success)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("root", "kernel", "user", "probe"):
        parser.add_argument(name, type=Path)
    parser.add_argument("revision")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--nm", default="aarch64-linux-gnu-nm")
    sys.exit(capture(parser.parse_args()))
