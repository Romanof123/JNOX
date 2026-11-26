#!/usr/bin/env python3
"""Exercise the bounded diagnostic gateway over two isolated AArch64 QEMU guests."""
import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import re
import shutil
import socket
import struct
import subprocess
import tempfile
import time

from linux_gateway_evidence import BASELINE, capture_receipts, verify
from ordinary_amc_capture import sha
from qemu_capture import request
from verify_linux_virtio_net_baseline import MAC, require


GUEST_MAC = bytes.fromhex(MAC.replace(":", ""))
PEER_MAC = bytes.fromhex("525400223344")
GUEST_IP = socket.inet_aton("10.0.2.15")
PEER_IP = socket.inet_aton("10.0.2.2")
DEVICE = f"virtio-net-device,netdev=lan,mac={MAC}"
GUEST_PORT = 43003
PEER_PORT = 43004


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def remaining(deadline):
    seconds = deadline - time.monotonic()
    if seconds <= 0:
        raise TimeoutError("integration deadline expired")
    return seconds


def exact(sock, size, deadline):
    data = bytearray()
    while len(data) < size:
        sock.settimeout(remaining(deadline))
        chunk = sock.recv(size - len(data))
        require(chunk, "unexpected stream EOF")
        data.extend(chunk)
    return bytes(data)


def checksum(data):
    if len(data) % 2:
        data += b"\0"
    value = sum(x[0] for x in struct.iter_unpack("!H", data))
    while value >> 16:
        value = (value & 0xffff) + (value >> 16)
    return ~value & 0xffff


class WirePeer:
    """A single private Ethernet neighbor. QEMU's socket backend prefixes each frame with its length."""

    def __init__(self, sock):
        self.sock = sock

    def send(self, frame, deadline):
        self.sock.settimeout(remaining(deadline))
        self.sock.sendall(struct.pack("!I", len(frame)) + frame)

    def ipv4(self, frame):
        if len(frame) < 42 or frame[:6] != PEER_MAC or frame[6:12] != GUEST_MAC:
            return None
        ip = frame[14:]
        if (ip[0] != 0x45 or ip[9] != 17 or ip[12:20] != GUEST_IP + PEER_IP
                or int.from_bytes(ip[6:8], "big") & 0x3fff):
            return None
        total = int.from_bytes(ip[2:4], "big")
        udp = ip[20:total]
        if len(udp) != 32 or total != 52 or checksum(ip[:20]) != 0:
            return None
        sport, dport, length, check = struct.unpack("!HHHH", udp[:8])
        if (sport, dport, length) != (GUEST_PORT, PEER_PORT, len(udp)):
            return None
        if check and checksum(GUEST_IP + PEER_IP + struct.pack("!BBH", 0, 17, length) + udp) != 0:
            return None
        return udp[8:]

    def receive(self, deadline):
        while True:
            length, = struct.unpack("!I", exact(self.sock, 4, deadline))
            require(14 <= length <= 2048, "invalid isolated Ethernet frame size")
            frame = exact(self.sock, length, deadline)
            if frame[12:14] == b"\x08\x06":
                arp = frame[14:42]
                if (frame[6:12] == GUEST_MAC and len(arp) == 28
                        and arp[:18] == bytes.fromhex("0001080006040001") + GUEST_MAC + GUEST_IP
                        and arp[24:] == PEER_IP):
                    reply = bytes.fromhex("0001080006040002") + PEER_MAC + PEER_IP + GUEST_MAC + GUEST_IP
                    self.send(GUEST_MAC + PEER_MAC + b"\x08\x06" + reply, deadline)
            if frame[12:14] == b"\x08\x00":
                payload = self.ipv4(frame)
                if payload is not None:
                    return payload

    def reply(self, payload, deadline):
        require(len(payload) == 24, "unexpected reply length")
        length = 8 + len(payload)
        ports = struct.pack("!HHH", PEER_PORT, GUEST_PORT, length)
        pseudo = PEER_IP + GUEST_IP + struct.pack("!BBH", 0, 17, length)
        check = checksum(pseudo + ports + b"\0\0" + payload) or 0xffff
        udp = ports + struct.pack("!H", check) + payload
        ip = struct.pack("!BBHHHBBH", 0x45, 0, 20 + length, 0, 0, 64, 17, 0) + PEER_IP + GUEST_IP
        ip = ip[:10] + struct.pack("!H", checksum(ip)) + ip[12:]
        self.send(GUEST_MAC + PEER_MAC + b"\x08\x00" + ip + udp, deadline)


def unix_connect(path, proc, deadline):
    while True:
        remaining(deadline)
        require(proc.poll() is None, "QEMU exited during startup; inspect qemu.stderr")
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            sock.settimeout(min(1, remaining(deadline)))
            sock.connect(str(path))
            return sock
        except (ConnectionRefusedError, FileNotFoundError):
            sock.close()
            time.sleep(0.01)
        except BaseException:
            sock.close()
            raise


class Guest:
    def __init__(self, stack, root, tmp, qemu, args, deadline, fds=(), identities=None):
        self.root = root
        self.deadline = deadline
        self.stack = stack
        root.mkdir()
        qmp_path = tmp / (root.name + ".qmp")
        argv = [qemu, "-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
                "-accel", "tcg", "-nodefaults", "-no-reboot", "-display", "none", "-monitor", "none",
                "-S", "-qmp", f"unix:{qmp_path},server=on,wait=off", *args,
                "-object", f"filter-dump,id=wire,netdev=lan,file={root/'wire.pcap'}"]
        save(root / "launch.json", {"argv": argv, "qemu_sha256": sha(Path(qemu)), **(identities or {})})
        stderr = stack.enter_context((root / "qemu.stderr").open("xb"))
        self.proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=stderr, stderr=stderr,
                                     pass_fds=fds)
        stack.callback(self.cleanup)
        self.qsock = stack.enter_context(unix_connect(qmp_path, self.proc, deadline))
        self.qsock.settimeout(min(5, remaining(deadline)))
        self.pipe = stack.enter_context(self.qsock.makefile("rwb", buffering=0))
        self.greeting = json.loads(self.pipe.readline())
        require("QMP" in self.greeting, "missing QMP greeting")
        self.cmd("qmp_capabilities")
        save(root / "qmp.json", {"greeting": self.greeting, "initial": self.cmd("query-status"),
                                  "pid": self.proc.pid})

    def cmd(self, op, **kwargs):
        self.qsock.settimeout(min(5, remaining(self.deadline)))
        return request(self.pipe, op, **kwargs)

    def quit(self):
        self.cmd("quit")
        code = self.proc.wait(timeout=min(5, remaining(self.deadline)))
        require(code == 0, f"controlled QEMU shutdown failed ({code})")
        return code

    def cleanup(self):
        if self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait(timeout=5)


class Linux(Guest):
    def __init__(self, stack, out, tmp, qemu, image, generation, deadline, event, native_probe=None):
        root = out / f"linux-{generation}"
        serial_path = tmp / f"linux-{generation}.serial"
        self.generation = generation
        self.event = event
        super().__init__(stack, root, tmp, qemu, [
            "-m", "384M", "-chardev", f"socket,id=uart,path={serial_path},server=on,wait=off",
            "-serial", "chardev:uart", "-kernel", str(image / "Image"), "-initrd", str(image / "initrd.gz"),
            "-append", f"console=ttyAMA0 rdinit=/init panic=-1 loglevel=3 jnox.generation={generation}",
            "-netdev", "user,id=lan,restrict=on,ipv6=off,hostfwd=tcp:127.0.0.1:0-10.0.2.15:8080",
            "-device", DEVICE], deadline, identities={"image_sha256": sha(image / "Image"),
                                                      "initrd_sha256": sha(image / "initrd.gz")})
        self.serial = stack.enter_context(unix_connect(serial_path, self.proc, deadline))
        self.serial.settimeout(remaining(deadline))
        self.lines = stack.enter_context(self.serial.makefile("rb"))
        self.log = stack.enter_context((root / "uart.log").open("xb", buffering=0))
        self.input = stack.enter_context((root / "input.bin").open("xb", buffering=0))
        info = self.cmd("human-monitor-command", **{"command-line": "info usernet"})
        (root / "usernet.txt").write_text(info)
        port = re.search(r"TCP\[HOST_FORWARD\]\s+\d+\s+127\.0\.0\.1\s+(\d+)\s+10\.0\.2\.15\s+8080\b", info)
        require(port, "missing restricted device peer forwarding rule")
        self.port = int(port[1])
        self.cmd("cont")
        if native_probe is not None:
            native_probe("linux_boot")
        self.wait_line(b"GWREADY " + str(generation).encode())
        event("ready", generation=generation, pid=self.proc.pid)
        if native_probe is not None:
            native_probe("linux_ready")

    def wait_line(self, prefix):
        while True:
            self.serial.settimeout(remaining(self.deadline))
            line = self.lines.readline(8193)
            require(line and len(line) <= 8192 and self.log.tell() + len(line) <= 2**20,
                    "invalid or missing Linux serial output")
            self.log.write(line)
            line = line.rstrip(b"\r\n")
            if line.startswith(prefix):
                if prefix.startswith(b"GWREADY"):
                    require(line == prefix, "wrong Linux generation")
                return line

    def exchange(self, record):
        gen = self.generation
        deadline = min(self.deadline, time.monotonic() + int.from_bytes(record[12:16], "big") / 1000)
        line = b"GWREQ " + record.hex().encode() + b"\n"
        self.serial.settimeout(remaining(deadline))
        self.serial.sendall(line)
        self.input.write(line)
        self.event("serial_request", generation=gen, hex=record.hex())
        with socket.create_connection(("127.0.0.1", self.port), timeout=remaining(deadline)) as nic:
            actual = exact(nic, 24, deadline)
            self.event("nic_device_request", generation=gen, hex=actual.hex())
            require(actual == record, "Linux NIC request differs from submitted RPC")
            transformed = bytes(b ^ 0xa5 for b in actual[16:])
            nic.settimeout(remaining(deadline))
            nic.sendall(transformed)
            self.event("nic_device_reply", generation=gen, hex=transformed.hex())
        result = self.wait_line(b"GWREP ")
        require(re.fullmatch(rb"GWREP [0-9a-f]{48}", result), "invalid Linux serial reply")
        response = bytes.fromhex(result[6:].decode())
        require(response == record[:3] + b"\x02" + record[4:16] + transformed, "invalid device completion")
        self.event("serial_reply", generation=gen, hex=response.hex())
        return response

    def stop(self):
        code = self.quit()
        self.event("linux_stopped", generation=self.generation, pid=self.proc.pid, returncode=code)
        return code


def freeze(src, dest, linux, out):
    require(BASELINE.is_file(), f"missing tracked Linux baseline (check sparse checkout): {BASELINE}")
    baseline = json.loads(BASELINE.read_bytes())["operands"]
    manifest = json.loads((src / "manifest.json").read_bytes())
    dest.mkdir()
    for name in ("manifest.json", "kernel.elf", "build.log", *(row["elf"] for row in manifest["slots"])):
        require(Path(name).name == name, "invalid native package filename")
        shutil.copyfile(src / name, dest / name)
    require(sha(dest / "kernel.elf") == manifest["kernel_elf_sha256"] and
            sha(dest / "build.log") == manifest["build_log_sha256"] and
            all(sha(dest / row["elf"]) == row["elf_sha256"] for row in manifest["slots"]),
            "native package hash mismatch")
    operands = json.loads((linux / "operands.json").read_bytes())
    require(operands["image_sha256"] == baseline["image_sha256"] and
            operands["modules"] == baseline["modules"], "Linux image or modules differ from #416")
    require(sha(linux / "Image") == operands["image_sha256"] and
            sha(linux / "initrd.gz") == operands["initrd_sha256"], "Linux image hash mismatch")
    save(out / "linux-operands.json", operands)


def run(out, linux, qemu, deadline, event):
    package = out / "package"
    with tempfile.TemporaryDirectory(prefix="gw417-") as tmpdir, ExitStack() as stack:
        tmp = Path(tmpdir)
        first = Linux(stack, out, tmp, qemu, linux, 1, deadline, event)
        sock, child = socket.socketpair()
        stack.enter_context(sock)
        stack.enter_context(child)
        root = out / "native"
        native = Guest(stack, root, tmp, qemu, ["-m", "128M", "-serial", "file:" + str(root / "uart.log"),
                       "-kernel", str(package / "kernel.elf"), "-netdev", f"socket,id=lan,fd={child.fileno()}",
                       "-global", "virtio-mmio.force-legacy=false", "-device", DEVICE],
                       deadline, fds=(child.fileno(),), identities={"kernel_sha256": sha(package / "kernel.elf")})
        child.close()
        link = WirePeer(sock)
        native.cmd("cont")

        def read(gen, seq, ms, tag):
            record = link.receive(deadline)
            generation, rid, budget = struct.unpack("!III", record[4:16])
            event("jnox_request", generation=generation, id=rid, hex=record.hex())
            require(record[:4] == b"JG\x01\x01" and (generation, rid, budget) == (gen, seq, ms),
                    f"unexpected native request during {tag}")
            capture_receipts(native.cmd, package, out, tag)
            return record

        def deliver(record, delivery):
            link.reply(record, deadline)
            event("jnox_reply", generation=int.from_bytes(record[4:8], "big"), hex=record.hex(),
                  delivery=delivery)

        initial = read(1, 1, 10000, "primary")
        saved = first.exchange(initial)
        stopped = first.stop()
        deliver(saved, "current")
        read(1, 2, 250, "outage")
        require(first.proc.poll() == stopped, "retired Linux process still running")
        event("offline", generation=1, id=2, linux_pid=first.proc.pid, linux_returncode=stopped)
        recovered = read(2, 1, 10000, "recovery")
        require(recovered[8:] == initial[8:], "stale-control request differs outside generation")
        deliver(saved, "stale")
        def startup_probe(tag):
            event("linux_boot_probe", generation=2, tag=tag, edge="start")
            capture_receipts(native.cmd, package, out, tag)
            event("linux_boot_probe", generation=2, tag=tag, edge="end")

        second = Linux(stack, out, tmp, qemu, linux, 2, deadline, event, native_probe=startup_probe)
        deliver(second.exchange(recovered), "current")
        second.stop()
        while True:
            remaining(deadline)
            require(native.proc.poll() is None, "JNOX terminated before exit report")
            raw = (root / "uart.log").read_bytes()
            require(b"NATIVE EL0 FAULT" not in raw, "native application fault")
            exits = re.findall(rb"^NATIVE EL0 EXIT PID (\d+) (-?\d+)$", raw, re.M)
            if len(exits) == 3:
                break
            time.sleep(0.015)
        native.cmd("stop")
        capture_receipts(native.cmd, package, out, "final")
        code = native.quit()
        event("native_stopped", pid=native.proc.pid, returncode=code)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path)
    parser.add_argument("--linux", type=Path, help="prepared Image, initrd.gz and operands.json")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timeout", type=int, default=90)
    parser.add_argument("--replay", type=Path)
    args = parser.parse_args()
    if args.replay:
        print(json.dumps(verify(args.replay.resolve()), indent=2))
        return
    if not (args.package and args.linux and args.out and args.timeout > 0):
        parser.error("supply --package, --linux, --out and a positive timeout")
    out = args.out.resolve()
    out.mkdir(parents=True)
    linux = args.linux.resolve()
    result = {"passed": False}
    try:
        freeze(args.package.resolve(), out / "package", linux, out)
        qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
        with (out / "events.jsonl").open("x", buffering=1) as events:
            def event(name, **fields):
                events.write(json.dumps({"event": name, "t_ns": time.monotonic_ns(), **fields}) + "\n")
            run(out, linux, qemu, time.monotonic() + args.timeout, event)
        result["observed"] = verify(out)
        result["passed"] = True
    except Exception as exc:
        result["error"] = str(exc)
        raise
    finally:
        save(out / "result.json", result)
    print("PASS JNOX/Linux device gateway, Linux outage and fresh-generation completion")


if __name__ == "__main__":
    main()
