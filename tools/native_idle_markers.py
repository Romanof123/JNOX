#!/usr/bin/env python3
"""Resolve the four #340 wake-image instruction sites and join their host observations to guest UART."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import sys

from elftools.elf.elffile import ELFFile
from read_hierarchy_health import read_uart

ELF_HASH = "3e5947d2e194e2aab8c83cbe5fdbbb675c2a0b4ab55fb6d484f7f66cc71da0a9"
APP_HASHES = ("1f8029762aa1014fd0558210909818cbcc88f5a32702f6d813881c491a94803c",
              "bc6b12de55a0fa802ed43a3433e3f1e6d19ceb5919add115054edddcbf85aa8f",
              "8515d699100270180839d94afaf74bd7854e022344a29ca4532bbfd4d2fdb43a")
HEADER = struct.Struct("<39Q")
RECORD = struct.Struct("<7Q")
CAP = 8192


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def sites(kernel):
    blob = kernel.read_bytes()
    require(hashlib.sha256(blob).hexdigest() == ELF_HASH, "wrong #340 wake kernel SHA-256")
    elf = ELFFile(io.BytesIO(blob))
    sym = elf.get_section_by_name(".symtab")

    def address(name):
        symbols = sym.get_symbol_by_name(name)
        require(symbols and len(symbols) == 1, f"missing/ambiguous {name}")
        return symbols[0]["st_value"]

    def word(pc):
        for seg in elf.iter_segments():
            start = seg["p_vaddr"]
            if seg["p_type"] == "PT_LOAD" and seg["p_flags"] & 1 and start <= pc <= start + seg["p_filesz"] - 4:
                return struct.unpack_from("<I", blob, seg["p_offset"] + pc - start)[0]
        raise ValueError(f"site outside linked executable bytes: {pc:#x}")

    def branch(pc, opcode):
        insn = word(pc)
        require(insn >> 26 == opcode, f"incorrect branch at {pc:#x}")
        disp = insn & ((1 << 26) - 1)
        return pc + 4 * (disp - (1 << 26) if disp & (1 << 25) else disp)

    idle = address("native_sleep_idle")
    irq = branch(address("native_el0_vectors") + 5 * 128, 5)
    require([word(idle+x) for x in (-20,-16,-12,-8,-4,0)] ==
            [0xd5033fdf,0xd53be021,0xa9000801,0xd50342ff,0xd5033fdf,0xd503207f],
            "idle ISB/MRS/store/unmask/ISB/WFI mismatch")
    require([word(irq+x) for x in (0,4,8)] == [0xaa1f03e0,0xd5033fdf,0xd53be021]
            and branch(irq+12,37) == address("native_sleep_irq"), "idle vector or call target mismatch")
    return idle, [(pc, word(pc)) for pc in (idle-16,idle-12,irq+8,irq+12)]


def correlate(args, idle, addresses):
    raw = args.markers.read_bytes()
    require(len(raw) == HEADER.size + CAP * RECORD.size, "incomplete marker file")
    magic, version, count, dropped, invalid, wall_res, cpu_res, *cal = HEADER.unpack_from(raw)
    require((magic,version) == (0x393333584f4e4a,1) and args.tid > 0 and count <= CAP, "invalid marker header")
    samples = list(RECORD.iter_unpack(raw[HEADER.size:HEADER.size + count*RECORD.size]))
    report = dict(kernel_sha256=ELF_HASH, sites=addresses, marker_sha256=hashlib.sha256(raw).hexdigest(),
                  count=count, dropped=dropped, invalid=invalid, monotonic_resolution_ns=wall_res,
                  thread_cpu_resolution_ns=cpu_res, installation_thread_empty_brackets_ns=cal,
                  anchors=[], limitations=[])
    if invalid or dropped or not wall_res or not cpu_res:
        report["limitations"].append("marker flags, loss or unavailable clock resolution")
    uart = args.uart.read_bytes()
    report["uart_sha256"] = hashlib.sha256(uart).hexdigest()
    lines = uart.splitlines()
    for prefix, digest in zip(("","PEER ","PEER2 "), APP_HASHES):
        require(lines.count(f"NATIVE EL0 {prefix}ELF {digest}".encode()) == 1, "wrong guest ELF identities")
    health = read_uart(args.uart)
    report["health"] = health["health"]
    if health["detail"] != "complete" or health["health"]["overflow"]:
        report["limitations"].append("guest detail incomplete or counters overflowed")
    events = [list(map(int, ln.split()[3:])) for ln in lines if ln.startswith(b"NATIVE HIERARCHY EVENT ")]
    timers = [list(map(int, ln.split()[3:])) for ln in lines if ln.startswith(b"NATIVE HIERARCHY TIMER ")]
    entries = [(r,t,timers[i-1]) for i,(r,t) in enumerate(zip(events,timers)) if i and r[1] == 1 and r[8] == 3]
    report["idle_irq_count"] = len(entries)
    if count != 4 * len(entries) or not count:
        report["limitations"].append("missing or additional idle boundary/IRQ callbacks")
    for i,(seq,site,cpu,tid,w0,c,w1) in enumerate(samples):
        if (seq,site,cpu,tid) != (i,i%4,0,args.tid) or not (0 < w0 <= w1 and c > 0) or (
                i and (w0 < samples[i-1][6] or c < samples[i-1][5])):
            report["limitations"].append(f"unexpected callback identity, ordering or path at {i}")
            break
    if report["limitations"]:
        return report
    prev = 0
    for i,(row,t,old) in enumerate(entries):
        require(t[10] == old[1] and t[11] == row[3] and t[16] == row[19]
                and t[15] & 15 == 5 and t[17] & 1023 == row[2] and old[3] == row[13],
                "unmatched prior arm, vector or saved frame")
        require(prev <= t[18] <= t[19] <= t[11] and t[19] > 0 and t[20] & 128, "unexpected idle landing/mask")
        prev = t[11]
        anchor = dict(row=row[0], arm=t[10], preceding_arm_kind=old[2], due_ticks=row[13],
                      iar=t[17], idle_saved_pc=t[16], wfi_plus_four=t[16] == idle+4,
                      outcome=t[21], scored_refill=t[21] in (1,2), samples=[])
        for j, (kind, guest) in enumerate((("masked_idle",t[19]),("idle_irq",t[11]))):
            a,b = samples[4*i+2*j:4*i+2*j+2]
            anchor["samples"].append(dict(kind=kind, guest_ticks=guest, seq=[a[0],b[0]],
                                          host_ns=[a[4],b[6]], thread_cpu_ns=[a[5],b[5]],
                                          width_ns=b[6]-a[4]))
        report["anchors"].append(anchor)
    return report


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="action", required=True)
    spec = sub.add_parser("spec")
    spec.add_argument("--plugin", required=True, type=Path)
    spec.add_argument("--output", required=True, type=Path)
    audit = sub.add_parser("correlate")
    audit.add_argument("--markers", required=True, type=Path)
    audit.add_argument("--uart", required=True, type=Path)
    audit.add_argument("--tid", required=True, type=int)
    for parser in (spec,audit):
        parser.add_argument("--kernel", required=True, type=Path)
    args = p.parse_args()
    try:
        idle, addresses = sites(args.kernel)
        if args.action == "spec":
            plugin, out = args.plugin.resolve(strict=True), args.output.resolve()
            require(plugin.is_file() and not out.exists() and not any("," in str(p) for p in (plugin,out)),
                    "expected plugin and fresh output paths without commas")
            print(",".join([str(plugin),f"out={out}",*(f"site{i}={pc:x}:{word:x}" for i,(pc,word) in enumerate(addresses))]))
            return 0
        result = correlate(args,idle,addresses)
        print(json.dumps(result,indent=2))
        return 2 if result["limitations"] else 0
    except (OSError, ValueError) as exc:
        p.exit(1,f"invalid marker evidence: {exc}\n")


if __name__ == "__main__":
    sys.exit(main())
