"""Freeze and capture the two ordinary-fifo positions without replacing an attempt."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

from ordinary_amc_capture import FLAGS, save, sha
from qemu_boot import stop


def freeze(args):
    if not re.fullmatch("[0-9a-f]{40}", args.revision):
        raise ValueError("expected full source revision")
    root = args.root.resolve()
    root.mkdir()
    kernel = root / "kernel.elf"
    shutil.copyfile(args.elf, kernel)
    qemu = Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True)
    for filename, cmd in [
        ("symbols", [args.nm, "-nS", str(kernel)]),
        ("disassembly", [args.objdump, "-d", str(kernel)]),
        ("qemu.version", [str(qemu), "--version"]),
    ]:
        with (root / filename).open("xb") as out:
            subprocess.run(cmd, stdout=out, check=True)
    argv = {}
    for mode in ("plain", "traced"):
        tracing = ["-d", "int", "-D", str(root / mode / "exceptions.log")] if mode == "traced" else []
        argv[mode] = [str(qemu), *FLAGS, "-serial", "stdio", *tracing, "-kernel", str(kernel)]
    save(root / "freeze.json", dict(revision=args.revision, argv=argv, qemu_sha256=sha(qemu),
         selector=["virt", "mutex", "ordinary-fifo"], timeout_ns=15_000_000_000,
         quiet_ns=100_000_000, elf_sha256=sha(kernel)))
    print(sha(root / "freeze.json"))


def run(args):
    root = args.root.resolve()
    manifest = root / "freeze.json"
    if sha(manifest) != args.freeze_sha256:
        raise ValueError("freeze digest mismatch")
    spec = json.loads(manifest.read_text())
    argv = spec["argv"][args.mode]
    if sha(Path(argv[-1])) != spec["elf_sha256"] or sha(Path(argv[0])) != spec["qemu_sha256"]:
        raise ValueError("frozen executable changed")
    if args.mode == "traced" and not (root / "plain" / "outcome.json").is_file():
        raise ValueError("first plain outcome required")
    out = root / args.mode
    out.mkdir()
    start = time.monotonic_ns()
    last = start
    size = 0
    reason = "timeout"
    shutdown = status = None
    uart = out / "uart"
    try:
        with uart.open("xb") as serial, (out / "stderr").open("xb") as err:
            proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=serial, stderr=err)
            try:
                while True:
                    now = time.monotonic_ns()
                    if now - start >= spec["timeout_ns"]:
                        break
                    if proc.poll() is not None:
                        reason = "early exit"
                        break
                    observed = uart.stat().st_size
                    if observed != size:
                        size, last = observed, now
                        if size > 1024 * 1024:
                            reason = "UART overflow"
                            break
                    elif size and now - last >= spec["quiet_ns"]:
                        if uart.read_bytes().splitlines(keepends=True)[-1] in (b"FIFO END\n", b"FIFO END\r\n"):
                            reason = "terminal quiet"
                            break
                    time.sleep(0.002)
            finally:
                observed_end = time.monotonic_ns()
                shutdown = stop(proc)
                status = proc.returncode
    except OSError as error:
        reason = str(error)
    if reason == "terminal quiet" and uart.stat().st_size != size:
        reason = "UART changed during stop"
    end = locals().get("observed_end", time.monotonic_ns())
    save(out / "outcome.json", dict(reason=reason, process_returncode=status, shutdown=shutdown,
         elapsed_ns=end-start, quiet_ns=end-last, freeze_sha256=args.freeze_sha256))
    return int(reason != "terminal quiet" or shutdown is not None)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    first = commands.add_parser("freeze")
    first.add_argument("root", type=Path)
    first.add_argument("elf", type=Path)
    first.add_argument("revision")
    first.add_argument("--qemu", default="qemu-system-aarch64")
    first.add_argument("--nm", default="aarch64-linux-gnu-nm")
    first.add_argument("--objdump", default="aarch64-linux-gnu-objdump")
    boot = commands.add_parser("run")
    boot.add_argument("root", type=Path)
    boot.add_argument("freeze_sha256")
    boot.add_argument("mode", choices=["plain", "traced"])
    options = parser.parse_args()
    sys.exit(freeze(options) if options.action == "freeze" else run(options))
