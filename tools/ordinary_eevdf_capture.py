#!/usr/bin/env python3
"""Freeze and capture one cold telemetry boot in a new directory, retaining failed attempts."""
import argparse
import json
import re
from pathlib import Path
import shutil
import subprocess
import sys
import time
import threading

from ordinary_amc_capture import FLAGS, sha, save
from qemu_capture import request

MARKERS = (b"EEVDF APP COMPLETE", b"EEVDF APP FAIL")
EXPECTED_SIZE = (12 + 512 * 33 + 8) * 8


def symbol(table, name):
    hits = [row.split() for row in table.splitlines() if row.split()[-1:] == [name]]
    if len(hits) != 1 or len(hits[0]) != 4:
        raise ValueError("missing or ambiguous ELF symbol " + name)
    return tuple(int(word, 16) for word in hits[0][:2])


def capture(args):
    if not re.fullmatch("[0-9a-f]{40}", args.revision):
        raise ValueError("source revision must be a full lowercase Git SHA")
    root = args.root.resolve()
    root.mkdir()
    elf = root / "kernel.elf"
    shutil.copyfile(args.elf, elf)
    qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
    symbols = subprocess.check_output([args.nm, "-nS", str(elf)])
    (root / "symbols").write_bytes(symbols)
    with (root / "disassembly").open("xb") as out:
        subprocess.run([args.objdump, "-d", str(elf)], stdout=out, check=True)
    (root / "qemu-version").write_bytes(subprocess.check_output([qemu, "--version"]))
    addr, size = symbol(symbols.decode(), "jnox_eevdf_app_journal")
    if size != EXPECTED_SIZE:
        raise ValueError("unexpected linked journal extent")
    cmd = [
        qemu,
        *FLAGS,
        "-qmp",
        "stdio",
        "-d",
        "int",
        "-D",
        str(root / "exceptions.log"),
        "-serial",
        "file:" + str(root / "uart.bin"),
        "-kernel",
        str(elf),
    ]
    record = dict(
        version=1,
        revision=args.revision,
        argv=cmd,
        qemu_sha256=sha(Path(qemu)),
        elf_sha256=sha(elf),
        timeout_seconds=20,
        started_ns=time.time_ns(),
    )
    save(root / "launch.json", record)
    reason = "timeout"
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
            # Pipe reads run under the same absolute deadline, including QMP handshake and dump.
            expired = threading.Event()

            def expire():
                expired.set()
                proc.kill()

            watchdog = threading.Timer(20, expire)
            watchdog.start()
            try:
                if "QMP" not in json.loads(proc.stdout.readline()):
                    raise ValueError("invalid QMP greeting")

                class Stream:
                    write = proc.stdin.write
                    readline = proc.stdout.readline

                stream = Stream()
                request(stream, "qmp_capabilities")
                while proc.poll() is None:
                    path = root / "uart.bin"
                    if path.exists():
                        if path.stat().st_size > 2**20:
                            raise ValueError("oversize UART")
                        lines = path.read_bytes().splitlines(keepends=True)
                        marker = next(
                            (
                                line.rstrip()
                                for line in lines
                                if line.endswith(b"\n") and line.rstrip() in MARKERS
                            ),
                            None,
                        )
                        if marker is not None:
                            request(stream, "stop")
                            request(
                                stream,
                                "pmemsave",
                                val=addr,
                                size=size,
                                filename=str(root / "journal.bin"),
                            )
                            reason = marker.decode()
                            break
                    time.sleep(0.005)
                if proc.poll() is not None and not expired.is_set():
                    reason = "premature exit"
            finally:
                watchdog.cancel()
                watchdog.join()
    except (OSError, ValueError, RuntimeError) as exc:
        reason = str(exc)
    finally:
        if proc is not None:
            if proc.poll() is None:
                proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        session = dict(
            reason=reason,
            pid=proc.pid if proc else None,
            ended_ns=time.time_ns(),
            hashes={p.name: sha(p) for p in sorted(root.iterdir()) if p.is_file()},
        )
        save(root / "session.json", session)
        print(sha(root / "session.json"))
    return int(reason != MARKERS[0].decode())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("elf", type=Path)
    parser.add_argument("revision")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--nm", default="aarch64-linux-gnu-nm")
    parser.add_argument("--objdump", default="aarch64-linux-gnu-objdump")
    sys.exit(capture(parser.parse_args()))
