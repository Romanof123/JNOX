#!/usr/bin/env python3
"""QEMU wrapper for one SMP launch on Linux. Set SMP_CAPTURE_DIR to a new path."""
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time


def journal_location(kernel):
    table = subprocess.check_output(["aarch64-linux-gnu-nm", "-S", kernel], text=True)
    for line in table.splitlines():
        fields = line.split()
        if len(fields) == 4 and fields[3] == "smp_audit":
            return int(fields[0], 16), int(fields[1], 16)
    raise ValueError("missing SMP audit symbol")


def request(stream, command, **arguments):
    stream.write(json.dumps({"execute": command, "arguments": arguments}).encode() + b"\n")
    while True:
        answer = json.loads(stream.readline())
        if "event" in answer:
            continue
        if "return" not in answer:
            raise RuntimeError(answer)
        return answer["return"]


def snapshot(sock_path, out, address, size):
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
            request(stream, "pmemsave", val=address, size=size, filename=str(out / "audit.bin"))


def main():
    out = Path(os.environ["SMP_CAPTURE_DIR"]).resolve()
    out.mkdir(parents=True, exist_ok=False)
    args = sys.argv[1:]
    address, size = journal_location(args[args.index("-kernel") + 1])
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
            (out / "launch.json").write_text(json.dumps({"argv": command, "pid": guest.pid,
                "audit_address": address, "audit_size": size}))
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
                snapshot(sock_path, out, address, size)
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


if __name__ == "__main__":
    sys.exit(main())
