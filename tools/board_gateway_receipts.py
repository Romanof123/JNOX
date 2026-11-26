"""Replay raw Bao guest RAM reads and the Linux gateway's on-wire TCP exchanges."""
import struct
import time
from pathlib import Path

from linux_gateway_evidence import BASELINE, GUEST, digest, load, packets, require, wire_record
from native_amc_roots import linked_roots
from native_el0_replay import permissions, vread
from qemu_capture import journal_location
from verify_linux_virtio_net_baseline import guest_observations, save
from verify_single_board_boot import NativeRAM, RetainedRAM

STAGES = ("pending", "pending_later", "final")
PROBES = (("jnox_gateway_result", 24), ("jnox_gateway_heartbeat", 136))
PRIV = "BG PRIV uid=65534 gid=65534 caps=0 nnp=1 devmem=denied map=0x0b000000+4096"


def native_layout(package):
    pkg = Path(package)
    manifest = load(pkg / "manifest.json")
    kernel = pkg / "kernel.elf"
    kernel_data = kernel.read_bytes()
    require(digest(kernel) == manifest["kernel_elf_sha256"], "kernel digest")
    slots = manifest["slots"]
    require(len(slots) == 2, "expected application and independent peer")
    roots = linked_roots(kernel, 2)
    probes = []
    for index, (slot, root, (name, words)) in enumerate(zip(slots, roots, PROBES)):
        require(slot["slot"] == index and Path(slot["elf"]).name == slot["elf"], "slot identity")
        elf = pkg / slot["elf"]
        require(digest(elf) == slot["elf_sha256"] and elf.read_bytes() in kernel_data, "linked slot image")
        va, size = journal_location(elf, name)
        require(size == 8 * words and not va % 8 and 0x400000 <= va < va + size <= 0x800000,
                "linked private result extent")
        probes.append((root, va, size))
    symbols = {}
    for key, length in (("boot_len", 8), ("boot_log", 4096), ("timer_irqs", 8), ("last_irq_ticks", 8)):
        addr, size = journal_location(kernel, "jnox_partition_" + key)
        require(size == length, key + " size")
        symbols[key] = addr
    symbols["journal"], size = journal_location(kernel, "jnox_native_el0_journal")
    require(size >= 128, "EL0 journal header extent")
    return probes, symbols, [s["elf_sha256"] for s in slots]


def decode(mem, layout):
    probes, symbols, identities = layout

    def word(name):
        return int.from_bytes(mem.read(symbols[name], 8), "little")

    length = word("boot_len")
    require(0 < length <= 4096, "boot log extent")
    boot = mem.read(symbols["boot_log"], length)
    for prefix, identity in zip(("NATIVE EL0 ELF ", "NATIVE EL0 PEER ELF "), identities):
        require((prefix + identity).encode() in boot.splitlines(), "booted native image")
    require(b"JNOX BAO PARTITION BOOT" in boot.splitlines() and b"NATIVE EL0 FAULT" not in boot,
            "native boot or fault")
    header = struct.unpack("<16Q", mem.read(symbols["journal"], 128))
    require(header[0] == 1 and header[1] == 2 and header[2] == 0 and header[12] == 4
            and header[14] == probes[0][0], "native execution journal")
    result = dict(timer_irqs=word("timer_irqs"), last_irq_ticks=word("last_irq_ticks"),
                  exit_status=header[9], pages=[])
    for label, (root, va, size) in zip(("app", "peer"), probes):
        pages = [int(permissions(mem, root, vpn * 4096, "user rw"), 16) & 0x0000fffffffff000
                 for vpn in range(va // 4096, (va + size - 1) // 4096 + 1)]
        result["pages"].append(pages)
        result[label] = list(struct.unpack(f"<{size // 8}Q", vread(mem, root, va, size)))
    require(set(result["pages"][0]).isdisjoint(result["pages"][1]), "shared private receipt pages")
    return result


def sample(qmp, out, layout, stage):
    require(stage in (*STAGES, "failure"), "unknown observation stage")
    mem = NativeRAM(qmp, out)
    start = time.monotonic_ns()
    try:
        decoded = decode(mem, layout)
    except Exception:
        save(out / f"native-{stage}-reads.json", mem.records)
        raise
    data = dict(decoded, start_ns=start, end_ns=time.monotonic_ns(), reads=mem.records)
    save(out / f"native-{stage}.json", data)
    return data


def wire_flows(path):
    flows = {}
    for src, dst, proto, seg in packets(path):
        if proto != 6 or len(seg) < 20:
            continue
        sp, dp, seq = struct.unpack_from("!HHI", seg)
        outgoing = src == GUEST and sp == 8080
        if not outgoing and not (dst == GUEST and dp == 8080):
            continue
        header = (seg[12] >> 4) * 4
        require(20 <= header <= len(seg), "invalid TCP header")
        if header == len(seg):
            continue
        endpoint = (dst, dp) if outgoing else (src, sp)
        parts = flows.setdefault(endpoint, ({}, {}))[0 if outgoing else 1]
        for delta, value in enumerate(seg[header:]):
            pos = (seq + int(bool(seg[13] & 2)) + delta) & 0xffffffff
            require(parts.setdefault(pos, value) == value, "conflicting TCP retransmission")

    def assemble(parts):
        starts = [i for i in parts if ((i - 1) & 0xffffffff) not in parts]
        require(len(starts) == 1, "incomplete TCP stream")
        return bytes(parts[(starts[0] + i) & 0xffffffff] for i in range(len(parts)))

    return [(assemble(tx), assemble(rx)) for tx, rx in flows.values()]


def replay(out, native, linux):
    out, linux = Path(out), Path(linux)
    layout = native_layout(native)
    stages = []
    for stage in STAGES:
        data = load(out / f"native-{stage}.json")
        mem = RetainedRAM(data["reads"])
        decoded = decode(mem, layout)
        require(next(mem.records, None) is None and all(data[k] == v for k, v in decoded.items()),
                "saved native sample differs from RAM")
        require(0 < data["start_ns"] <= data["end_ns"], "invalid observation interval")
        stages.append(data)

    first, second, final = stages
    app, peer = final["app"], final["peer"]
    records = load(out / "peer.json")
    require(len(records) == 2, "expected two external exchanges")
    require(0 < records[0]["begin_ns"] <= first["start_ns"] <= first["end_ns"] < second["start_ns"]
            <= second["end_ns"] <= records[0]["end_ns"] < records[1]["begin_ns"]
            < records[1]["end_ns"] <= final["start_ns"], "outstanding observation chronology")
    require(all(s["pages"] == first["pages"] for s in stages), "native backing changed")
    require(all(s["app"][0:2] == [1, 1] and s["app"][15] == 0 for s in stages[:2]),
            "application already completed first request")
    for s in stages:
        require(s["peer"][0:4] == [1, 2, (1 << 64)-13, (1 << 64)-13] and s["peer"][135] == 0,
                "independent peer access denial or runtime failure")
    require(first["peer"][4] < second["peer"][4] and first["peer"][134] < second["peer"][134]
            and all(0 < first[key] < second[key] for key in ("timer_irqs", "last_irq_ticks")),
            "no independent progress while exchange outstanding")
    require(app[:2] == [1, 5] and app[14:20] == [0, 2, 1, 1, 0, 0]
            and app[21] == (1 << 64)-14 and app[22] in ((1 << 64)-14, (1 << 64)-11)
            and final["exit_status"] == 81, "final app disposition")
    start, end, limit, fallback = app[6:10]
    require(0 < app[3] < app[4] <= start < limit <= end <= fallback <= app[10] < app[11]
            and limit-start == 250_000_000 and fallback-start <= 300_000_000
            and app[4]-app[3] < 10_000_000_000 and app[11]-app[10] < 10_000_000_000,
            "guest deadline or fallback timing")
    require((app[13] == app[20] == 0) or (app[13] > 0 and app[10] <= app[20] < app[11]),
            "stale response disposition")
    ticks = [peer[6 + i % 128] for i in range(max(0, peer[4]-128), peer[4])]
    require(app[3] < second["peer"][134] < app[4], "peer wake outside first guest exchange")
    require(all(a < b for a, b in zip(ticks, ticks[1:])) and any(start < t < limit for t in ticks),
            "missing independent wake inside outage")
    token = app[2].to_bytes(8, "big")
    reqs = [wire_record(1, gen, idx, ms, token) for gen, idx, ms in ((1,1,10000),(1,2,250),(2,1,10000))]
    returns = [app[i].to_bytes(8, "big") for i in (5,12)]
    require(len({token, *returns}) == 3 and all(any(r) for r in returns), "reused request or response")
    accepted = list(zip((reqs[0], reqs[2]), returns))
    for record, (request, response) in zip(records, accepted):
        require(record["request"] == request.hex() and record["response"] == response.hex(),
                "peer receipt differs from EL0 result")
    require(wire_flows(out / "wire.pcap") == accepted, "captured TCP payloads differ from EL0 and peer")
    lines = (out / "uart.log").read_text(errors="replace").splitlines()
    stock = load(BASELINE)["operands"]
    package_ops = load(linux / "linux.json")["operands"]
    for key in ("release", "image_sha256", "modules"):
        require(package_ops[key] == stock[key], "baseline mismatch " + key)
    dtb = (linux / "guest.dtb").read_bytes()
    for line in guest_observations(stock, "", dtb):
        if not line.startswith("LD RECEIVED"):
            require(lines.count(line) == 1, "Linux device identity " + line)
    expected = [PRIV, "BG READY"]
    for i, request in enumerate(reqs):
        expected.append("BG REQ " + request.hex())
        if i == 1:
            if "BG TIMEOUT 1 2" in lines:
                expected.append("BG TIMEOUT 1 2")
        else:
            reply = wire_record(2, 1 if i == 0 else 2, 1, 10000, returns[i // 2])
            expected.append("BG REP " + reply.hex())
    require([line for line in lines if line.startswith("BG ")] == expected, "Linux gateway UART transcript")
    return dict(outage_ns=fallback-start, peer_ticks_in_outage=sum(start < t < limit for t in ticks),
                peer_counts=[s["peer"][4] for s in stages], device_replies=[r.hex() for r in returns],
                linux_module_sha256=stock["modules"]["virtio_net"]["ko_sha256"])
