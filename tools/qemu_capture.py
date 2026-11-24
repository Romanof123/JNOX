#!/usr/bin/env python3
"""Capture a guest journal, UART, QMP status and exceptions during one Linux QEMU run."""
import json
import os
import re
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time


def journal_location(kernel, symbol, *, optional=False):
    table = subprocess.check_output(["aarch64-linux-gnu-nm", "-S", kernel], text=True)
    for line in table.splitlines():
        fields = line.split()
        if len(fields) == 4 and fields[3] == symbol:
            return int(fields[0], 16), int(fields[1], 16)
    if optional:
        return None
    raise ValueError(f"missing journal symbol {symbol}")


def request(stream, command, **arguments):
    stream.write(json.dumps({"execute": command, "arguments": arguments}).encode() + b"\n")
    while True:
        answer = json.loads(stream.readline())
        if "event" in answer:
            continue
        if "return" not in answer:
            raise RuntimeError(answer)
        return answer["return"]


def snapshot(sock_path, out, address, size, idle_range=None, trace_range=None, extra=(), *, freeze=False):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.8)
        sock.connect(str(sock_path))
        with sock.makefile("rwb", buffering=0) as stream:
            if "QMP" not in json.loads(stream.readline()):
                raise ValueError("invalid QMP greeting")
            request(stream, "qmp_capabilities")
            status = request(stream, "query-status")
            cpus = request(stream, "query-cpus-fast")
            (out / "qmp.json").write_text(json.dumps({"status": status, "cpus": cpus}))
            if not status["running"]:
                raise ValueError("guest stopped before host termination")
            if idle_range is not None or freeze:
                request(stream, "stop")
                registers = request(stream, "human-monitor-command", **{"command-line": "info registers"})
                (out / "registers.txt").write_text(registers)
            if idle_range is not None:
                match = re.search(r"\bPC=([0-9a-fA-F]+)\b", registers)
                start, length = idle_range
                if not match or not start <= int(match[1], 16) < start + length:
                    raise ValueError("terminal PC outside fixed_runtime_idle")
            request(stream, "pmemsave", val=address, size=size, filename=str(out / "audit.bin"))
            for name, (base, length) in extra:
                request(stream, "pmemsave", val=base, size=length, filename=str(out / (name + ".bin")))
            if trace_range is not None:
                trace_address, trace_size = trace_range
                request(stream, "pmemsave", val=trace_address, size=trace_size,
                        filename=str(out / "trace.bin"))


def main(symbol, directory_variable, idle_symbol=None, trace_symbol=None, *, require_trace=False, extra_symbols=(), freeze=False):
    out = Path(os.environ[directory_variable]).resolve()
    out.mkdir(parents=True, exist_ok=False)
    args = sys.argv[1:]
    kernel = args[args.index("-kernel") + 1]
    address, size = journal_location(kernel, symbol)
    idle_range = journal_location(kernel, idle_symbol) if idle_symbol else None
    trace_range = journal_location(kernel, trace_symbol, optional=not require_trace) if trace_symbol else None
    extra = [(name, journal_location(kernel, name)) for name in extra_symbols]
    if extra and idle_range is None:
        raise ValueError("extra memory snapshots require a stopped idle guest")
    stop_requested = False

    def stop(signum, frame):
        nonlocal stop_requested
        stop_requested = True

    signal.signal(signal.SIGTERM, stop)
    with tempfile.TemporaryDirectory(dir="/tmp") as td:
        sock_path = Path(td) / "qmp"
        command = [shutil.which("qemu-system-aarch64") or "qemu-system-aarch64",
                   "-qmp", f"unix:{sock_path},server=on,wait=off",
                   "-d", "int", "-D", str(out / "int.trace"), *args]
        with subprocess.Popen(command, stdout=subprocess.PIPE) as guest:
            launch = {"argv": command, "pid": guest.pid, "audit_address": address, "audit_size": size}
            if extra:
                launch["extra_symbols"] = dict(extra)
            if trace_range is not None:
                launch.update(trace_symbol=trace_symbol, trace_address=trace_range[0], trace_size=trace_range[1])
            (out / "launch.json").write_text(json.dumps(launch))
            # Drain the guest UART concurrently so the normal observer still
            # sees the live byte stream. Retain all bytes, including late ones.
            import threading

            def forward():
                with (out / "uart.bin").open("wb") as uart:
                    while block := os.read(guest.stdout.fileno(), 4096):
                        uart.write(block)
                        sys.stdout.buffer.write(block)
                        sys.stdout.buffer.flush()

            pump = threading.Thread(target=forward)
            pump.start()
            try:
                while not stop_requested:
                    if guest.poll() is not None:
                        raise RuntimeError(f"guest exited early: {guest.returncode}")
                    time.sleep(0.004)
                snapshot(sock_path, out, address, size, idle_range, trace_range, extra, freeze=freeze)
            finally:
                if guest.poll() is None:
                    guest.terminate()
                try:
                    guest.wait(timeout=0.7)
                except subprocess.TimeoutExpired:
                    guest.kill()
                    guest.wait()
                    raise
                finally:
                    pump.join()
                    (out / "exit.json").write_text(json.dumps({"qemu_status": guest.returncode}))
            return 0 if guest.returncode in (0, -signal.SIGTERM) else 1
