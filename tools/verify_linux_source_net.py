#!/usr/bin/env python3
"""Capture and replay one native Linux-source virtio-net control-plane trial."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile
import threading
import time

from native_amc_roots import linked_roots
from native_el0_replay import PA_MASK, permissions, vread
from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import capture_halt, journal_location, request
from verify_linux_source_rtc import RAM, load, require

NIC = 0x0a003e00
PIN = "05e676f7fc4f91783c961c860b930fae33462348064887eea95be3a440282a29"
PROBES = (("jnox_net_driver_result", 32), ("jnox_net_peer_result", 16))
TRACE = "memory_region_ops_read\nmemory_region_ops_write\nvirtio_mmio_read\nvirtio_mmio_write_offset\n"
NEG = 1 << 64


def argv(qemu, out, kernel, mtu, mac, callback=None):
    base = [qemu, *FLAGS, "-qmp", "stdio", "-serial", "file:"+str(out/"uart.log"), "-kernel", str(kernel),
            "-global", "virtio-mmio.force-legacy=false", "-netdev", "user,id=nic,restrict=on,ipv6=off",
            "-device", f"virtio-net-device,netdev=nic,mac={mac},host_mtu={mtu}"]
    if callback is None:
        return base + ["-trace", f"events={out/'net.events'},file={out/'mmio.trace'}"]
    return base + ["-d", "in_asm,exec,nochain", "-dfilter", callback["filter"], "-D", str(out/"callback.log"),
                   "-trace", f"events={out/'net.events'}"]


def decode(pkg, ram):
    manifest = load(pkg/"manifest.json")
    expected = dict(driver_slot=0, unauthorized_slots=[1], device="virtio-net", mmio_base=NIC,
                    mmio_bytes=512, driver_source="drivers/net/virtio_net.c", transport="virtio-net-mmio-modern",
                    qemu_device="virtio-net-device")
    require((manifest["board"], manifest["policy"], manifest["profile"]) == ("virt", "fifo", "native")
            and manifest["linux_source_net"] == expected, "native NIC profile")
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
        nic_page = int(permissions(ram, roots[i], (NIC & ~4095), "kernel rw", device=True), 16)
        require(nic_page & PA_MASK == (NIC & ~4095), "incorrect privileged NIC mapping")
        data.append(list(struct.unpack(f"<{words}Q", vread(ram, roots[i], va, size))))
    require(pages[0].isdisjoint(pages[1]), "aliased receipts")
    return manifest, data


def provenance(pkg, imp, check_source=False):
    from elftools.elf.elffile import ELFFile
    from elftools.elf.relocation import RelocationSection

    m = load(imp)
    origin = m["upstream"]
    require(m["mode"] == "service" and m["profile"] == "virtio-net" and
            origin["path"] == "drivers/net/virtio_net.c" and origin["tag"] == "v6.18.65" and
            origin["commit"] == "4700e3adf3b24b39dd4929e9c6d62811ba3ccea8" and
            m["source"]["sha256"] == origin["sha256"] == m["input_sha256"][origin["path"]] == PIN
            and m["source"]["size"] == origin["size"] == 196513, "original Linux translation unit")
    if check_source:
        require(sha(Path(m["source"]["path"])) == PIN, "original Linux source differs")
    callback = "virtnet_validate"
    blobs, patches = [], set()
    for field, name, binding in (("driver_object","virtio_net.original.o","STB_LOCAL"),
                                 ("callback_object","virtio_net.callback.o","STB_GLOBAL"),
                                 ("service","user.elf","STB_GLOBAL")):
        path = pkg/name if field == "service" else imp.parent/name
        require(sha(path) == m[field]["sha256"], field+" bytes")
        with path.open("rb") as f:
            elf = ELFFile(f)
            require(elf["e_machine"] == "EM_AARCH64" and elf.little_endian and elf.elfclass == 64,
                    "object architecture")
            table = elf.get_section_by_name(".symtab")
            hits = table.get_symbol_by_name(callback)
            require(len(hits or []) == 1 and hits[0]["st_info"]["bind"] == binding and
                    hits[0]["st_info"]["type"] == "STT_FUNC", field+" callback identity")
            sym = hits[0]
            idx = sym["st_shndx"]
            section = elf.get_section(idx)
            start = sym["st_value"] - section["sh_addr"]
            body = section.data()[start:start+sym["st_size"]]
            require(body and len(body) == m["callback"]["size"], "callback size")
            blobs.append(body)
            if field != "service":
                require(elf["e_type"] == "ET_REL" and section.name == ".text."+callback and
                        table.get_symbol_by_name("virtnet_probe"), "whole upstream driver object")
                if field == "driver_object":
                    for rel in elf.iter_sections():
                        if isinstance(rel, RelocationSection) and rel["sh_info"] == idx:
                            patches.update(r["r_offset"] // 4 for r in rel.iter_relocations())
            else:
                require(elf["e_type"] == "ET_EXEC" and elf.get_section_by_name(".note.jnox.abi")
                        and not any(s["p_type"] in ("PT_INTERP","PT_DYNAMIC") for s in elf.iter_segments()),
                        "static native ELF")
                caller = table.get_symbol_by_name("jnox_net_validate")[0]
                sc = elf.get_section(caller["st_shndx"])
                addr = caller["st_value"]
                off = addr-sc["sh_addr"]
                words = struct.iter_unpack("<I", sc.data()[off:off+caller["st_size"]])
                branches = []
                for i, (word,) in enumerate(words):
                    if word >> 26 == 0b100101:
                        disp = word & 0x3ffffff
                        branches.append(addr+4*i+4*(disp-(1<<26) if disp&(1<<25) else disp))
                require(sym["st_value"] in branches, "adapter does not call original callback")
    fingerprint = hashlib.sha256(blobs[0]).hexdigest()
    require(blobs[0] == blobs[1] and fingerprint == m["callback"]["original_section_sha256"] ==
            m["callback"]["promoted_section_sha256"], "callback promotion changed instructions")
    unchanged = [i for i in range(0,len(blobs[0]),4) if i//4 not in patches]
    require(unchanged and all(blobs[0][i:i+4] == blobs[2][i:i+4] for i in unchanged),
            "linked callback changed an instruction without a relocation")
    require(sha(imp.parent/"service.map") == m["link_map_sha256"], "link map identity")
    return dict(source_sha256=PIN, whole_object_sha256=m["driver_object"]["sha256"],
                callback_section_sha256=fingerprint, driver_sha256=m["service"]["sha256"],
                kernel_sha256=sha(pkg/"kernel.elf"), package_sha256=sha(pkg/"manifest.json"),
                import_sha256=sha(imp))


def callback_layout(pkg):
    from elftools.elf.elffile import ELFFile

    with (pkg/"user.elf").open("rb") as stream:
        elf = ELFFile(stream)
        symbol, = elf.get_section_by_name(".symtab").get_symbol_by_name("virtnet_validate")
        entry, size = symbol["st_value"], symbol["st_size"]
        section = elf.get_section(symbol["st_shndx"])
        pos = entry-section["sh_addr"]
        code = section.data()[pos:pos+size]
        require(len(code) == size and size >= 16 and not (entry | size) % 4, "invalid linked callback")
    peer_prefix = None
    with (pkg/"native_peer.elf").open("rb") as stream:
        for seg in ELFFile(stream).iter_segments():
            if seg["p_type"] != "PT_LOAD" or not seg["p_flags"] & 1:
                continue
            va = seg["p_vaddr"]
            if va & ~4095 <= entry < (va+seg["p_memsz"]+4095) & ~4095:
                off = entry-va
                require(off >= 0, "ambiguous peer executable page prefix")
                peer_prefix = seg.data()[off:off+16].ljust(16,b"\0").hex()
    return dict(entry=entry, size=size, filter=f"0x{entry:x}+0x{size:x}",
                sha256=hashlib.sha256(code).hexdigest(), first_bytes=code[:16].hex(),
                peer_entry_bytes=peer_prefix)


EXEC = re.compile(rb"^Trace 0: (0x[0-9a-f]+) \[([0-9a-f]+)/([0-9a-f]+)/([0-9a-f]+)/([0-9a-f]+)\]", re.I)
INSN = re.compile(rb"^\s*0x([0-9a-f]+):\s+([0-9a-f]{8})\s+", re.I)
MMIO = re.compile(rb"\b(?:memory_region_ops_(?:read|write)|virtio_mmio_(?:read|write_offset))\s")


STOP_BEFORE = re.compile(rb"Stopped execution of TB chain before (0x[0-9a-f]+) \[([0-9a-f]+)\]", re.I)


def callback_attempts(lines, entry):
    attempts, completed = [], []
    for i,line in enumerate(lines):
        row = EXEC.match(line)
        if not row or int(row[3],16) != entry:
            continue
        attempts.append((i,row.groups()))
        following = lines[i+1].rstrip() if i+1 < len(lines) else b""
        if following.startswith(b"Stopped execution of TB chain before"):
            stop = STOP_BEFORE.fullmatch(following)
            require(stop and int(stop[1],16) == int(row[1],16) and int(stop[2],16) == entry,
                    "stop-before marker differs from adjacent callback attempt")
        else:
            completed.append(i)
    return attempts, completed


def verify_execution(raw, layout):
    lines = raw.splitlines()
    entry = layout["entry"]
    translations = []
    attempts, completed = callback_attempts(lines, entry)
    for i,line in enumerate(lines):
        asm = INSN.match(line)
        if asm and int(asm[1],16) == entry:
            buf = bytearray()
            for insn in lines[i:i+4]:
                row = INSN.match(insn)
                if not row or int(row[1],16) != entry+len(buf):
                    break
                buf.extend(int(row[2],16).to_bytes(4,"little"))
            require(buf and layout["first_bytes"].startswith(buf.hex()), "translated callback entry differs from ELF")
            require(layout["peer_entry_bytes"] is None or not layout["peer_entry_bytes"].startswith(buf.hex()),
                    "translated entry also matches peer")
            translations.append(i)
    require(len(completed) == 3, "expected exactly three executed callback entries")
    require(len(translations) == 1 and translations[0] < attempts[0][0] and
            all(row == attempts[0][1] for _,row in attempts),
            "execution records do not identify a single translated entry block")
    # Translation of another TB at the same address cannot supply the first execution record.
    between = lines[translations[0]+1:attempts[0][0]]
    require(not any(EXEC.match(line) or line.startswith(b"IN:") for line in between),
            "translated entry is not followed by its executing block")
    return dict(callback_entries=len(completed), callback_stopped_before=len(attempts)-len(completed), callback_entry_va=hex(entry), callback_filter=layout["filter"])


def verify_device(out, d, p, mtu, mac):
    rawmac = bytes.fromhex(mac.replace(":",""))
    selected = (1 << 32) | 40
    require(d[:5] == [0x564e455444303031,100,0,0,3] and d[5]&40 == 40 and d[6]&1 and
            d[7:15] == [int.from_bytes(rawmac,"little"), mtu, selected, selected, 0, mtu, 1, 0] and
            d[15:31] == [NEG-22,0,1,NEG-22,0,1,NEG-22,3,0,11,0,NEG-19,0,3,0,NEG-19],
            "driver configuration or callback results")
    require(p[:9] == [0x564e455450303031,100,0]+[NEG-13]*5+[6] and
            all(a < b for a,b in zip(p[9:14],p[10:15])) and p[14]-p[9] >= 100_000_000,
            "peer authority or progress")
    accesses, model = [], []
    for line in (out/"mmio.trace").read_text().splitlines():
        phys = re.search(r"\bmemory_region_ops_(read|write) cpu (-?\d+) mr \S+ addr (0x[0-9a-fA-F]+) value (0x[0-9a-fA-F]+) size (\d+)", line)
        if phys:
            op, cpu, pa, value, width = phys.groups()
            offset = int(pa,16)-NIC
            if 0 <= offset < 512:
                require(cpu == "0", "NIC access outside CPU0")
                accesses.append((op, offset, int(value,16), int(width)))
            continue
        mmio = re.search(r"\bvirtio_mmio_(read|write_offset) virtio_mmio_(?:read|write) offset (0x[0-9a-f]+)(?: value (0x[0-9a-f]+))?$", line)
        require(mmio, "unrecognized MMIO trace record")
        op, off, val = mmio.groups()
        model.append(("read" if op == "read" else "write", int(off,16), int(val,16) if val else None))
    require(model == [(op, off, value if op == "write" else None) for op, off, value, width in accesses],
            "transport and physical MMIO events disagree")
    events = iter(accesses)
    def take(op, offset, value=None, width=4):
        got = next(events, None)
        require(got and got[:2] == (op,offset) and got[3] == width,
                f"expected {op} offset {offset:#x} width {width}, got {got}")
        raw = got[2]
        # QEMU traces the uint64_t callback result before the memory accessor
        # delivers the selected width. The feature bank is a 32-bit register.
        cpu_value = raw & 0xffffffff if (op, offset, width) == ("read", 16, 4) else raw
        require(value is None or cpu_value == value,
                f"expected {op} offset {offset:#x} value {value}, got raw {raw} CPU {cpu_value}")
        return cpu_value
    def claim():
        for off,v in ((0,0x74726976),(4,2),(8,1)):
            take("read",off,v)
        reset()
        take("write",112,1)
        take("write",112,3)
        take("read",112,3)
        take("read",112,3)
    def reset():
        take("write",112,0)
        take("read",112,0)
    def offered(bank):
        take("write",20,bank)
        take("read",16,d[5+bank])
    claim()
    offered(0)
    offered(1)
    for off,width,val in [*((256+i,1,b) for i,b in enumerate(rawmac)), (266,2,mtu), (266,2,mtu)]:
        offered(0)
        gen = take("read",252)
        take("read",off,val,width)
        take("read",252,gen)
    take("read",112,3)
    offered(0)
    offered(1)
    take("read",112,3)
    for bank,features in ((0,40),(1,1)):
        take("write",36,bank)
        take("write",32,features)
    take("write",112,11)
    take("read",112,11)
    take("read",112,11)
    reset()
    claim()
    reset()
    require(next(events,None) is None, "unexpected NIC access after release")
    return dict(mac=mac, mtu=mtu, offered_banks=d[5:7], negotiated=selected,
                callback_rc=[0,-22,-22], mmio_accesses=len(accesses), peer_wakes=p[8],
                raw_feature_reads=[value for op, off, value, width in accesses if (op,off,width) == ("read",16,4)])

def replay(pkg, imp, out, mtu=None, mac=None, require_exec=False):
    identities = provenance(pkg, imp)
    launch = load(out/"launch.json")
    schema = launch["schema"]
    callback = callback_layout(pkg) if schema == 2 else None
    require(schema in (1,2) and (schema == 2 or not require_exec) and
            (out/"callback.log").exists() == (schema == 2) and launch.get("callback") == callback,
            "capture execution schema or linked filter differs")
    require( launch["identities"] == identities and
            (mtu is None or mtu == launch["mtu"]) and (mac is None or mac == launch["mac"]),
            "launch operands or build identity")
    mtu, mac = launch["mtu"], launch["mac"]
    require(68 <= mtu <= 65535 and re.fullmatch(r"(?:[a-f0-9]{2}:){5}[a-f0-9]{2}",mac), "invalid configuration")
    command = launch["argv"]
    require(command == argv(command[0], Path(launch["capture_dir"]), Path(launch["kernel_path"]), mtu, mac, callback)
            and (out/"net.events").read_text() == TRACE, "unexpected QEMU machine or trace selection")
    qmp, halted = load(out/"qmp.json"), load(out/"halted.json")
    require(qmp["status"]["running"] is True and qmp["status"]["status"] == "running" and
            qmp["stop_return"] == {} and [c["cpu-index"] for c in qmp["cpus"]] == [0] and
            halted["status"] == "paused" and halted["running"] is False, "stopped CPU evidence")
    ram = RAM(load(out/"ram.json"))
    manifest, (driver, peer) = decode(pkg, ram)
    require(ram.used == set(ram.records), "unexpected retained RAM")
    uart = (out/"uart.log").read_bytes().splitlines()
    for slot, prefix in zip(manifest["slots"], (b"NATIVE EL0 ELF ", b"NATIVE EL0 PEER ELF ")):
        require(prefix+slot["elf_sha256"].encode() in uart, "booted identity")
    exits = {s for s in uart if s.startswith(b"NATIVE EL0 EXIT PID ")}
    require(exits == {b"NATIVE EL0 EXIT PID 0 0", b"NATIVE EL0 EXIT PID 1 0"} and
            not any(b"NATIVE EL0 FAULT" in s for s in uart), "native terminal receipt")
    verdict = {**identities, **verify_device(out, driver, peer, mtu, mac)}
    if callback:
        raw = (out/"callback.log").read_bytes()
        require(b"".join(line for line in raw.splitlines(keepends=True) if MMIO.search(line)) ==
                (out/"mmio.trace").read_bytes(), "raw combined QEMU MMIO trace differs")
        verdict.update(verify_execution(raw, callback))
    return verdict


def capture(pkg, imp, out, binary, timeout, mtu, mac):
    qemu = shutil.which(binary)
    require(qemu, "QEMU executable missing")
    identities = provenance(pkg, imp, check_source=True)
    out.mkdir(parents=True, exist_ok=False)
    (out/"net.events").write_text(TRACE)
    callback = callback_layout(pkg)
    cmd = argv(qemu, out, pkg/"kernel.elf", mtu, mac, callback)
    launch = dict(schema=2, callback=callback, mtu=mtu, mac=mac, argv=cmd, capture_dir=str(out), kernel_path=str(pkg/"kernel.elf"),
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
            raw = (out/"callback.log").read_bytes()
            (out/"mmio.trace").write_bytes(b"".join(line for line in raw.splitlines(keepends=True) if MMIO.search(line)))
            save(out/"result.json", replay(pkg, imp, out, mtu, mac, require_exec=True))
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            watchdog.cancel()
            watchdog.join()


def mutation_checks(pkg, imp, out):
    """Each temporary altered capture must fail on a named independent check."""
    replay(pkg, imp, out)
    passed = []
    with tempfile.TemporaryDirectory(prefix="jnox-net-negative-") as temp:
        copy = Path(temp)
        for name in ("launch.json", "qmp.json", "halted.json", "net.events", "ram.json", "uart.log", "mmio.trace"):
            shutil.copyfile(out/name, copy/name)

        if (out/"callback.log").exists():
            shutil.copyfile(out/"callback.log", copy/"callback.log")

        def reject(name, filename, changed, reason):
            path = copy/filename
            raw = path.read_bytes()
            try:
                path.write_bytes(changed)
                try:
                    replay(pkg, imp, copy)
                except ValueError as exc:
                    require(reason in str(exc), f"{name}: unrelated failure {exc}")
                else:
                    raise ValueError(f"{name}: corrupted evidence passed")
            finally:
                path.write_bytes(raw)
            passed.append(name)

        uart = (copy/"uart.log").read_bytes()
        reject("peer exit", "uart.log", uart.replace(b"EXIT PID 1 0", b"EXIT PID 1 9"), "terminal receipt")
        ram = load(copy/"ram.json")
        marker = struct.pack("<Q", 0x564e455450303031)
        for key, value in ram.items():
            raw = bytes.fromhex(value)
            if raw.startswith(marker) and len(raw) >= 9*8:
                raw = bytearray(raw)
                struct.pack_into("<Q", raw, 3*8, 0)
                ram[key] = raw.hex()
                break
        else:
            raise ValueError("missing retained peer receipt bytes for negative control")
        reject("peer authority", "ram.json", json.dumps(ram).encode(), "peer authority")
        lines = (copy/"mmio.trace").read_bytes().splitlines(keepends=True)
        for pos, line in enumerate(lines):
            if re.search(rb"memory_region_ops_read .*addr 0x0*a003f0a\b", line, re.I):
                changed = re.sub(rb"value 0x[0-9a-f]+", b"value 0x0", line, flags=re.I)
                reject("physical MTU", "mmio.trace", b"".join([*lines[:pos],changed,*lines[pos+1:]]),
                       "expected read offset")
                break
        else:
            raise ValueError("missing physical MTU read for negative control")
        pos = next(i for i,l in enumerate(lines) if b"virtio_mmio_write_offset" in l)
        reject("missing model write", "mmio.trace", b"".join([*lines[:pos],*lines[pos+1:]]),
               "transport and physical MMIO")
        if (copy/"callback.log").exists():
            layout = callback_layout(pkg)
            combined = (copy/"callback.log").read_bytes().splitlines(keepends=True)
            attempts, completed = callback_attempts(combined, layout["entry"])
            index = completed[-1]
            reject("callback execution entry", "callback.log", b"".join(combined[:index]+combined[index+1:]),
                   "three executed callback entries")
            stopped = next((i for i,_ in attempts if i not in completed), None)
            if stopped is None:
                # A fourth identical TB entry without a matching stop is an extra execution.
                extra = combined[:index]+[combined[index]]+combined[index:]
                reject("fourth execution without stop", "callback.log", b"".join(extra),
                       "three executed callback entries")
            else:
                reject("missing stop-before", "callback.log", b"".join(combined[:stopped+1]+combined[stopped+2:]),
                       "three executed callback entries")
                wrong = b"Stopped execution of TB chain before 0x0 [0000000000000000]\n"
                reject("wrong stop-before identity", "callback.log",
                       b"".join(combined[:stopped+1]+[wrong]+combined[stopped+2:]), "stop-before marker differs")
    return passed


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    for key in ("package", "driver-manifest", "out"):
        cli.add_argument("--"+key, type=Path, required=True)
    cli.add_argument("--qemu", default="qemu-system-aarch64")
    cli.add_argument("--timeout", type=float, default=25)
    cli.add_argument("--mtu", type=int)
    cli.add_argument("--mac", type=str.lower)
    cli.add_argument("--replay", action="store_true")
    cli.add_argument("--require-exec", action="store_true", help="require three original callback execution entries")
    cli.add_argument("--mutation-tests", action="store_true", help="exercise four negative replay controls on copies")
    cli.add_argument("--compare", type=Path, help="independently replay a second configuration with the same binaries")
    args = cli.parse_args()
    pkg, imp, out = (p.resolve() for p in (args.package, args.driver_manifest, args.out))
    if not args.replay:
        if args.mtu is None or args.mac is None or not 68 <= args.mtu <= 65535 or not re.fullmatch(
                r"(?:[0-9a-f]{2}:){5}[0-9a-f]{2}", args.mac):
            cli.error("live capture requires --mtu 68..65535 and --mac xx:xx:xx:xx:xx:xx")
        capture(pkg, imp, out, args.qemu, args.timeout, args.mtu, args.mac)
    result = replay(pkg, imp, out, args.mtu, args.mac, require_exec=args.require_exec)
    if args.compare:
        other = replay(pkg, imp, args.compare.resolve(), require_exec=args.require_exec)
        require(result["mtu"] != other["mtu"] and result["mac"] != other["mac"],
                "the two cold boots reused the same MTU or MAC")
        result = {"first":result, "second":other}
    if args.mutation_tests:
        result["rejected_mutations"] = mutation_checks(pkg, imp, out)
    print(json.dumps(result, indent=2))
