"""Capture and independently replay JNOX/Linux virtio_net gateway observations (#417)."""
import argparse
from functools import lru_cache
import hashlib
import ipaddress
import json
from pathlib import Path
import re
import struct
import tempfile

from native_amc_roots import linked_roots
from native_el0_replay import Snapshot, permissions, vread
from qemu_capture import journal_location


PROBES = (("jnox_gateway_result", 24), ("jnox_gateway_heartbeat", 244))
SLOTS = ("user.elf", "native_peer.elf", "native_peer2.elf")
PHASES = ("primary", "outage", "recovery", "linux_boot", "linux_ready", "final")
MEM_START, MEM_END = 0x40000000, 0x48000000
GUEST, HOST = (ipaddress.IPv4Address(ip).packed for ip in ("10.0.2.15", "10.0.2.2"))
BASELINE = Path(__file__).absolute().parents[1] / "docs/evidence/linux-virtio-net/result.json"


def require(ok, message):
    if not ok:
        raise ValueError(message)


def load(path):
    return json.loads(path.read_bytes())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


@lru_cache(maxsize=4)
def layout(pkg):
    """Derive each slot's own TTBR0 and linked probe instead of trusting captured addresses."""
    m = load(pkg / "manifest.json")
    require((m["profile"], m["board"], m["app"], m["policy"]) == ("native", "virt", "process", "fifo")
            and not m["tracked_changes"] and len(m["slots"]) == 3, "native profile or dirty build")
    require(m["socket_service"] == {"slot": 2, "authorized_clients": [0]} and
            m["net_service"]["slot"] == 2 and
            m.get("ipc_routes") == [{"caller": 0, "server": 2}], "gateway caller/peer authorities")
    kernel = pkg / "kernel.elf"
    raw = kernel.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == m["kernel_elf_sha256"], "native kernel identity")
    roots = linked_roots(kernel, 3)
    probes = []
    for slot, (name, spec) in enumerate(zip(SLOTS, m["slots"])):
        elf = pkg / name
        data = elf.read_bytes()
        require(spec["elf"] == name and spec["slot"] == slot and
                hashlib.sha256(data).hexdigest() == spec["elf_sha256"] and data in raw,
                f"slot {slot} image differs from booted kernel")
        if slot < len(PROBES):
            sym, words = PROBES[slot]
            va, size = journal_location(elf, sym)
            require(size == 8 * words and not (va & 7) and
                    0x400000 <= va < va + size <= 0x800000, f"unexpected {sym} extent")
            probes.append((roots[slot], va, size))
    require(len({s["elf_sha256"] for s in m["slots"]}) == 3, "expected distinct application binaries")
    return m, probes


class PhysicalReads(Snapshot):
    """Use the same page-table decoder for live and saved reads, including user permissions."""
    def __init__(self, records, reader=None):
        self.records = records
        self.reader = reader
        self.used = set()

    def read(self, pa, size):
        require(MEM_START <= pa < pa + size <= MEM_END and size <= 4096, "physical probe outside guest RAM")
        key = f"{pa:x}/{size}"
        if key not in self.records and self.reader is not None:
            self.records[key] = self.reader(pa, size).hex()
        value = bytes.fromhex(self.records[key])
        require(len(value) == size, "truncated physical probe")
        self.used.add(key)
        return value


def decode_probes(records, probes, reader=None):
    memory = PhysicalReads(records, reader)
    result, by_slot = [], []
    for root, va, size in probes:
        pages = set()
        for offset in range(va // 4096, (va + size - 1) // 4096 + 1):
            page = 4096 * offset
            desc = int(permissions(memory, root, page, "user rw"), 16)
            pages.add(desc & 0x0000fffffffff000)
        by_slot.append(pages)
        result.append(list(struct.unpack(f"<{size // 8}Q", vread(memory, root, va, size))))
    require(not by_slot[0].intersection(by_slot[1]) and set(records) == memory.used,
            "private receipt pages overlap or contain unconsumed memory")
    return result, [sorted(pages) for pages in by_slot]


def capture_receipts(qmp_request, package_dir, capture_root, tag):
    require(tag in PHASES, "unknown native receipt phase")
    _m, probes = layout(Path(package_dir))
    records = {}
    with tempfile.TemporaryDirectory(prefix="jnox-gw-") as tmp:
        dest = Path(tmp).resolve() / "read.bin"

        def read(pa, size):
            qmp_request("pmemsave", val=pa, size=size, filename=str(dest))
            return dest.read_bytes()

        decode_probes(records, probes, read)
    dst = Path(capture_root) / "native" / ("receipt-" + tag + ".json")
    with dst.open("x", encoding="utf8") as file:
        json.dump(records, file, sort_keys=True)


def packets(path):
    """Read the guest-facing Ethernet records produced by QEMU filter-dump."""
    data = path.read_bytes()
    endian = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">",
              b"\x4d\x3c\xb2\xa1": "<", b"\xa1\xb2\x3c\x4d": ">"}.get(data[:4])
    require(endian and len(data) >= 24 and struct.unpack_from(endian + "I", data, 20)[0] == 1,
            f"{path}: expected classic Ethernet pcap")
    offset = 24
    while offset < len(data):
        require(len(data) - offset >= 16, "truncated pcap record")
        cap, original = struct.unpack_from(endian + "II", data, offset + 8)
        offset += 16
        require(cap == original and offset + cap <= len(data), "incomplete wire packet")
        packet = data[offset:offset+cap]
        offset += cap
        # QEMU 6.2 can include the 10- or 12-byte virtio-net header in filter-dump.
        for lead in (0, 10, 12):
            start = lead + 14
            if (packet[lead+12:start] != b"\x08\x00" or len(packet) < start+20
                    or packet[start] >> 4 != 4):
                continue
            ip = packet[start:]
            hl, total = 4 * (ip[0] & 15), int.from_bytes(ip[2:4], "big")
            require(20 <= hl <= total <= len(ip), "invalid captured IPv4 size")
            if not (int.from_bytes(ip[6:8], "big") & 0x3fff):
                yield ip[12:16], ip[16:20], ip[9], ip[hl:total]
            break


def udp_records(path):
    records = []
    for src, dst, proto, segment in packets(path):
        if proto != 17 or len(segment) < 8:
            continue
        sport, dport, length = struct.unpack_from("!HHH", segment)
        endpoints = (src, dst, sport, dport)
        if endpoints in ((GUEST, HOST, 43003, 43004), (HOST, GUEST, 43004, 43003)):
            require(length == len(segment), "UDP datagram length")
            records.append((src == GUEST, segment[8:]))
    return records


def tcp_device_exchange(path, request, returned):
    """Reassemble both directions of the one device-peer connection, tolerating retransmission."""
    streams = {}
    for src, dst, proto, segment in packets(path):
        if proto != 6 or len(segment) < 20:
            continue
        sp, dp, seq = struct.unpack_from("!HHI", segment)
        outgoing = src == GUEST and sp == 8080
        incoming = dst == GUEST and dp == 8080
        if not (outgoing or incoming):
            continue
        head = 4 * (segment[12] >> 4)
        require(20 <= head <= len(segment), "invalid captured TCP header")
        peer = (dst, dp) if outgoing else (src, sp)
        if len(segment) > head:
            parts = streams.setdefault(peer, {True: {}, False: {}})[outgoing]
            start = (seq + bool(segment[13] & 2)) & 0xffffffff
            for off, byte in enumerate(segment[head:]):
                pos = (start + off) & 0xffffffff
                require(parts.setdefault(pos, byte) == byte, "conflicting TCP retransmission")
    require(len(streams) == 1, "expected one device-operation TCP connection carrying data")
    sides = next(iter(streams.values()))
    for direction, expected in ((True, request), (False, returned)):
        parts = sides[direction]
        starts = [pos for pos in parts if ((pos - 1) & 0xffffffff) not in parts]
        require(len(starts) == 1 and len(parts) == len(expected), "incomplete or extra device TCP data")
        require(bytes(parts[(starts[0]+n) & 0xffffffff] for n in range(len(parts))) == expected,
                "TCP device operation differs from gateway data")


def wire_record(kind, generation, req_id, timeout, payload):
    return struct.pack("!2sBBIII8s", b"JG", 1, kind, generation, req_id, timeout, payload)


def check_guest(root, m, probes):
    decoded = [decode_probes(load(root / "native" / ("receipt-"+tag+".json")), probes) for tag in PHASES]
    snaps = [readings for readings, _pages in decoded]
    require(all(pages == decoded[0][1] for _, pages in decoded),
            "JNOX private receipt backing changed across Linux restart")
    app, beat = snaps[-1]
    require(app[0:2] == [1, 5] and app[19] == 0, "app did not finish and close")
    require(app[13:19] == [1, 0, 2, 1, 1, 0], "stale/invalid/accept/timeout/output/fallback receipts")
    token = app[2].to_bytes(8, "big")
    reply = bytes(b ^ 0xa5 for b in token)
    require(app[5] == app[12] == int.from_bytes(reply, "big"), "app did not accept real device operations")
    first, first_end, begin, end, limit, fallback, restart, restored, stale = (
        app[n] for n in (3,4,6,7,8,9,10,11,20))
    require(0 < first < first_end <= begin < limit <= end <= fallback <= restart < stale < restored,
            "native monotonic time or stale-response order")
    require(limit - begin == 250_000_000 and fallback - begin <= 300_000_000, "late or premature fallback")
    require(first_end-first < 10_000_000_000 and restored-restart < 10_000_000_000,
            "accepted a device completion after its deadline")
    require(beat[0] == 1 and beat[1] == 240 and beat[2] == (1 << 64)-13 and beat[3] == 50_000_000,
            "independent task, period or unauthorized broker disposition")
    ticks = beat[4:4+beat[1]]
    require(all(a < b for a, b in zip(ticks, ticks[1:])), "peer clock regressed")
    outage_ticks = sum(begin < tick < limit for tick in ticks)
    restart_ticks = sum(stale < tick < restored for tick in ticks)
    require(outage_ticks > 0 and restart_ticks > 0, "peer did not run inside outage and recovery windows")
    counts = [snapshot[1][1] for snapshot in snaps]
    require(counts == sorted(counts), "live independent task did not retain monotonic progress")
    require([read[0][1] for read in snaps] == [1, 2, 3, 3, 3, 5] and
            [read[0][17] for read in snaps[1:]] == [1, 0, 0, 0, 1],
            "native app did not enter, hold and leave its software fallback state")
    boot_count, ready_count = counts[3:5]
    require(0 < boot_count < ready_count <= beat[1], "no heartbeat advance across observed Linux startup")
    for tag, live in zip(PHASES[3:5], snaps[3:5]):
        heartbeat = live[1]
        count = heartbeat[1]
        require(heartbeat[:1] == beat[:1] and heartbeat[2:4] == beat[2:4] and
                heartbeat[4:4+count] == ticks[:count], f"{tag}: independent heartbeat history diverged")
    raw = (root/"native"/"uart.log").read_bytes().splitlines()
    for prefix, slot in zip(("", "PEER ", "PEER2 "), m["slots"]):
        require(f"NATIVE EL0 {prefix}ELF {slot['elf_sha256']}".encode() in raw, "native boot identity")
    exits = [line for line in raw if line.startswith(b"NATIVE EL0 EXIT ")]
    require(exits == [f"NATIVE EL0 EXIT PID {i} {code}".encode() for i, code in enumerate((81,82,0))]
            and not any(b"NATIVE EL0 FAULT" in line for line in raw), "native terminal UART")
    return token, reply, {"deadline_ns": limit, "fallback_ns": fallback, "elapsed_to_fallback_ns": fallback-begin,
                          "lag_after_deadline_ns": fallback-limit, "peer_ticks_in_outage": outage_ticks,
                          "peer_ticks_during_recovery": restart_ticks, "stale_rejections": app[13],
                          "peer_samples": len(ticks), "peer_counts_at_live_reads": dict(zip(PHASES, counts)),
                          "peer_ticks_between_linux_startup_reads": ready_count-boot_count}


def check_linux(root, generation, request, returned, operands):
    where = root / f"linux-{generation}"
    launch = load(where / "launch.json")
    require(launch["image_sha256"] == operands["image_sha256"] and
            launch["initrd_sha256"] == operands["initrd_sha256"], "Linux launch identity changed")
    argv = launch["argv"]
    require(any(f"jnox.generation={generation}" in arg.split() for arg in argv) and
            any("restrict=on" in arg and "hostfwd=tcp:127.0.0.1:0-10.0.2.15:8080" in arg
                for arg in argv) and any("virtio-net-device" in arg for arg in argv), "Linux device configuration")
    lines = (where / "uart.log").read_bytes().splitlines()
    response = request[:3] + b"\x02" + request[4:16] + returned
    expect = [b"LD BOOT aarch64 6.18.65-0-virt"]
    for name in ("failover", "net_failover", "virtio_mmio", "virtio_net"):
        expect.append(f"LD LOADED {name} {operands['modules'][name]['ko_sha256']}".encode())
    expect.append(b"LD DEVICE /sys/bus/virtio/drivers/virtio_net /sys/module/virtio_net "
                  b"/sys/bus/platform/drivers/virtio-mmio 52:54:00:12:34:56")
    fdt = [line for line in lines if re.fullmatch(rb"LD FDT [0-9a-f]{64}", line)]
    require(len(fdt) == 1, "missing unique running Linux FDT measurement")
    expect += [fdt[0], f"GWREADY {generation}".encode(), b"GWACCEPT " + request.hex().encode(),
               b"GWREP " + response.hex().encode()]
    observed = [line for line in lines if line.startswith((b"LD BOOT ", b"LD LOADED ", b"LD DEVICE ",
                                                           b"LD FDT ", b"GW"))]
    require(observed == expect, f"Linux generation {generation}: unexpected console observations")
    require((where / "input.bin").read_bytes() == b"GWREQ " + request.hex().encode() + b"\n",
            "Linux serial-origin input differs")
    tcp_device_exchange(where / "wire.pcap", request, returned)
    return fdt[0][7:].decode()


def check_events(root, requests, replies, returned):
    rows = [json.loads(line) for line in (root/"events.jsonl").read_bytes().splitlines()]
    it = iter(rows)
    times = [row["t_ns"] for row in rows]
    require(all(isinstance(t, int) and t >= 0 for t in times) and
            all(a <= b for a, b in zip(times, times[1:])), "host monotonic transcript chronology")

    def take(name, **fields):
        item = next(it, {})
        require(item.get("event") == name and all(item.get(k) == v for k, v in fields.items()),
                f"missing or incorrect host {name} event")
        return item

    def operation(gen, req, rep):
        take("serial_request", generation=gen, hex=req.hex())
        take("nic_device_request", generation=gen, hex=req.hex())
        take("nic_device_reply", generation=gen, hex=returned.hex())
        take("serial_reply", generation=gen, hex=rep.hex())

    def pid(subdir):
        value = load(root / subdir / "qmp.json")["pid"]
        require(type(value) == int and value > 0, f"bad launch pid for {subdir}")
        return value

    linux1, linux2, native = [pid(name) for name in ("linux-1", "linux-2", "native")]
    require(len({linux1, linux2, native}) == 3, "expected distinct QEMU process identities")
    take("ready", generation=1, pid=linux1)
    take("jnox_request", generation=1, id=1, hex=requests[0].hex())
    operation(1, requests[0], replies[0])
    stopped = take("linux_stopped", generation=1, pid=linux1, returncode=0)
    take("jnox_reply", generation=1, delivery="current", hex=replies[0].hex())
    take("jnox_request", generation=1, id=2, hex=requests[1].hex())
    take("offline", generation=1, id=2, linux_pid=linux1, linux_returncode=0)
    take("jnox_request", generation=2, id=1, hex=requests[2].hex())
    take("jnox_reply", generation=1, delivery="stale", hex=replies[0].hex())
    def probe(tag):
        start = take("linux_boot_probe", generation=2, tag=tag, edge="start")["t_ns"]
        end = take("linux_boot_probe", generation=2, tag=tag, edge="end")["t_ns"]
        return [start, end]
    boot_probe = probe("linux_boot")
    ready = take("ready", generation=2, pid=linux2)
    ready_probe = probe("linux_ready")
    operation(2, requests[2], replies[1])
    take("jnox_reply", generation=2, delivery="current", hex=replies[1].hex())
    take("linux_stopped", generation=2, pid=linux2, returncode=0)
    take("native_stopped", pid=native, returncode=0)
    require(next(it, None) is None, "unexpected additional device lifecycle or gateway traffic")
    return {"host_ns_from_linux_exit_to_next_ready": ready["t_ns"] - stopped["t_ns"],
            "host_boot_probe_ns": boot_probe, "host_ready_observed_ns": ready["t_ns"],
            "host_ready_probe_ns": ready_probe}


def verify(capture_root, *, baseline=BASELINE):
    root = Path(capture_root)
    # Reopen package and symbols after boot even when the observer captured the live reads.
    layout.cache_clear()
    m, probes = layout(root/"package")
    require(digest(root/"package"/"build.log") == m["build_log_sha256"] and
            load(root/"native"/"launch.json")["kernel_sha256"] == m["kernel_elf_sha256"],
            "native launch/build metadata mismatch")
    token, returned, native = check_guest(root, m, probes)
    requests = [wire_record(1, gen, seq, budget, token)
                for gen, seq, budget in ((1, 1, 10000), (1, 2, 250), (2, 1, 10000))]
    replies = [wire_record(2, gen, 1, 10000, returned) for gen in (1, 2)]
    traffic = [(True, requests[0]), (False, replies[0]), (True, requests[1]), (True, requests[2]),
               (False, replies[0]), (False, replies[1])]
    require(udp_records(root/"native"/"wire.pcap") == traffic, "native NIC UDP trace differs")
    ops = load(root/"linux-operands.json")
    known = load(baseline)["operands"]
    for key in ("packages", "release", "modules", "image_sha256", "vmlinuz_sha256", "config_sha256",
                "busybox_sha256"):
        require(ops[key] == known[key], f"Linux operand changed from #416 baseline: {key}")
    fdts = [check_linux(root, gen, req, returned, ops) for gen, req in ((1, requests[0]), (2, requests[2]))]
    lifecycle = check_events(root, requests, replies, returned)
    return {"source_commit": m["commit"], "native_kernel_sha256": m["kernel_elf_sha256"],
            "linux_image_sha256": ops["image_sha256"], "linux_module_sha256": ops["modules"]["virtio_net"]["ko_sha256"],
            "linux_guest_fdt_sha256": fdts, "request_token_hex": token.hex(),
            "device_reply_hex": returned.hex(), **lifecycle,
            "native_observations": native}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--baseline", type=Path, default=BASELINE)
    args = parser.parse_args()
    print(json.dumps(verify(args.capture, baseline=args.baseline), indent=2))
