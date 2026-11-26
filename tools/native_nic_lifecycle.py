"""Capture a native NIC reset and rebind on Linux QEMU virt, or replay a capture."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import secrets
import shutil
import socket
import struct
import subprocess
import tempfile
import time

from native_nic_receipts import ELFS, decode, layout
from qemu_capture import journal_location, request
from replay_nic_epoch_frames import BROADCAST, HOST, PROTOCOL, check_exchange


def require(test, message):
    if not test:
        raise ValueError(message)


def word(raw):
    match = re.fullmatch(r"\s*[0-9a-fA-Fx]+:\s*(0x[0-9a-fA-F]+)\s*", raw)
    require(match, f"unexpected HMP memory response {raw!r}")
    return int(match[1], 16)


def read_exact(sock, count):
    data = bytearray()
    while len(data) < count:
        part = sock.recv(count - len(data))
        require(part, "QEMU Ethernet socket closed")
        data.extend(part)
    return bytes(data)


def lan_announcement(number):
    # Three ordinary ARP announcements from the isolated link peer.
    address = bytes((192, 0, 2, number))
    body = struct.pack("!HHBBH6s4s6s4s", 1, 0x0800, 6, 4, 1,
                       HOST, address, bytes(6), address)
    return (BROADCAST + HOST + b"\x08\x06" + body).ljust(60, b"\0")


def verify_pcap(out, announcements):
    data = (out / "wire.pcap").read_bytes()
    byte_order = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">"}.get(data[:4])
    require(byte_order is not None and len(data) >= 24, "invalid pcap")
    require(struct.unpack_from(byte_order + "I", data, 20)[0] == 1, "expected Ethernet pcap")
    cursor = 24
    expected = [(out / name).read_bytes() for name in ("primary.challenge", "primary.reply", "primary.retire")]
    expected.extend(announcements)
    expected.extend((out / name).read_bytes() for name in ("spare.challenge", "spare.reply", "spare.retire"))
    matched = 0
    while cursor < len(data):
        require(cursor + 16 <= len(data), "truncated pcap record")
        size, wire_size = struct.unpack_from(byte_order + "II", data, cursor + 8)
        cursor += 16
        require(size == wire_size and cursor + size <= len(data), "truncated Ethernet packet")
        if matched < len(expected) and data[cursor:cursor + size] == expected[matched]:
            matched += 1
        cursor += size
    require(matched == len(expected), "missing ordered exchange in QEMU Ethernet capture")


def capture_hashes(out):
    return {path.relative_to(out).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(out.rglob("*")) if path.is_file() and path.name != "sha256.json"}


def reset_registers(path, base):
    # QEMU 6.2 logs full bus addresses for the guest CPU's MMIO operations.
    events = []
    for op, address, value in re.findall(
            r"memory_region_ops_(read|write) cpu 0 [^\n]*? addr (0x[0-9a-fA-F]+) "
            r"value (0x[0-9a-fA-F]+) size 4 name 'virtio-mmio'", path.read_text()):
        offset = int(address, 16) - base
        if offset in (0x70, 0x30, 0x44):
            events.append((op, offset, int(value, 16)))
    request_reset = ("write", 0x70, 0)
    require(events.count(request_reset) == 1, "device reset request count")
    reset = events[events.index(request_reset):]
    pos = 1
    while pos < len(reset) and reset[pos][:2] == ("read", 0x70) and reset[pos][2] != 0:
        pos += 1
    require(pos <= 1024 and reset[pos:] == [
        ("read", 0x70, 0), ("write", 0x30, 0), ("read", 0x44, 0),
        ("write", 0x30, 1), ("read", 0x44, 0)
    ], "device reset acknowledgement and disabled RX/TX queue order")
    return reset


def replay(out, *, require_mmio=False, require_lan=False):
    require(capture_hashes(out) == json.loads((out / "sha256.json").read_text()), "capture file digests")
    meta = json.loads((out / "launch.json").read_text())
    traced = "-D" in meta["argv"]
    require(traced == (out / "qemu-mmio.log").exists(), "missing or unexpected device trace")
    require(not require_mmio or traced, "device register trace required")
    require(not require_lan or meta.get("lan_announcements"), "LAN announcements required")
    if traced:
        mmio = json.loads((out / "retired.json").read_bytes())["mmio"]
        print("Reset transactions:", reset_registers(out / "qemu-mmio.log", mmio))
    package_kernel = out / "package" / "kernel.elf"
    require(meta["kernel_sha256"] == hashlib.sha256(package_kernel.read_bytes()).hexdigest(), "booted kernel digest")
    require(tuple(meta["queues"]) == journal_location(package_kernel, "jnox_native_net_queues")
            and tuple(meta["phase"]) == journal_location(package_kernel, "jnox_native_net_phase"),
            "linked device symbols")
    base, size = meta["queues"]
    require(base % 4096 == 0 and size > 0 and size % 8192 == 0, "queue symbol geometry")
    device = json.loads((out / "queue-diagnostics.json").read_text())
    probes = layout(out / "package")
    receipts = {}
    nonces = []
    positions = []
    uart = (out / "uart").read_bytes()
    for tag, phase in (("primary", 1), ("retired", 3), ("progress", 3), ("spare", 1), ("final", 3)):
        snap = json.loads((out / (tag + ".json")).read_text())
        phase_data = (out / (tag + ".phase")).read_bytes()
        require(phase_data == struct.pack("<I", phase), f"{tag} phase")
        require(word(snap["id"]) == 1 and word(snap["version"]) == 2, f"{tag} NIC identity")
        active = phase == 1
        require(word(snap["status"]) == (15 if active else 0), f"{tag} status")
        require(word(snap["ready"]) == int(active), f"{tag} selected queue ready")
        require(snap["mmio"] in range(0xa000000, 0xa004000, 0x200), f"{tag} MMIO transport")
        if device["path"] is not None:
            require(len(snap.get("device_queues", [])) == 2, f"{tag} missing queue diagnostics")
            for index, q in enumerate(snap["device_queues"]):
                require(q["queue-index"] == index, f"{tag} queue index")
                addr = q["vring-desc"]
                require(addr == (base + index * (size // 2) if active else 0),
                        f"{tag} device-side queue {index} descriptor address")
        buf = (out / (tag + ".queues")).read_bytes()
        require(len(buf) == size, f"{tag} queue capture size")
        if active:
            spans = []
            for index in range(8):
                addr, length, flags, _ = struct.unpack_from("<QIHH", buf, index * 16)
                require(length > 0 and base <= addr < addr + length <= base + size // 2 and flags == 2,
                        f"{tag} RX descriptor {index}")
                spans.append((addr, addr + length))
            spans.sort()
            require(all(a[1] <= b[0] for a, b in zip(spans, spans[1:])), f"{tag} overlapping RX buffers")
            avail = struct.unpack_from("<H", buf, 130)[0]
            used = struct.unpack_from("<H", buf, 154)[0]
            pending = (avail - used) & 0xffff
            require(used > 0 and 0 < pending <= 8, f"{tag} consumed and outstanding RX descriptors")
            ring = struct.unpack_from("<8H", buf, 132)
            outstanding = {ring[(used + i) % 8] for i in range(pending)}
            require(len(outstanding) == pending and max(outstanding) < 8, f"{tag} available ring")
        else:
            require(not any(buf), f"{tag} scrub")
        receipts[tag] = decode(probes, snap["private"])
        positions.append(snap["uart_offset"])
    require(positions == sorted(positions) and 0 <= positions[0] <= positions[-1] <= len(uart), "UART offsets")
    before, after = (receipts[tag][1] for tag in ("retired", "progress"))
    require(before["dma_peer_ticks"] < after["dma_peer_ticks"] and before["dma_peer_ns"] < after["dma_peer_ns"],
            "retired peer progress")
    primary, peer, spare = receipts["final"]
    require(primary["dma_phase"] == 2 and primary["dma_echo_count"] == 1, "primary receipts")
    require(peer["dma_phase"] == 2 and 0 < peer["dma_peer_finished"] <= peer["dma_peer_ticks"] - 5,
            "peer completion")
    require(all(peer[key] == -13 for key in ("dma_peer_rx", "dma_peer_tx", "dma_peer_claim")), "peer rights")
    require(spare["dma_phase"] == 3 and spare["dma_echo_count"] == 1 and spare["dma_spare_preclaim"] == -11
            and spare["dma_spare_claim"] == 0 and spare["dma_spare_duplicate"] == -114, "spare receipts")
    exits = re.findall(rb"NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?\n", uart)
    require(sorted(exits) == [(b"0", b"0"), (b"1", b"0"), (b"2", b"0")], "native exits")
    require(not re.search(rb"\b(?:fault|panic|exception)\b", uart, re.I), "guest fault marker")
    print("Retired peer ticks:", before["dma_peer_ticks"], "->", after["dma_peer_ticks"])
    for idx, tag in enumerate(("primary", "spare"), 1):
        nonces.append(check_exchange((out / (tag + ".challenge")).read_bytes(),
                                     (out / (tag + ".reply")).read_bytes(), idx))
    require(nonces[0] != nonces[1], "replayed challenge")
    announcements = []
    if meta.get("lan_announcements", False):
        before = json.loads((out / "retired.json").read_bytes())["stopped_ns"]
        after = json.loads((out / "progress.json").read_bytes())["stopped_ns"]
        rows = json.loads((out / "lan.json").read_bytes())
        require([r["number"] for r in rows] == [10, 11, 12], "missing LAN announcements")
        for row in rows:
            frame = bytes.fromhex(row["frame"])
            require(frame == lan_announcement(row["number"]), "ARP announcement content")
            require(before < row["sent_ns"] < after, "ARP send outside retirement observations")
            before = row["sent_ns"]
            announcements.append(frame)
    verify_pcap(out, announcements)
    print("Verified NIC phase, selected queue readiness, RX descriptors, scrub, peer progress and two wire replies.")


def capture(args):
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    kernel = args.kernel.resolve()
    pkg = out / "package"
    pkg.mkdir()
    for name in (*ELFS, "manifest.json", "kernel.elf"):
        shutil.copyfile(kernel.parent / name, pkg / name)
    kernel = pkg / "kernel.elf"
    probes = layout(pkg)
    queues = journal_location(kernel, "jnox_native_net_queues")
    phase = journal_location(kernel, "jnox_native_net_phase")
    uart = out / "uart"
    def wait_for(predicate):
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            if predicate():
                return
            require(proc.poll() is None, "QEMU exited")
            time.sleep(.025)
        raise TimeoutError("guest checkpoint timed out")

    with tempfile.TemporaryDirectory() as tmp, socket.socket() as listener, (out / "qemu.log").open("wb") as log:
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        listener.settimeout(args.timeout)
        qmp = Path(tmp) / "qmp"
        argv = [args.qemu, "-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
                "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none", "-monitor", "none",
                "-no-reboot", "-kernel", str(kernel), "-serial", "file:" + str(uart),
                "-qmp", f"unix:{qmp},server=on,wait=off",
                "-netdev", f"socket,id=lan,connect=127.0.0.1:{listener.getsockname()[1]}",
                "-global", "virtio-mmio.force-legacy=false",
                "-device", "virtio-net-device,netdev=lan,mac=52:54:00:12:34:56",
                "-object", f"filter-dump,id=wire,netdev=lan,file={out/'wire.pcap'}"]
        if args.mmio_trace:
            argv.extend(["-D", str(out / "qemu-mmio.log")])
        (out / "launch.json").write_text(json.dumps(dict(argv=argv, queues=queues, phase=phase,
            kernel_sha256=hashlib.sha256(kernel.read_bytes()).hexdigest(), lan_announcements=args.lan_announcements), indent=2))
        proc = subprocess.Popen(argv, stdout=log, stderr=log, stdin=subprocess.DEVNULL)
        try:
            wire, _ = listener.accept()
            with wire, socket.socket(socket.AF_UNIX) as control:
                wire.settimeout(args.timeout)
                wait_for(qmp.exists)
                control.settimeout(args.timeout)
                control.connect(str(qmp))
                with control.makefile("rwb", buffering=0) as stream:
                    require("QMP" in json.loads(stream.readline()), "missing QMP greeting")
                    request(stream, "qmp_capabilities")
                    commands = {item["name"] for item in request(stream, "query-commands")}
                    queue_path = None
                    if {"x-query-virtio", "x-query-virtio-queue-status"} <= commands:
                        devices = request(stream, "x-query-virtio")
                        (out / "virtio-devices.json").write_text(json.dumps(devices))
                        if len(devices) == 1:
                            queue_path = devices[0]["path"]
                    (out / "queue-diagnostics.json").write_text(json.dumps(dict(path=queue_path,
                        commands=sorted(name for name in commands if "virtio" in name))))
                    def xp(addr):
                        return request(stream, "human-monitor-command", **{"command-line": f"xp /1wx {addr:#x}"})
                    def dump(tag, addr, size):
                        request(stream, "pmemsave", val=addr, size=size, filename=str(out / tag))
                    def snapshot(tag):
                        request(stream, "stop")
                        try:
                            require(request(stream, "query-status")["status"] == "paused", "pause failed")
                            snap = {key: xp(mmio + offset) for key, offset in
                                    (("id", 8), ("version", 4), ("status", 0x70), ("ready", 0x44))}
                            if queue_path is not None:
                                snap["device_queues"] = [request(stream, "x-query-virtio-queue-status",
                                    path=queue_path, queue=index) for index in range(2)]
                            def fetch(pa, size):
                                return word(request(stream, "human-monitor-command",
                                    **{"command-line": f"xp /1gx {pa:#x}"})).to_bytes(size, "little")
                            snap["private"] = {}
                            values = decode(probes, snap["private"], fetch)
                            snap.update(mmio=mmio, uart_offset=uart.stat().st_size, stopped_ns=time.monotonic_ns())
                            dump(tag + ".phase", *phase)
                            dump(tag + ".queues", *queues)
                            (out / (tag + ".json")).write_text(json.dumps(snap))
                            return values
                        finally:
                            request(stream, "cont")
                    def send(tag, epoch, opcode):
                        frame = (BROADCAST + HOST + PROTOCOL + bytes([epoch, opcode]) + secrets.token_bytes(16)).ljust(60, b"\0")
                        (out / tag).write_bytes(frame)
                        wire.sendall(struct.pack(">I", len(frame)) + frame)
                    def exchange(tag, epoch):
                        send(tag + ".challenge", epoch, 1)
                        size = int.from_bytes(read_exact(wire, 4), "big")
                        require(14 <= size <= 65535, "invalid Ethernet length")
                        reply = read_exact(wire, size)
                        (out / (tag + ".reply")).write_bytes(reply)
                        check_exchange((out / (tag + ".challenge")).read_bytes(), reply, epoch)
                    wait_for(lambda: word(xp(phase[0])) == 1)
                    matches = [addr for addr in range(0xa000000, 0xa004000, 0x200) if word(xp(addr + 8)) == 1]
                    require(len(matches) == 1, "expected one virtio NIC")
                    mmio = matches[0]
                    exchange("primary", 1)
                    snapshot("primary")
                    # Limit emulator logging to the actual primary reset window.
                    def trace_mmio(enabled):
                        if args.mmio_trace:
                            for event_name in ("memory_region_ops_read", "memory_region_ops_write"):
                                request(stream, "trace-event-set-state", name=event_name, enable=enabled)
                    trace_mmio(True)
                    send("primary.retire", 1, 3)
                    wait_for(lambda: word(xp(phase[0])) == 3)
                    trace_mmio(False)
                    before = snapshot("retired")[1]["dma_peer_ticks"]
                    if args.lan_announcements:
                        traffic = []
                        for number in (10, 11, 12):
                            frame = lan_announcement(number)
                            wire.sendall(struct.pack(">I", len(frame)) + frame)
                            traffic.append(dict(number=number, frame=frame.hex(), sent_ns=time.monotonic_ns()))
                        (out / "lan.json").write_text(json.dumps(traffic, indent=2))
                    peer_probe = next(p for p in probes if p[1] == "dma_peer_ticks")
                    from verify_native_uart_recovery import walk
                    records = json.loads((out / "retired.json").read_text())["private"]
                    peer_pa = walk(peer_probe[2], peer_probe[3], lambda pa, size: bytes.fromhex(records[str(pa)]))
                    wait_for(lambda: word(xp(peer_pa)) > before)
                    snapshot("progress")
                    wait_for(lambda: word(xp(phase[0])) == 1)
                    exchange("spare", 2)
                    snapshot("spare")
                    send("spare.retire", 2, 3)
                    wait_for(lambda: word(xp(phase[0])) == 3)
                    wait_for(lambda: len(re.findall(rb"NATIVE EL0 EXIT PID \d+ -?\d+\r?\n", uart.read_bytes())) >= 3)
                    snapshot("final")
        finally:
            proc.terminate()
            try:
                proc.wait(5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
    (out / "sha256.json").write_text(json.dumps(capture_hashes(out), indent=2))
    replay(out, require_mmio=args.mmio_trace, require_lan=args.lan_announcements)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    parser.add_argument("--kernel", type=Path, help="native AArch64 ELF with symbols; omit for offline replay")
    parser.add_argument("--mmio-trace", action="store_true", help="retain QEMU memory-region events over the primary reset")
    parser.add_argument("--lan-announcements", action="store_true",
                        help="send three ordinary ARP announcements on the isolated link during handoff")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timeout", type=float, default=45)
    args = parser.parse_args()
    if args.kernel:
        capture(args)
    else:
        replay(args.out, require_mmio=args.mmio_trace, require_lan=args.lan_announcements)
