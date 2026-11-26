#!/usr/bin/env python3
"""Boot a packaged JNOX shell, retain raw UART, and check its diagnostic snapshots."""
import argparse
import os
from pathlib import Path
import queue
import re
import subprocess
import threading
import time

from ordinary_amc_capture import FLAGS, save, sha


HEADER = re.compile(rb"^dmesg records=(\d+) dropped=(\d+)\n", re.M)
ROW = re.compile(rb"\[(\d+)\] ticks=(\d+) ([^\n]+)\n")
PROMPT = b"jnox$ "


def verify(raw, case):
    if b"JNOX M1 BOOT\nJNOX M1 READY\n" not in raw or b"NATIVE EL0 FAULT" in raw:
        raise ValueError("missing clean kernel boot")
    snapshots = []
    for match in HEADER.finditer(raw):
        count, dropped = map(int, match.groups())
        if not 0 < count <= 4:
            raise ValueError("invalid snapshot record count")
        pos = match.end()
        events = []
        ticks = []
        for seq in range(dropped + 1, dropped + count + 1):
            row = ROW.match(raw, pos)
            if row is None or int(row[1]) != seq:
                raise ValueError("missing or unordered diagnostic record")
            ticks.append(int(row[2]))
            events.append(row[3])
            pos = row.end()
        if ticks != sorted(ticks) or ROW.match(raw, pos):
            raise ValueError("inconsistent record ordering or extent")
        snapshots.append((dropped, events, raw[match.start():pos]))
    boot_events = [b"native boot", b"shell ready"]
    if not snapshots or snapshots[0][:2] != (0, boot_events):
        raise ValueError("missing initial boot and ready snapshot")
    exits = dict(re.findall(rb"^NATIVE EL0 EXIT PID (\d+) (-?\d+)\n", raw, re.M))
    if case == "interactive":
        if len(snapshots) != 2 or snapshots[0] != snapshots[1] or exits != {b"0": b"0"}:
            raise ValueError("live reads changed the snapshot or shell failed")
        if raw.count(PROMPT + b"dmesg\n") != 2 or PROMPT + b"exit\n" not in raw:
            raise ValueError("missing echoed operator commands")
    else:
        denied = case == "denied"
        status = b"1" if denied else b"0"
        if exits != {b"0": b"0", b"1": status, b"2": b"0"}:
            raise ValueError(f"unexpected exits {exits!r}")
        if len(snapshots) not in (2, 3) or snapshots[1][0] != 2:
            raise ValueError("missing bounded snapshot after the pipeline")
        expected = {b"child spawn pid=1", b"child spawn pid=2",
                    b"child exit pid=1 status=" + status, b"child exit pid=2 status=0"}
        events = snapshots[1][1]
        if set(events) != expected or any(
            events.index(b"child spawn pid=" + pid) > events.index(b"child exit pid=" + pid + b" status=" + rc)
            for pid, rc in ((b"1", status), (b"2", b"0"))
        ):
            raise ValueError("snapshot disagrees with child lifecycle")
        if len(snapshots) == 3 and snapshots[1] != snapshots[2]:
            raise ValueError("re-reading consumed or modified the runtime snapshot")
        output = b"0" if denied else b"2"
        if b"\n" + output + b"\n" not in raw:
            raise ValueError("missing pipeline result")
        if (b"dmesg: permission denied\n" in raw) != denied:
            raise ValueError("unexpected dmesg access result")
        if denied:
            # Independent terminal UART record identifies the writing PID and fd.
            message = b"dmesg: permission denied\n"
            encoded = b" ".join(str(byte).encode() for byte in message)
            expected_line = rb"^NATIVE SHELL CONSOLE \d+ 1 2 " + str(len(message)).encode() + b" " + encoded + b"$"
            if not re.search(expected_line, raw, re.M):
                raise ValueError("denial did not come from slot 1 stderr")


def run(args):
    out = args.out
    out.mkdir(parents=True, exist_ok=False)
    cmd = [args.qemu, *FLAGS, "-serial", "stdio", "-kernel", str(args.kernel.resolve())]
    save(out / "command.json", dict(argv=cmd, kernel_sha256=sha(args.kernel), case=args.case))
    chunks = queue.Queue()
    raw = bytearray()
    cursor = 0
    deadline = time.monotonic() + args.timeout

    def until(token):
        nonlocal cursor
        while True:
            start = raw.find(token, cursor)
            if start >= 0:
                cursor = start + len(token)
                return
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"waiting for {token!r}")
            try:
                chunk = chunks.get(timeout=remaining)
            except queue.Empty:
                raise TimeoutError(f"waiting for {token!r}") from None
            if not chunk:
                raise RuntimeError(f"UART closed before {token!r}")
            raw.extend(chunk)

    with (out / "uart.bin").open("wb") as uart, (out / "qemu.stderr").open("wb") as stderr:
        proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr)
        def read_uart():
            while True:
                chunk = os.read(proc.stdout.fileno(), 4096)
                uart.write(chunk)
                chunks.put(chunk)
                if not chunk:
                    return

        reader = threading.Thread(target=read_uart)
        reader.start()
        try:
            if args.case == "interactive":
                with (out / "input.bin").open("wb") as sent:
                    for command in (b"dmesg\n", b"dmesg\n", b"exit\n"):
                        until(PROMPT)
                        for byte in command:
                            key = bytes([byte])
                            proc.stdin.write(key)
                            proc.stdin.flush()
                            sent.write(key)
                            until(key)
            until(b"NATIVE EL0 EXIT PID " + (b"0" if args.case == "interactive" else b"2") + b" 0\n")
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            reader.join()
            proc.stdin.close()
            proc.stdout.close()
    verify((out / "uart.bin").read_bytes(), args.case)
    print(f"{args.case}: verified JNOX dmesg, UART {sha(out / 'uart.bin')}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("case", choices=["scripted", "denied", "interactive"])
    parser.add_argument("kernel", type=Path)
    parser.add_argument("out", type=Path, help="fresh evidence directory")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timeout", type=float, default=20)
    run(parser.parse_args())
