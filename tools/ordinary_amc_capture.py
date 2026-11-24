#!/usr/bin/env python3
"""Freeze an exact ordinary AMC ELF and capture its first cold default-clock virt boot.

freeze CASE NEW_DIRECTORY ELF QEMU SOURCE_REVISION
run DIRECTORY FREEZE_SHA256
A launch consumes its first directory even when QEMU fails. UART and exception
logs belong to the same process. No physical timing or hardware claim follows.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time


CASES = ("exact-lo", "exact-hi", "over-hi")
FLAGS = ["-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
         "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none",
         "-monitor", "none", "-no-reboot"]


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as src:
        for block in iter(lambda: src.read(1024*1024), b""):
            digest.update(block)
    return digest.hexdigest()


def save(path, record):
    with path.open("x", encoding="utf-8") as f:
        json.dump(record, f, indent=2)
        f.write("\n")


def freeze(args):
    root = args.root.resolve()
    root.mkdir()
    shutil.copyfile(args.elf, root/"kernel.elf")
    qemu = args.qemu.resolve(strict=True)
    save(root/"freeze.json", dict(version=1, case=args.case, source=args.revision,
         kernel_sha256=sha(root/"kernel.elf"), qemu=str(qemu), qemu_sha256=sha(qemu),
         flags=FLAGS, allowance_seconds=15, observer="same-process -d int"))
    print(sha(root/"freeze.json"))
    return 0


def capture(args):
    root = args.root.resolve()
    if sha(root/"freeze.json") != args.digest:
        raise ValueError("freeze hash mismatch")
    record = json.loads((root/"freeze.json").read_bytes())
    dest = root/"first"
    dest.mkdir()
    log, uart = dest/"exceptions.log", dest/"uart.bin"
    cmd = [record["qemu"], *FLAGS, "-serial", "file:"+str(uart), "-d", "int",
           "-D", str(log), "-kernel", str(root/"kernel.elf")]
    session = dict(freeze_sha256=args.digest, case=record["case"], argv=cmd,
                   started_unix_ns=time.time_ns())
    save(dest/"attempt.json", session)
    start=time.monotonic_ns()
    proc=None
    reason="deadline"
    try:
        if (record["case"] not in CASES or record["flags"]!=FLAGS or record["allowance_seconds"]!=15
                or sha(root/"kernel.elf")!=record["kernel_sha256"]
                or sha(Path(cmd[0]))!=record["qemu_sha256"]):
            raise ValueError("frozen command or input differs")
        with (dest/"stderr").open("xb") as err,(dest/"stdout").open("xb") as out:
            start=time.monotonic_ns()
            proc=subprocess.Popen(cmd,stdin=subprocess.DEVNULL,stderr=err,stdout=out)
            while time.monotonic_ns()-start<15_000_000_000:
                if uart.exists():
                    if uart.stat().st_size>2*1024*1024:
                        raise ValueError("bounded UART exceeded")
                    with uart.open("rb") as f:
                        f.seek(max(0,uart.stat().st_size-512))
                        tail=f.read().splitlines(keepends=True)
                    if any(line.rstrip(b"\r\n")==b"AMR END" and line.endswith(b"\n") for line in tail):
                        reason="guest_footer";break
                    if any(line.startswith(b"AMR FAIL ") and line.endswith(b"\n") for line in tail):
                        reason="guest_failure";break
                if proc.poll() is not None:
                    reason="process_exit";break
                time.sleep(.005)
    except (ValueError,OSError) as exc:
        reason=str(exc)
    finally:
        end=time.monotonic_ns()
        if proc is not None and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=1)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        session.update(reason=reason, elapsed_ns=end-start,ended_unix_ns=time.time_ns(),
                       returncode=None if proc is None else proc.returncode,
                       hashes={f.name:sha(f) for f in sorted(dest.iterdir()) if f.is_file()})
        save(dest/"session.json",session)
        print(sha(dest/"session.json"))
    return int(reason!="guest_footer")


def main():
    parser=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter)
    commands=parser.add_subparsers(dest="command",required=True)
    f=commands.add_parser("freeze")
    f.add_argument("case",choices=CASES)
    for name in ("root","elf","qemu"):
        f.add_argument(name,type=Path)
    f.add_argument("revision")
    a=commands.add_parser("run")
    a.add_argument("root",type=Path)
    a.add_argument("digest")
    args=parser.parse_args()
    try:
        return freeze(args) if args.command=="freeze" else capture(args)
    except (ValueError,OSError,KeyError) as exc:
        parser.exit(1,f"{exc}\n")


if __name__=="__main__":
    sys.exit(main())
