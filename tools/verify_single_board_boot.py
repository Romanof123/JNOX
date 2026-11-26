#!/usr/bin/env python3
"""Capture interleaved JNOX EL0 progress and stock Linux NIC exchanges on one Bao board."""
import argparse
import json
from pathlib import Path
import re
import secrets
import shutil
import struct
import subprocess
import threading
import time

from linux_gateway_evidence import BASELINE, GUEST, packets
from native_amc_roots import linked_roots
from native_el0_replay import Snapshot, permissions, vread
from qemu_capture import journal_location, request
from verify_linux_virtio_net_baseline import MAC, guest_observations, http, require, save, sha


def load(path):
    return json.loads(path.read_bytes())


class QMP:
    def __init__(self, proc):
        self.proc = proc

    def write(self, data):
        self.proc.stdin.write(data)
        self.proc.stdin.flush()

    def readline(self):
        return self.proc.stdout.readline()

    def call(self, name, **args):
        return request(self, name, **args)

    def monitor(self, cmd, **args):
        return self.call("human-monitor-command", **{"command-line": cmd, **args})


class NativeRAM(Snapshot):
    """Read only the symbol or translation entries requested, in JNOX stage-1 IPA space."""
    def __init__(self, qmp, out):
        self.qmp = qmp
        self.dest = out / "current-read.bin"
        self.records = []

    def read(self, ipa, size):
        require(0x40000000 <= ipa < ipa + size <= 0x48000000 and size <= 4096,
                "read outside partition or excessive extent")
        pa = ipa + 0x08000000
        self.qmp.call("pmemsave", val=pa, size=size, filename=str(self.dest))
        raw = self.dest.read_bytes()
        require(len(raw) == size, "truncated machine memory sample")
        self.records.append({"ipa": hex(ipa), "pa": hex(pa), "bytes": raw.hex()})
        return raw


def native_layout(pkg):
    manifest = load(pkg / "manifest.json")
    kernel = pkg / "kernel.elf"
    slot = manifest["slots"][0]
    require(slot["slot"] == 0 and Path(slot["elf"]).name == slot["elf"], "primary workload slot")
    user = pkg / slot["elf"]
    kernel_bytes, user_bytes = kernel.read_bytes(), user.read_bytes()
    require(sha(kernel_bytes) == manifest["kernel_elf_sha256"] and
            sha(user_bytes) == slot["elf_sha256"] and user_bytes in kernel_bytes, "native image hashes")
    syms = {}
    for name, size in (("boot_log", 4096), ("boot_len", 8), ("timer_irqs", 8), ("last_irq_ticks", 8)):
        addr, length = journal_location(kernel, "jnox_partition_" + name)
        require(size == length, "unexpected native journal extent")
        syms[name] = addr
    syms["el0_journal"], journal_size = journal_location(kernel, "jnox_native_el0_journal")
    require(journal_size >= 16 * 8, "truncated linked EL0 journal header")
    user_va, size = journal_location(user, "jnox_partition_heartbeat")
    require(size == 16 and user_va % 8 == 0, "unexpected heartbeat layout")
    return syms, linked_roots(kernel, len(manifest["slots"]))[0], user_va, slot["elf_sha256"]


def decode_native(mem, layout):
    syms, root, va, identity = layout
    def u64(name):
        return int.from_bytes(mem.read(syms[name], 8), "little")
    n = u64("boot_len")
    require(0 < n <= 4096, "empty or corrupt JNOX boot journal")
    boot = mem.read(syms["boot_log"], n)
    require(b"JNOX BAO PARTITION BOOT" in boot.splitlines() and
            ("NATIVE EL0 ELF " + identity).encode() in boot.splitlines(), "JNOX loaded workload identity")
    irqs, tick = u64("timer_irqs"), u64("last_irq_ticks")
    header = struct.unpack("<16Q", mem.read(syms["el0_journal"], 16 * 8))
    require(header[1] == 2 and header[2] == 0 and header[12] == 4 and header[14] == root,
            "native EL0 journal state, fault or task root differs from linked workload")
    for page in {va & ~4095, (va + 15) & ~4095}:
        permissions(mem, root, page, "user rw")
    count, ns = struct.unpack("<QQ", vread(mem, root, va, 16))
    return boot, {"count": count, "task_ns": ns, "timer_irqs": irqs, "last_irq_ticks": tick,
                  "root": hex(root), "task_va": hex(va), "kernel_symbols_ipa": syms}


def sample(qmp, out, layout, index):
    mem = NativeRAM(qmp, out)
    start = time.monotonic_ns()
    try:
        boot, decoded = decode_native(mem, layout)
    finally:
        # Keep the bounded raw boot journal even if image entry or a page walk failed.
        if len(mem.records) >= 2:
            (out / f"native-{index}.log").write_bytes(bytes.fromhex(mem.records[1]["bytes"]))
    data = dict(decoded, start_ns=start, end_ns=time.monotonic_ns(), reads=mem.records)
    save(out / f"native-{index}.json", data)
    return data


class RetainedRAM(Snapshot):
    def __init__(self, records):
        self.records = iter(records)

    def read(self, ipa, size):
        read = next(self.records)
        raw = bytes.fromhex(read["bytes"])
        require(int(read["ipa"],16) == ipa and int(read["pa"],16) == ipa + 0x08000000
                and 0x40000000 <= ipa < ipa + size <= 0x48000000 and len(raw) == size <= 4096,
                "unexpected retained physical read")
        return raw


def replay(out, native, linux):
    layout = native_layout(native)
    events = load(out / "challenges.json")
    require(len(events) == 4 and len({e["nonce"] for e in events}) == 4, "challenge count or reuse")
    dtb = (out / "running.dtb").read_bytes()
    require(dtb == (linux / "guest.dtb").read_bytes(), "running FDT differs")
    stock = load(BASELINE)["operands"]
    uart = (out / "uart.log").read_text(errors="replace").splitlines()
    samples = []
    for i, event in enumerate(events):
        nonce = event["nonce"]
        require(re.fullmatch("[0-9a-f]{64}", nonce) and event["begin_ns"] < event["end_ns"],
                "invalid challenge or interval")
        require(all(line in uart for line in guest_observations(stock, nonce, dtb)), "missing UART observation")
        require((out / f"challenge-{i}.request").read_bytes() == f"GET /echo/{nonce} HTTP/1.0\r\n\r\n".encode()
                and (out / f"challenge-{i}.response").read_bytes().partition(b"\r\n\r\n")[2] ==
                (nonce + "\n").encode(), "retained challenge differs")
        if i + 1 < len(events):
            data = load(out / f"native-{i}.json")
            ram = RetainedRAM(data["reads"])
            boot, decoded = decode_native(ram, layout)
            require(next(ram.records, None) is None and all(data[k] == v for k,v in decoded.items())
                    and boot == (out / f"native-{i}.log").read_bytes(), "retained native data differs")
            require(event["end_ns"] <= data["start_ns"] <= data["end_ns"] <= events[i+1]["begin_ns"],
                    "native sample outside challenge interval")
            samples.append(data)
    for left, right in zip(samples, samples[1:]):
        require(all(0 < left[k] < right[k] for k in ("count", "task_ns", "timer_irqs", "last_irq_ticks")),
                "missing EL0 or timer progress")
    require(samples[-1]["start_ns"] - samples[0]["end_ns"] > 150_000_000, "short native interval")
    require((out / "dtb.response").read_bytes().partition(b"\r\n\r\n")[2] == dtb, "wire FDT differs")
    check_wire(out, ["dtb", *(f"challenge-{i}" for i in range(4))])


def check_wire(out, names):
    streams = {}
    for src, dst, proto, tcp in packets(out / "wire.pcap"):
        if proto != 6 or len(tcp) < 20:
            continue
        sp, dp, seq = struct.unpack_from("!HHI", tcp)
        received = dst == GUEST and dp == 8080
        if not received and not (src == GUEST and sp == 8080):
            continue
        offset = 4 * (tcp[12] >> 4)
        require(20 <= offset <= len(tcp), "invalid TCP header")
        if offset == len(tcp):
            continue
        peer = (src, sp) if received else (dst, dp)
        side = streams.setdefault(peer, {True: {}, False: {}})[received]
        for i, byte in enumerate(tcp[offset:]):
            key = (seq + bool(tcp[13] & 2) + i) & 0xffffffff
            require(side.setdefault(key, byte) == byte, "conflicting retransmission")

    def assemble(parts):
        starts = [pos for pos in parts if (pos - 1) & 0xffffffff not in parts]
        require(len(starts) == 1, "incomplete TCP stream")
        return bytes(parts[(starts[0] + i) & 0xffffffff] for i in range(len(parts)))

    flows = [(assemble(sides[True]), assemble(sides[False])) for sides in streams.values()
             if sides[True] and sides[False]]
    for name in names:
        pair = tuple((out / f"{name}.{suffix}").read_bytes() for suffix in ("request", "response"))
        require(pair in flows, f"missing exact bidirectional wire record for {name}")


def uart_irq_report(out):
    uart = out / "uart.log"
    blocks = []
    current = None
    for line in uart.read_text(errors="replace").splitlines() if uart.exists() else ():
        if line.startswith("LD IRQ BEGIN "):
            current = {"phase": line.removeprefix("LD IRQ BEGIN "), "complete": False, "rows": []}
            blocks.append(current)
        elif current and line == "LD IRQ END " + current["phase"]:
            current["complete"] = True
            current = None
        elif current:
            row = {"text": line}
            match = re.match(r"^\s*(\d+):\s+(\d+)\s+(GIC\S*)\s+(\d+)\s+(.*)$", line)
            if match:
                row.update(virq=int(match[1]), count=int(match[2]), controller=match[3],
                           hwirq=int(match[4]), description=match[5])
            current["rows"].append(row)
    nic = [{"phase": block["phase"], "count": row["count"], "virq": row["virq"]}
           for block in blocks if block["complete"] for row in block["rows"]
           if row.get("hwirq") == 79 and re.search("virtio|eth0|a003e00", row["description"], re.I)]
    save(out / "linux-irqs.json", {"blocks": blocks, "nic_irq79_samples": nic})


def inspect(qmp, out, stage):
    cpus = qmp.call("query-cpus-fast")
    save(out / f"cpus-{stage}.json", cpus)
    for cpu in cpus:
        idx = cpu["cpu-index"]
        (out / f"cpu{idx}-{stage}.txt").write_text(qmp.monitor("info registers", **{"cpu-index": idx}))
    return cpus


def run(args):
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    qemu = shutil.which(args.qemu)
    require(qemu, "QEMU executable unavailable")
    loader = load(args.loader_args) if args.loader_args else ["-kernel", str(args.bao.resolve())]
    require(isinstance(loader, list) and all(isinstance(x, str) for x in loader), "invalid loader argv")
    argv = [qemu, "-machine", "virt,virtualization=on,secure=off,gic-version=2", "-cpu", "cortex-a72",
            "-accel", "tcg", "-smp", "2", "-m", "512M", "-nodefaults", "-no-reboot", "-display", "none",
            "-monitor", "none", "-qmp", "stdio", "-serial", f"file:{out/'uart.log'}", *loader,
            "-global", "virtio-mmio.force-legacy=false", "-netdev",
            "user,id=lan,restrict=on,ipv6=off,hostfwd=tcp:127.0.0.1:0-10.0.2.15:8080",
            "-device", f"virtio-net-device,netdev=lan,mac={MAC}",
            "-object", f"filter-dump,id=wire,netdev=lan,file={out/'wire.pcap'}"]
    if args.trace_events:
        events = args.trace_events.read_text()
        (out / "trace-events.txt").write_text(events)
        argv.extend(["-trace", f"events={out/'trace-events.txt'},file={out/'device.trace'}"])
    version = subprocess.check_output([qemu, "--version"], text=True).splitlines()[0]
    launch = {"argv": argv, "qemu_version": version, "bao_sha256": sha(args.bao.read_bytes()),
              "qemu_sha256": sha(Path(qemu).read_bytes())}
    save(out / "launch.json", launch)
    deadline = time.monotonic() + args.timeout
    with (out / "qemu.stderr").open("wb") as err, subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as proc:
        save(out / "launch.json", dict(launch, pid=proc.pid))
        guard = threading.Timer(args.timeout + 8, proc.kill)
        guard.start()
        qmp = QMP(proc)
        negotiated = False
        layout = None
        try:
            require("QMP" in json.loads(qmp.readline()), "missing greeting")
            qmp.call("qmp_capabilities")
            negotiated = True
            cpus = inspect(qmp, out, "start")
            require(sorted(c["cpu-index"] for c in cpus) == [0, 1], "expected two board CPUs")
            for cmd in ("qtree", "mtree", "usernet"):
                (out / f"{cmd}.txt").write_text(qmp.monitor("info " + cmd))
            tree = (out / "qtree.txt").read_text()
            require("virtio-net-device" in tree and MAC in tree and "virtio-mmio" in tree,
                    "guest NIC missing from board qtree")
            port = re.search(r"TCP\[HOST_FORWARD\]\s+\d+\s+127\.0\.0\.1\s+(\d+)\s+10\.0\.2\.15\s+8080\b",
                             (out / "usernet.txt").read_text())
            require(port, "missing Linux host forward")
            port = int(port[1])
            layout = native_layout(args.native)
            while b"LD READY" not in (out / "uart.log").read_bytes():
                require(time.monotonic() < deadline and proc.poll() is None, "Linux readiness timeout")
                time.sleep(0.1)
            operands = load(args.linux / "linux.json")["operands"]
            stock = load(BASELINE)["operands"]
            require(all(operands[k] == stock[k] for k in ("release", "packages", "image_sha256",
                    "modules", "vmlinuz_sha256", "config_sha256")), "stock Linux identity differs")
            dtb = http(port, "/board.dtb", out, "dtb", deadline)
            require(dtb == (args.linux / "guest.dtb").read_bytes(), "guest FDT differs from supplied tree")
            (out / "running.dtb").write_bytes(dtb)
            for path, prop, value in [("/memory@50000000", "reg", [0,0x50000000,0,0xc000000]),
                    ("/virtio_mmio@a003e00", "reg", [0,0xa003e00,0,0x200]),
                    ("/virtio_mmio@a003e00", "interrupts", [0,47,1]), ("/cpus/cpu@0", "reg", [0]),
                    ("/pl011@9000000", "interrupts", [0,1,4])]:
                observed = subprocess.check_output(["fdtget", "-t", "x", out / "running.dtb", path, prop])
                require([int(v,16) for v in observed.split()] == value, f"unexpected {path}/{prop}")
            events, samples = [], []
            for index in range(4):
                nonce = secrets.token_hex(32)
                begin = time.monotonic_ns()
                body = http(port, "/echo/" + nonce, out, f"challenge-{index}", deadline)
                events.append({"nonce": nonce, "begin_ns": begin, "end_ns": time.monotonic_ns()})
                require(body == (nonce + "\n").encode(), "incorrect HTTP challenge")
                lines = (out / "uart.log").read_text(errors="replace").splitlines()
                require(all(line in lines for line in guest_observations(operands, nonce, dtb)), "UART provenance")
                if index < 3:
                    while True:
                        observed = sample(qmp, out, layout, index)
                        if observed["count"] > (samples[-1]["count"] if samples else 0):
                            break
                        require(time.monotonic() < deadline, "JNOX EL0 wake timeout")
                        time.sleep(0.05)
                    samples.append(observed)
                    time.sleep(0.12)
            save(out / "challenges.json", events)
            for left, right in zip(samples, samples[1:]):
                require(all(0 < left[k] < right[k] for k in ("count", "task_ns", "timer_irqs", "last_irq_ticks")),
                        "JNOX EL0 task or interrupt journal did not advance between HTTP exchanges")
            require(samples[-1]["start_ns"] - samples[0]["end_ns"] > 150_000_000,
                    "insufficient interleaved observation interval")
            inspect(qmp, out, "end")
            qmp.call("quit")
            require(proc.wait(timeout=5) == 0, "QEMU exit")
            replay(out, args.native, args.linux)
            save(out / "result.json", {"passed": True, "pid": proc.pid, "samples": len(samples),
                                       "first_http_start_ns": events[0]["begin_ns"],
                                       "last_http_end_ns": events[-1]["end_ns"]})
        except Exception as exc:
            save(out / "failure.json", {"error": repr(exc), "returncode": proc.poll()})
            if negotiated and proc.poll() is None:
                inspect(qmp, out, "failure")
                if layout:
                    try:
                        sample(qmp, out, layout, "failure")
                    except Exception as native_exc:
                        save(out / "native-failure.json", {"error": repr(native_exc)})
            raise
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            guard.cancel()
            guard.join()
            uart_irq_report(out)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("native", "linux", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--bao", type=Path)
    parser.add_argument("--replay", action="store_true", help="check retained observations without launching QEMU")
    parser.add_argument("--loader-args", type=Path, help="JSON argv replacing the default -kernel bao.bin")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--trace-events", type=Path, help="list of built-in QEMU device trace events")
    parser.add_argument("--timeout", type=float, default=90)
    args = parser.parse_args()
    if args.replay:
        replay(args.out, args.native, args.linux)
    elif args.bao:
        run(args)
    else:
        parser.error("--bao is required for a live run")
