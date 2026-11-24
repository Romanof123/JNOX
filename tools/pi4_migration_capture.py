#!/usr/bin/env python3
"""Freeze one CHG-075 model case, then retain each of two cold launches on Linux.

freeze CASE NEW_DIR ELF RAW DISASSEMBLY QEMU --source COMMIT
       [--wire-version {1,2,3}] [--workload {sum-65536,sample-crc32-v1}]
       [--compiler BUILD_RUSTC_VERSION]
MG3 freezes both positions for one stock-model image. Select the stream
workload explicitly and supply the compiler version from the build when the
linked ELF omits rustc's version comment.
run FROZEN_DIR TRUSTED_FREEZE_SHA256 ORDINAL
Physical UART acquisition uses pi4_serial_capture.py with board/card/firmware
and witnessed power-cycle records. Model thread logs are never physical trace.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import subprocess
import sys
import time

CASES = ("migration-global", "migration-partitioned", "migration-clustered")
FLAGS = ["-machine", "raspi4b", "-accel", "tcg,thread=multi", "-smp", "4",
         "-m", "2G", "-display", "none", "-monitor", "none", "-serial", "stdio"]
FILES = ("kernel.elf", "kernel8.img", "disassembly.txt")


BASELINE = "sum-65536"
STREAM = "sample-crc32-v1"
STOCK_SHA = "3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862"


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as src:
        for block in iter(lambda: src.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def save(path, value):
    with path.open("x", encoding="utf-8") as f:
        json.dump(value, f, indent=2)
        f.write("\n")


def freeze(args):
    root = args.directory.resolve()
    root.mkdir()
    for name, src in zip(FILES, (args.elf, args.raw, args.disassembly)):
        shutil.copyfile(src, root / name)
    qemu = args.qemu.resolve(strict=True)
    from pi4_mg3_symbols import declared_wire, mg3_symbols
    elf, raw, disassembly = [(root / name).read_bytes() for name in FILES]
    wire = declared_wire(raw)
    selected = getattr(args, "wire_version", None)
    if selected is not None and selected != wire:
        raise ValueError("selected wire version disagrees with linked image")
    workload = getattr(args, "workload", BASELINE)
    symbols = mg3_symbols(elf, raw, args.case, workload)
    record = dict(schema=1, wire_version=wire, platform="raspi4b-tcg", case=args.case, source=args.source,
                  qemu=str(qemu), qemu_sha256=sha(qemu), flags=FLAGS,
                  hashes={name: sha(root / name) for name in FILES},
                  ceiling_ns=30_000_000_000, quiet_ns=1_000_000_000, positions=[1, 2])
    if wire == 3 or workload == STREAM:
        if workload == STREAM and wire < 2:
            raise ValueError("sample stream requires MG2 or later")
        record["workload"] = workload
        if record["qemu_sha256"] != STOCK_SHA:
            raise ValueError("this model profile requires the frozen stock QEMU")
        from pi4_migration_replay import layout
        bodies, _ = layout(elf, raw, disassembly, workload)
        if len(bodies) != 1:
            raise ValueError("expected one linked input-processing loop")
    if wire == 3:
        from elftools.elf.elffile import ELFFile
        comment = ELFFile(io.BytesIO(elf)).get_section_by_name(".comment")
        comments = comment.data().decode().strip("\0").split("\0") if comment else []
        compiler = getattr(args, "compiler", None) or next(
            (item for item in comments if "rustc" in item.lower()), None)
        if not compiler:
            raise ValueError("provide the build's rustc version with --compiler")
        names = ("pi4_migration_capture.py", "pi4_migration_replay.py", "pi4_mg3_symbols.py",
                 "pi4_mg3_replay.py", "pi4_smp_replay.py")
        record.update(local_symbols=symbols, loop_range=list(bodies[0]), compiler=compiler,
                      elf_comments=comments,
                      tool_sha256={name: sha(Path(__file__).with_name(name)) for name in names})
    save(root / "freeze.json", record)
    print(sha(root / "freeze.json"))


def acquire(args, terminal_observer=None):
    root = args.directory.resolve()
    if sha(root / "freeze.json") != args.digest:
        raise ValueError("freeze digest mismatch")
    record = json.loads((root / "freeze.json").read_text())
    if record["platform"] != "raspi4b-tcg" or record["flags"] != FLAGS:
        raise ValueError("observer configuration mismatch")
    if args.ordinal == 2 and not (root / "first-1/session.json").is_file():
        raise ValueError("missing retained first position")
    dest = root / f"first-{args.ordinal}"
    dest.mkdir()
    log_items = getattr(terminal_observer, "log_items", "int,tid")
    cmd = [record["qemu"], *FLAGS, "-kernel", str(root / "kernel8.img"), "-d", log_items,
           "-D", str(dest / "observer-%d.log")]
    if terminal_observer is not None:
        cmd[1+len(FLAGS):1+len(FLAGS)] = terminal_observer.argv()
    session = dict(freeze_sha256=args.digest, case=record["case"], ordinal=args.ordinal,
                   platform="raspi4b-tcg", argv=cmd, wall_start_ns=time.time_ns())
    if terminal_observer is not None:
        session["diagnostic"] = getattr(terminal_observer, "profile", "mg2-post-terminal-ram-v1")
    save(dest / "attempt.json", session)
    proc = None
    tids = {}
    start = last = time.monotonic_ns()
    terminal = None
    reason = "deadline"
    observed_end = None
    try:
        if not Path("/proc/self/task").is_dir():
            raise ValueError("native Linux procfs is required for thread attribution")
        if sha(Path(cmd[0])) != record["qemu_sha256"] or any(
                sha(root / n) != record["hashes"][n] for n in FILES):
            raise ValueError("frozen binary or evidence input changed")
        with (dest / "uart.bin").open("xb") as uart, (dest / "stderr").open("xb") as err:
            start = last = time.monotonic_ns()
            proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=err)
            with selectors.DefaultSelector() as poll:
                poll.register(proc.stdout, selectors.EVENT_READ)
                partial = b""
                while time.monotonic_ns() - start < 30_000_000_000:
                    for comm in Path(f"/proc/{proc.pid}/task").glob("*/comm"):
                        try:
                            name = comm.read_text().strip()
                        except FileNotFoundError:
                            continue
                        if re.fullmatch(r"CPU [0-3]/TCG", name):
                            tid = comm.parent.name
                            if tids.setdefault(tid, name) != name:
                                raise ValueError("vCPU thread identity changed")
                    if terminal and time.monotonic_ns() - last >= 1_000_000_000:
                        reason = "terminal_quiet"
                        break
                    if not poll.select(.025):
                        continue
                    block = os.read(proc.stdout.fileno(), 65536)
                    if not block:
                        reason = "process_eof"
                        break
                    last = time.monotonic_ns()
                    uart.write(block)
                    uart.flush()
                    lines = (partial + block).split(b"\n")
                    partial = lines.pop()
                    if len(partial) > 8192 or uart.tell() > 8*1024*1024:
                        raise ValueError("UART evidence exceeded capacity")
                    for line in lines:
                        if line.startswith((b"MG END ", b"MG FAIL ", b"PI4 MIG FAIL", b"PI4 SMP BSP FAIL")):
                            terminal = line.decode("ascii", errors="replace")
        if terminal_observer is not None:
            observed_end = time.monotonic_ns()
            session["observation_monotonic_ns"] = dict(start=start,last_uart=last,end=observed_end)
            session["post_terminal"] = {"status": "unobserved", "reason": reason}
            if reason == "terminal_quiet":
                try:
                    terminal_observer.capture(dest, record, tids)
                    session["post_terminal"] = {"status": "captured"}
                except (OSError, ValueError, RuntimeError, EOFError, KeyError, TypeError) as error:
                    session["post_terminal"] = {"status": "failed", "reason": str(error)}
    except (OSError, ValueError) as error:
        reason = str(error)
    finally:
        end = observed_end if observed_end is not None else time.monotonic_ns()
        if proc is not None:
            if proc.poll() is None:
                proc.terminate()
            try:
                proc.wait(timeout=1)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            proc.stdout.close()
        save(dest / "threads.json", tids)
        session.update(reason=reason, terminal=terminal, wall_end_ns=time.time_ns(),
                       elapsed_ns=end-start, quiet_ns=end-last,
                       returncode=proc.returncode if proc else None,
                       hashes={f.name: sha(f) for f in sorted(dest.iterdir()) if f.is_file()})
        save(dest / "session.json", session)
        print(sha(dest / "session.json"))
    return int(reason != "terminal_quiet" or
               (terminal_observer is not None and session.get("post_terminal", {}).get("status") != "captured"))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="mode", required=True)
    f = sub.add_parser("freeze")
    f.add_argument("case", choices=CASES)
    for name in ("directory", "elf", "raw", "disassembly", "qemu"):
        f.add_argument(name, type=Path)
    f.add_argument("--source", required=True)
    f.add_argument("--wire-version", type=int, choices=(1, 2, 3),
                   help="require this version in the linked image")
    f.add_argument("--compiler", help="build compiler version if absent from ELF comments")
    f.add_argument("--workload", choices=(BASELINE, STREAM), default=BASELINE)
    a = sub.add_parser("run")
    a.add_argument("directory", type=Path)
    a.add_argument("digest")
    a.add_argument("ordinal", type=int, choices=(1, 2))
    args = parser.parse_args()
    try:
        return freeze(args) if args.mode == "freeze" else acquire(args)
    except (OSError, KeyError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
