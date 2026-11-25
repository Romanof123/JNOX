#!/usr/bin/env python3
"""Run a single cold QEMU position with bounded loopback or isolated TAP peers."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import struct
import subprocess
import tempfile
import threading
import time


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sock(kind, address, port, timeout):
    s = socket.socket(socket.AF_INET, kind)
    s.settimeout(timeout)
    s.bind((address, port))
    return s


def exact(s, size):
    data = bytearray()
    while len(data) < size:
        chunk = s.recv(size - len(data))
        require(chunk, "unexpected TCP EOF")
        data.extend(chunk)
    return bytes(data)


def tcp_peer(listener, target, log, timeout):
    c, source = listener.accept()
    with c:
        c.settimeout(timeout)
        data = exact(c, 9)
        log("tcp_out_rx", peer=source, data=data.hex())
        require(data == b"GUEST/out" and c.recv(1) == b"", "outbound bytes or FIN")
        for part in (b"HOST/", b"outbound"):
            c.sendall(part)
            log("tcp_out_tx", data=part.hex())
        c.shutdown(socket.SHUT_WR)
    # The first exchange proves the guest has already installed its listener.
    with socket.create_connection((target, 43001), timeout) as c:
        for part in (b"HOST/", b"inbound"):
            c.sendall(part)
        c.shutdown(socket.SHUT_WR)
        data = exact(c, 14)
        log("tcp_in_rx", peer=c.getpeername(), data=data.hex())
        require(data == b"GUEST/accepted" and c.recv(1) == b"", "accepted bytes or FIN")
    c, source = listener.accept()
    with c:
        c.settimeout(timeout)
        data = exact(c, 4)
        require(data == b"ARM!", "unexpected reset request")
        c.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        log("tcp_abortive_close", peer=source, received=data.hex())
    log("tcp_peer_done")


def udp_peer(s, log):
    packets = {}
    source = None
    for expected in range(1, 5):
        data, address = s.recvfrom(128)
        log("udp_before_relay", peer=address, data=data.hex())
        require(source in (None, address), "UDP sender changed")
        source = address
        require(data == bytes([ord("U"), expected, 10 * expected]), "pre-relay payload/order")
        packets[expected] = data
    log("udp_drop", peer=source, original=packets[2].hex())
    for seq in (1, 4, 3, 0):
        data = bytes([ord("R"), seq, 10 * seq])
        require(s.sendto(data, source) == len(data), "UDP short send")
        log("udp_after_relay", peer=source, data=data.hex())
    data, address = s.recvfrom(128)
    log("udp_final_rx", peer=address, data=data.hex())
    require(address == source and data == b"DONE", "missing post-window guest receipt")


def capture_ram(qmp_path, path):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.settimeout(30)
        s.connect(str(qmp_path))
        with s.makefile("rwb") as f:
            require("QMP" in json.loads(f.readline()), "invalid QMP greeting")
            def cmd(op, args=None):
                req = {"execute": op}
                if args:
                    req["arguments"] = args
                f.write(json.dumps(req).encode() + b"\n")
                f.flush()
                while True:
                    msg = json.loads(f.readline())
                    if "event" not in msg:
                        require("return" in msg, str(msg))
                        return
            cmd("qmp_capabilities")
            cmd("stop")
            cmd("pmemsave", {"val": 0x40000000, "size": 128 << 20, "filename": str(path)})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("package", type=Path)
    p.add_argument("out", type=Path)
    p.add_argument("--qemu", default="qemu-system-aarch64")
    p.add_argument("--timeout", type=int, default=120)
    p.add_argument("--isolated-tap", help="preconfigured TAP with host 10.0.2.2/24 in private netns")
    args = p.parse_args()
    out = args.out.resolve()
    out.mkdir()
    kernel = (args.package / "kernel.elf").resolve(strict=True)
    host = "10.0.2.2" if args.isolated_tap else "127.0.0.1"
    target = "10.0.2.15" if args.isolated_tap else host
    if args.isolated_tap:
        require(os.stat("/proc/self/ns/net").st_ino != os.stat("/proc/1/ns/net").st_ino,
                "TAP requires a separate network namespace")
        require(re.fullmatch(r"[A-Za-z0-9_-]{1,15}", args.isolated_tap), "invalid TAP name")
        netdev = f"tap,id=lan,ifname={args.isolated_tap},script=no,downscript=no"
    else:
        netdev = ("user,id=lan,net=10.0.2.0/24,host=10.0.2.2,"
                  "hostfwd=tcp:127.0.0.1:43001-10.0.2.15:43001")
    lock = threading.Lock()
    with (out / "peer.jsonl").open("x") as journal, tempfile.TemporaryDirectory() as tmp:
        def log(kind, **items):
            with lock:
                journal.write(json.dumps(dict(t_ns=time.monotonic_ns(), kind=kind, **items)) + "\n")
                journal.flush()
        qmp = Path(tmp) / "qmp.sock"
        argv = [args.qemu, "-machine", "virt,gic-version=2", "-cpu", "cortex-a72",
                "-smp", "1", "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none",
                "-monitor", "none", "-no-reboot", "-kernel", str(kernel),
                "-serial", "file:" + str(out / "uart"), "-qmp", f"unix:{qmp},server=on,wait=off",
                "-netdev", netdev, "-global", "virtio-mmio.force-legacy=false",
                "-device", "virtio-net-device,netdev=lan,mac=52:54:00:12:34:56",
                "-object", f"filter-dump,id=wire,netdev=lan,file={out/'wire.pcap'}"]
        (out / "launch.json").write_text(json.dumps(dict(argv=argv, kernel_sha256=
            hashlib.sha256(kernel.read_bytes()).hexdigest(), timeout=args.timeout), indent=2))
        status = {}
        with (
            sock(socket.SOCK_STREAM, host, 43002, args.timeout) as tcp,
            sock(socket.SOCK_DGRAM, host, 43004, args.timeout) as udp,
            (out / "qemu.log").open("wb") as stderr,
            concurrent.futures.ThreadPoolExecutor(2) as pool,
        ):
            tcp.listen(2)
            proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=stderr, stderr=stderr)
            futures = {name:pool.submit(fn, *arguments) for name, fn, arguments in (
                ("tcp", tcp_peer, (tcp, target, log, args.timeout)), ("udp", udp_peer, (udp, log)))}
            deadline = time.monotonic() + args.timeout
            try:
                while proc.poll() is None and time.monotonic() < deadline:
                    uart = (out / "uart").read_bytes() if (out / "uart").exists() else b""
                    rows = re.findall(rb"NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?\n", uart)
                    if len(rows) >= 3 or b"NATIVE EL0 FAULT" in uart:
                        break
                    time.sleep(0.05)
                if proc.poll() is None:
                    capture_ram(qmp, out / "ram.bin")
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    try:
                        proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        proc.kill()
                        proc.wait()
                for name,future in futures.items():
                    try:
                        future.result(timeout=max(0, deadline-time.monotonic())+1)
                        status[name]="complete"
                    except Exception as error:
                        status[name]=repr(error)
                uart = (out / "uart").read_bytes() if (out / "uart").exists() else b""
                status["guest_exits"] = [[int(pid),int(code)] for pid,code in
                    re.findall(rb"NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?\n", uart)]
                (out / "status.json").write_text(json.dumps(status,indent=2))
        return 0 if (status.get("tcp")==status.get("udp")=="complete" and
                     status["guest_exits"]==[[0,75],[1,76],[2,0]]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
