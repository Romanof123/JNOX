#!/usr/bin/env python3
"""Observe a JNOX caller, a Linux NIC and its external device peer on one Bao board."""
import argparse
import json
from pathlib import Path
import re
import secrets
import shutil
import socket
import struct
import subprocess
import threading
import time

from board_gateway_receipts import native_layout, replay, sample
from verify_linux_driver_gateway import exact, remaining
from verify_linux_virtio_net_baseline import MAC, require, save, sha
from verify_single_board_boot import QMP, inspect


def wait_line(proc, log, pattern, deadline):
    expression = re.compile(pattern)
    while True:
        remaining(deadline)
        for line in log.read_text(errors="replace").splitlines():
            match = expression.fullmatch(line)
            if match:
                return match
        require(proc.poll() is None, "QEMU exited while awaiting " + pattern)
        time.sleep(0.01)


def device_reply(proc, log, port, generation, take_samples, deadline):
    prefix = struct.pack("!2sBBIII", b"JG", 1, 1, generation, 1, 10_000).hex()
    match = wait_line(proc, log, "BG REQ (" + prefix + "[0-9a-f]{16})", deadline)
    begin = time.monotonic_ns()
    with socket.create_connection(("127.0.0.1", port), remaining(deadline)) as conn:
        rec = exact(conn, 24, deadline)
        require(rec.hex() == match[1], "device bytes differ from the Linux request")
        conn.settimeout(remaining(deadline))
        require(conn.recv(1) == b"", "device request exceeds its fixed size")
        if take_samples:
            take_samples()
        response = secrets.token_bytes(8)
        conn.sendall(response)
        conn.shutdown(socket.SHUT_WR)
    return {"request": rec.hex(), "response": response.hex(),
            "begin_ns": begin, "end_ns": time.monotonic_ns()}


def observe(args):
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    qemu = shutil.which(args.qemu)
    require(qemu, "QEMU executable unavailable")
    layout = native_layout(args.native)
    uart = out / "uart.log"
    argv = [qemu, "-machine", "virt,virtualization=on,secure=off,gic-version=2",
            "-cpu", "cortex-a72", "-accel", "tcg", "-smp", "2", "-m", "512M",
            "-nodefaults", "-no-reboot", "-display", "none", "-monitor", "none",
            "-qmp", "stdio", "-serial", f"file:{uart}", "-kernel", str(args.bao.resolve()),
            "-global", "virtio-mmio.force-legacy=false", "-netdev",
            "user,id=lan,restrict=on,ipv6=off,hostfwd=tcp:127.0.0.1:0-10.0.2.15:8080",
            "-device", f"virtio-net-device,netdev=lan,mac={MAC}",
            "-object", f"filter-dump,id=wire,netdev=lan,file={out/'wire.pcap'}"]
    launch = {"argv": argv, "qemu_sha256": sha(Path(qemu).read_bytes()),
              "qemu_version": subprocess.check_output([qemu, "--version"], text=True).splitlines()[0],
              "bao_sha256": sha(args.bao.read_bytes())}
    deadline = time.monotonic() + args.timeout
    with (out/"qemu.stderr").open("wb") as err, subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as proc:
        save(out/"launch.json", dict(launch, pid=proc.pid))
        qmp = QMP(proc)
        connected = False
        # Bound a stalled QMP transaction separately from its socket or UART deadline.
        watchdog = threading.Timer(args.timeout + 8, proc.kill)
        watchdog.start()
        try:
            require("QMP" in json.loads(qmp.readline()), "missing QMP greeting")
            qmp.call("qmp_capabilities")
            connected = True
            cpus = inspect(qmp, out, "start")
            require(sorted(c["cpu-index"] for c in cpus) == [0, 1], "expected two board CPUs")
            for name in ("usernet", "qtree", "mtree"):
                (out/f"{name}.txt").write_text(qmp.monitor("info " + name))
            match = re.search(r"TCP\[HOST_FORWARD\]\s+\d+\s+127\.0\.0\.1\s+(\d+)\s+10\.0\.2\.15\s+8080\b",
                              (out/"usernet.txt").read_text())
            require(match, "missing external device endpoint")
            port = int(match[1])
            wait_line(proc, uart, "BG READY", deadline)

            def sample_pending():
                sample(qmp, out, layout, "pending")
                time.sleep(0.12)
                sample(qmp, out, layout, "pending_later")

            exchanges = [device_reply(proc, uart, port, 1, sample_pending, deadline)]
            save(out/"peer.json", exchanges)
            # No device peer connects for the outage. Only the guests' clocks can expire it.
            wait_line(proc, uart, "BG TIMEOUT 1 2", deadline)
            exchanges.append(device_reply(proc, uart, port, 2, None, deadline))
            save(out/"peer.json", exchanges)
            while True:
                state = sample(qmp, out, layout, "final")
                if state["app"][1] == 5 and state["exit_status"] == 81:
                    break
                remaining(deadline)
                time.sleep(0.01)
            inspect(qmp, out, "end")
            qmp.call("quit")
            require(proc.wait(timeout=5) == 0, "QEMU exit")
            verdict = replay(out, args.native, args.linux)
            save(out/"result.json", {"passed": True, "measurements": verdict, "pid": proc.pid})
        except Exception as exc:
            save(out/"failure.json", {"error": repr(exc), "returncode": proc.poll()})
            if connected and proc.poll() is None:
                inspect(qmp, out, "failure")
                try:
                    sample(qmp, out, layout, "failure")
                except Exception as inner:
                    save(out/"native-failure-error.json", {"error": repr(inner)})
            raise
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            watchdog.cancel()
            watchdog.join()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("native", "linux", "out"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--bao", type=Path)
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timeout", type=float, default=35)
    parser.add_argument("--replay", action="store_true")
    args = parser.parse_args()
    if args.replay:
        print(json.dumps(replay(args.out, args.native, args.linux)))
    elif args.bao:
        observe(args)
    else:
        parser.error("--bao required for a live trial")
