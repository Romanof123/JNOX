#!/usr/bin/env python3
"""Observe a native JNOX PL031 driver in virt with independent device-model tracing."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import threading
import time

from native_amc_roots import linked_roots
from native_el0_replay import PA_MASK, Snapshot, permissions, vread
from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import capture_halt, journal_location, request

BASE, END = 0x40000000, 0x48000000
RTC_BASE = "2024-01-01T00:00:00"
SEED = int(datetime.fromisoformat(RTC_BASE).replace(tzinfo=timezone.utc).timestamp())
PROBES = (("jnox_rtc_driver_result", 32), ("jnox_rtc_peer_result", 16))


def require(ok, message):
    if not ok:
        raise ValueError(message)


def load(path):
    return json.loads(path.read_bytes())


TRACE_SELECTION = "pl031_*\ngic_acknowledge_irq\n"


def argv(qemu, out, kernel, schema=2):
    selection = "events=" + str(out/"rtc-gic.events") if schema == 2 else "enable=pl031_*"
    return [qemu, *FLAGS, "-rtc", "base=" + RTC_BASE + ",clock=vm", "-qmp", "stdio",
            "-serial", "file:" + str(out/"uart.log"), "-kernel", str(kernel),
            "-trace", selection + ",file=" + str(out/"pl031.trace")]


class RAM(Snapshot):
    """Every captured/read-back byte is ordinary RAM, including the stage-1 walk."""
    def __init__(self, records, reader=None):
        self.records, self.reader, self.used = records, reader, set()

    def read(self, pa, n):
        require(BASE <= pa < pa+n <= END and n <= 4096, "physical read outside native RAM")
        key = f"{pa:x}/{n}"
        if key not in self.records and self.reader:
            self.records[key] = self.reader(pa, n).hex()
        raw = bytes.fromhex(self.records[key])
        require(len(raw) == n, "truncated retained RAM")
        self.used.add(key)
        return raw


def decode(pkg, ram):
    manifest = load(pkg/"manifest.json")
    grant = manifest["linux_source_rtc"]
    require((manifest["board"], manifest["policy"], manifest["profile"]) == ("virt", "fifo", "native")
            and grant == dict(driver_slot=0, unauthorized_slots=[1], device="arm,pl031",
                              mmio_base=0x09010000, mmio_bytes=4096, intid=34,
                              driver_source="drivers/rtc/rtc-pl031.c"), "native RTC profile")
    kernel = pkg/"kernel.elf"
    require(sha(kernel) == manifest["kernel_elf_sha256"] and len(manifest["slots"]) == 2,
            "kernel/package mismatch")
    kernel_raw = kernel.read_bytes()
    roots = linked_roots(kernel, 2)
    data, pages = [], []
    for i, (name, words) in enumerate(PROBES):
        spec = manifest["slots"][i]
        require(spec["slot"] == i and spec["elf"] == ("user.elf","native_peer.elf")[i], "slot layout")
        elf = pkg/spec["elf"]
        require(sha(elf) == spec["elf_sha256"] and elf.read_bytes() in kernel_raw, "EL0 image identity")
        va, size = journal_location(elf, name)
        require(size == words*8 and va % 8 == 0 and 0x400000 <= va < va+size <= 0x800000,
                "invalid EL0 receipt extent")
        pages.append({int(permissions(ram, roots[i], page*4096, "user rw"), 16) & PA_MASK
                      for page in range(va//4096, (va+size-1)//4096+1)})
        rtc_page = int(permissions(ram, roots[i], 0x09010000, "kernel rw", device=True), 16)
        require(rtc_page & PA_MASK == 0x09010000, "incorrect privileged RTC mapping")
        data.append(list(struct.unpack(f"<{words}Q", vread(ram, roots[i], va, size))))
    require(pages[0].isdisjoint(pages[1]), "aliased receipts")
    return manifest, data


def parse_trace(path, schema=1):
    events = []
    for line in path.read_text().splitlines():
        acknowledge = re.search(r"\bgic_acknowledge_irq (cpu|vcpu) (\d+) acknowledged irq (\d+)$", line)
        if acknowledge:
            require(schema == 2 and acknowledge[1] == "cpu" and int(acknowledge[2]) == 0,
                    "unexpected GIC witness or CPU")
            events.append(("gic_ack", int(acknowledge[3])))
            continue
        match = re.search(r"\bpl031_(\w+)\s+(.*)$", line)
        require(match, "unrecognized QEMU PL031 trace record")
        name, args = match.groups()
        if name in ("read", "write"):
            m = re.fullmatch(r"addr (0x[0-9a-fA-F]+) value (0x[0-9a-fA-F]+)", args)
            require(m, "malformed traced register access")
            events.append((name, int(m[1],16), int(m[2],16)))
        elif name == "irq_state":
            m = re.fullmatch(r"irq state ([01])", args)
            require(m, "malformed RTC IRQ state")
            events.append((name, int(m[1])))
        elif name == "alarm_raised":
            require(args == "alarm raised", "malformed alarm trace")
            events.append((name,))
        elif name == "set_alarm":
            m = re.fullmatch(r"alarm set for (\d+) ticks", args)
            require(m, "malformed match trace")
            events.append((name, int(m[1])))
        else:
            raise ValueError("unexpected device trace event " + name)
    return events


def verify_device(trace, d, p, schema=1):
    require(d[:3] == [0x504c303331443031,100,0] and p[:3] == [0x504c303331503031,100,0],
            "terminal receipts")
    require(SEED <= d[3] <= SEED+10 and d[4] <= d[5] <= d[4]+1 and d[6] == d[7] == d[31] == d[4]+2
            and d[8:13] == [1,0,1,1,0] and d[13] >= d[6] and d[14:21] == [0,2,2,1,1,0xa0,0]
            and d[21] >= d[13] and d[22:25] == [0,0x141031,0xb105f00d] and 0 < d[25] < d[26] <= d[27]
            and d[28] == d[30] > 0 and 0 < d[29] <= 60000, "driver lifecycle receipts")
    require(p[3:8] == [(1<<64)-13]*4 + [8] and all(a < b for a,b in zip(p[8:],p[9:]))
            and p[15]-p[8] >= 3_500_000_000 and any(d[25] < stamp < d[26] for stamp in p[8:]),
            "denied peer or independent progress")
    idx = 0

    def take(event):
        nonlocal idx
        try:
            idx = trace.index(event, idx)+1
        except ValueError as exc:
            raise ValueError(f"missing ordered device trace {event} after {idx}") from exc

    def primecell():
        for j, byte in enumerate(bytes.fromhex("311014000df005b1")):
            take(("read", 0xfe0 + 4*j, byte))

    primecell()
    take(("read", 0, d[3]))
    take(("write", 8, d[4]))
    take(("read", 0, d[5]))
    take(("write", 4, d[6]))
    take(("write", 16, 1))
    take(("read", 4, d[7]))
    take(("alarm_raised",))
    take(("irq_state", 1))
    if schema == 2:
        take(("gic_ack", 34))
    take(("read", 24, 1))
    take(("write", 28, 1))
    take(("irq_state", 0))
    take(("read", 0, d[13]))
    take(("write", 16, 0))
    primecell()
    take(("read", 0, d[21]))
    require([e for e in trace if e[:2] == ("write", 8)] == [("write", 8, d[4])], "extra RTC load")
    return {"seed_read": d[3], "set_readback": d[5], "alarm_match": d[6], "callback_flags": d[19],
            "rebound_time": d[21], "peer_wakes": p[7], "trace_events": len(trace)}


def provenance(pkg, imp, original=None):
    meta = load(imp)
    expected = "3178d83f57c426bccb1a96cdceb3298b34651cc04c58b53c07a5dfc69bc94f76"
    blob_id = "eab39dfa4e5fe5d046b32de9d08c8da9c47e4040"
    source = meta["source"]
    require(meta["profile"] == "rtc-pl031" and meta["upstream"]["path"] == "drivers/rtc/rtc-pl031.c"
            and meta["upstream"]["tag"] == "v6.18" and source["translation_unit"] == "rtc-pl031.c"
            and meta["upstream"]["sha256"] == source["sha256"] == expected
            and meta["upstream"]["git_blob_sha1"] == source["git_blob_sha1"] == blob_id
            and source["size"] == meta["upstream"]["size"] == 12342, "original Linux translation unit")
    if original:
        require(sha(original) == expected and original.stat().st_size == 12342, "original source bytes")
    obj, elf = imp.parent/"rtc-pl031.o", pkg/"user.elf"
    require(sha(obj) == meta["driver_object"]["sha256"] and sha(elf) == meta["service"]["sha256"],
            "original object or linked native service identity")
    funcs = ("pl031_probe","pl031_interrupt","pl031_read_time","pl031_remove","pl031_set_time",
             "pl031_read_alarm","pl031_set_alarm","pl031_alarm_irq_enable")
    for artifact in (obj, elf):
        table = subprocess.check_output(["aarch64-linux-gnu-nm", "-S", "--defined-only", artifact], text=True)
        for name in funcs:
            require(re.search(r"(?m)^[a-fA-F0-9]+ [a-fA-F0-9]+ [tT] " + name + "$", table),
                    "missing linked function " + name)
    cmds = meta["commands"]
    require("-c" in cmds[0] and cmds[0][cmds[0].index("-c")+1] == source["translation_unit"]
            and cmds[0][-2:] == ["-o", obj.name] and obj.name in cmds[-1]
            and cmds[-1][-2:] == ["-o", "service.elf"], "Linux translation unit build commands")
    return dict(linux_source_sha256=expected, driver_object_sha256=sha(obj), user_sha256=sha(elf),
                kernel_sha256=sha(pkg/"kernel.elf"), package_sha256=sha(pkg/"manifest.json"),
                import_sha256=sha(imp))


def replay(pkg, imp, out, require_gic=False):
    identities = provenance(pkg, imp)
    launch = load(out/"launch.json")
    require(launch["identities"] == identities, "capture identities differ")
    command = launch["argv"]
    original_dir = Path(launch["capture_dir"])
    schema = launch.get("trace_schema", 1)
    selection = out/"rtc-gic.events"
    require(schema in (1, 2) and (schema == 2 or not require_gic), "GIC evidence schema required")
    require(selection.exists() == (schema == 2), "trace selection conflicts with capture schema")
    if schema == 2:
        require(selection.read_text() == TRACE_SELECTION, "device/GIC event selection differs")
    require(command == argv(command[0], original_dir, Path(launch["kernel_path"]), schema), "unexpected machine launch")
    qmp, halt = load(out/"qmp.json"), load(out/"halted.json")
    require(qmp["status"]["running"] is True and qmp["status"]["status"] == "running" and
            qmp["stop_return"] == {} and [c["cpu-index"] for c in qmp["cpus"]] == [0] and
            halt["running"] is False and halt["status"] == "paused", "stopped single CPU evidence")
    ram = RAM(load(out/"ram.json"))
    manifest, (driver, peer) = decode(pkg, ram)
    require(ram.used == set(ram.records), "unexpected retained physical read")
    uart = (out/"uart.log").read_bytes().splitlines()
    prefixes = (b"NATIVE EL0 ELF ", b"NATIVE EL0 PEER ELF ")
    for slot, prefix in zip(manifest["slots"], prefixes):
        require(prefix+slot["elf_sha256"].encode() in uart, "booted ELF digest")
    exits = [l for l in uart if l.startswith(b"NATIVE EL0 EXIT PID ")]
    require(set(exits) == {b"NATIVE EL0 EXIT PID 0 0", b"NATIVE EL0 EXIT PID 1 0"} and
            not any(b"NATIVE EL0 FAULT" in l for l in uart), "native exit or fault")
    operands = manifest["slots"][0].get("argv") or []
    target = int(operands[1]) if len(operands) >= 2 else 1961711998
    timeout = int(operands[2]) if len(operands) >= 3 else 5000
    require((driver[4], driver[29]) == (target, timeout), "packaged driver operands")
    trace = parse_trace(out/"pl031.trace", schema)
    results = verify_device(trace, driver, peer, schema)
    if schema == 2:
        results.update(trace_schema=schema, gic_cpu=0, gic_intid=34,
                       gic_intid34_acknowledgements=trace.count(("gic_ack", 34)))
    return dict(**identities, **results)


def capture(pkg, imp, out, binary, timeout):
    qemu = shutil.which(binary)
    require(qemu, "QEMU executable missing")
    identities = provenance(pkg, imp, Path(load(imp)["source"]["path"]))
    out.mkdir(parents=True, exist_ok=False)
    (out/"rtc-gic.events").write_text(TRACE_SELECTION)
    cmd = argv(qemu, out, pkg/"kernel.elf")
    launch = dict(trace_schema=2, argv=cmd, capture_dir=str(out), kernel_path=str(pkg/"kernel.elf"),
                  identities=identities, qemu_sha256=sha(Path(qemu)),
                  qemu_version=subprocess.check_output([qemu, "--version"], text=True).splitlines()[0])
    with (out/"stderr.log").open("xb") as err, subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, bufsize=0) as proc:
        save(out/"launch.json", dict(launch, pid=proc.pid))
        watchdog = threading.Timer(timeout + 5, proc.kill)
        watchdog.start()
        try:
            class Pipe:
                write = proc.stdin.write
                readline = proc.stdout.readline
            stream = Pipe()
            require("QMP" in json.loads(stream.readline()), "missing QMP greeting")
            request(stream, "qmp_capabilities")
            until = time.monotonic()+timeout
            while time.monotonic() < until and proc.poll() is None:
                uart = out/"uart.log"
                raw = uart.read_bytes() if uart.exists() else b""
                if b"NATIVE EL0 FAULT" in raw or len(set(re.findall(
                        rb"(?m)^NATIVE EL0 EXIT PID ([01]) -?\d+\r?$", raw))) == 2:
                    break
                time.sleep(.02)
            capture_halt(stream, out)
            dest = out/"sample.bin"
            def read(pa, size):
                request(stream, "pmemsave", val=pa, size=size, filename=str(dest))
                return dest.read_bytes()
            ram = RAM({}, read)
            try:
                decode(pkg, ram)
            finally:
                save(out/"ram.json", ram.records)
                dest.unlink(missing_ok=True)
            request(stream, "quit")
            require(proc.wait(timeout=5) == 0, "QEMU exit")
            save(out/"result.json", replay(pkg, imp, out, require_gic=True))
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            watchdog.cancel()
            watchdog.join()


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    for name in ("package", "driver-manifest", "out"):
        cli.add_argument("--"+name, type=Path, required=True)
    cli.add_argument("--qemu", default="qemu-system-aarch64")
    cli.add_argument("--timeout", type=float, default=25)
    cli.add_argument("--replay", action="store_true")
    cli.add_argument("--require-gic", action="store_true", help="reject legacy PL031-only captures")
    args = cli.parse_args()
    paths = [p.resolve() for p in (args.package, args.driver_manifest, args.out)]
    if args.replay:
        print(json.dumps(replay(*paths, require_gic=args.require_gic), indent=2))
    else:
        capture(*paths, args.qemu, args.timeout)
