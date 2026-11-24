"""Replay a native P/Q cold capture from its trusted session SHA-256."""
import argparse
import json
from pathlib import Path, PurePosixPath
import re
import struct

from native_el0_replay import PA_MASK, SVC, Snapshot, elf_file, need, permissions, symbol, vread
from native_weighted_capture import CLOCKS, qemu_command
from native_weighted_trace import transitions
from ordinary_amc_capture import sha
from ordinary_amc_replay import bounded, obj

TABLES = {
    "partition-p": [(0, 2), (1, 1), (2, 1)],
    "partition-q": [(1, 1), (2, 1), (0, 2), (1, 1)],
}
TOP = 0x01011000
U64_MAX = (1 << 64) - 1
ROW = struct.Struct("<16Q")
RESULT = struct.Struct("<6Q")


def captured(folder, digest):
    def read_json(name):
        return json.loads(bounded(folder / name, 65536), object_pairs_hook=obj)

    need(sha(folder / "session.json") == digest, "capture identity")
    session, run = read_json("session.json"), read_json("launch.json")
    names = {"launch.json", "kernel.elf", "user.elf", "probe.c", "stderr",
             "ram.bin", "uart.bin", "exceptions.log"}
    need(session["marker"] == "END" and set(session["hashes"]) == names, "complete capture")
    for name in names:
        need(sha(folder / name) == session["hashes"][name], "capture digest " + name)
    for item, name in (("kernel", "kernel.elf"), ("user", "user.elf"), ("probe", "probe.c")):
        need(run[item + "_sha256"] == session["hashes"][name], "launch digest " + item)
    need(run["version"] == 1 and run["clock"] in CLOCKS and run["arm"] in TABLES
         and re.fullmatch("[0-9a-f]{40}", run["revision"])
         and re.fullmatch("[0-9a-f]{64}", run["qemu_sha256"])
         and run["qemu_version"].strip(), "launch configuration")
    argv = [part.replace(chr(92), "/") for part in run["argv"]]
    need(argv == qemu_command(argv[0], PurePosixPath(argv[-1]).parent, run["clock"]),
         "QEMU machine and clock arguments")
    need(bounded(folder / "uart.bin", 1 << 20)
         == b"JNOX M1 BOOT\nJNOX M1 READY\nNATIVE PARTITION END\n", "UART completion")
    return run


class RetainedFrames:
    """Linked snapshots and independently recorded assembly restore operands."""

    def __init__(self, kernel, mem, transfers):
        self.mem = mem
        self.extents = []
        def bank(name, size, used):
            address, extent = symbol(kernel, "native_partition_" + name + "_v1")
            need(address % 16 == 0 and extent == size and
                 all(address + size <= start or end <= address for start, end in self.extents),
                 "retained bank layout " + name)
            self.extents.append((address, address + size))
            need(not any(mem.read(address, size)[used:]), "unused retained bank " + name)
            return address

        self.traps = bank("traps", 15 * 832, transfers * 832)
        self.resume = bank("resume", 15 * 832, transfers * 832)
        witnesses = bank("restore", 15 * 8, transfers * 8)
        cursor = bank("frame_cursor", 8, 8)
        need(int.from_bytes(mem.read(cursor, 8), "little") == transfers, "restore count")
        self.witnesses = struct.unpack("<15Q", mem.read(witnesses, 120))

    def words(self, address):
        return struct.unpack("<104Q", self.mem.read(address, 832))

    def selection(self, seq, pointer, initial):
        need(pointer == self.witnesses[seq - 1], "actual assembly restore base")
        if initial:
            need(all(pointer + 832 <= start or end <= pointer for start, end in self.extents),
                 "independent initial frame storage")
        return self.words(pointer)

    def interruption(self, seq, pointer):
        trap = self.traps + (seq - 1) * 832
        resume = self.resume + (seq - 1) * 832
        need(pointer == trap, "actual exception stack frame")
        original = self.words(trap)
        need(original == self.words(resume), "original and saved 104-word frames")
        return original, resume


def replay(folder, digest):
    run = captured(folder, digest)
    kernel = elf_file(bounded(folder / "kernel.elf", 64 << 20))
    user_bytes = bounded(folder / "user.elf", 16 << 20)
    user = elf_file(user_bytes)
    mem = Snapshot(run["ram_base"], bounded(folder / "ram.bin", 16 << 20))
    need(mem.base == symbol(kernel, "_start")[0] == 0x40200000
         and len(mem.raw) == run["ram_bytes"] == symbol(kernel, "__stack_top")[0] - mem.base,
         "linked RAM extent")
    embedded = []
    for seg in kernel.iter_segments():
        if seg["p_type"] == "PT_LOAD":
            data = seg.data()
            pos = data.find(user_bytes)
            if pos >= 0:
                need(data.find(user_bytes, pos + 1) < 0, "ambiguous packaged image")
                embedded.append(seg["p_paddr"] + pos)
    need(len(embedded) == 1 and mem.read(embedded[0], len(user_bytes)) == user_bytes,
         "exact packaged user ELF")
    va, size = symbol(user, "native_partition_result")
    need(size == RESULT.size, "linked result layout")
    vbar, size = symbol(kernel, "native_vruntime_vectors")
    need(vbar % 2048 == 0 and size == 2048, "linked EL1 vectors")
    addr, size = symbol(kernel, "native_vruntime_journal")
    need(size == 513 * ROW.size, "bounded journal")
    head, *rows = ROW.iter_unpack(mem.read(addr, size))
    table = TABLES[run["arm"]]
    slots = len(table) * 3 + 1
    freq, q, epoch, count = head[3:7]
    retained_symbol = kernel.get_section_by_name(".symtab").get_symbol_by_name("native_partition_traps_v1")
    version = 3 if retained_symbol else 2
    retained = RetainedFrames(kernel, mem, slots + 2) if retained_symbol else None
    need(head[:3] == (version, 3, 1 if run["arm"] == "partition-p" else 2)
         and freq > 0 and freq % 1000 == 0 and q == freq // 20 and epoch > 0
         and head[7:9] == (0, 0) and head[10:12] == (slots, slots)
         and head[14] == va and count == 3 + (slots + 2) * (3 if retained else 19)
         and not any(any(r) for r in rows[count:]), "terminal header and complete rows")
    items = iter(rows[:count])
    trace = iter(transitions(bounded(folder / "exceptions.log", 64 << 20)))

    def take(tag, pid, width, tick=None):
        row = next(items, ())
        need(row and row[0] == tag and row[2] == pid and (tick is None or row[1] == tick)
             and not any(row[3 + width:]), f"journal tag {tag}, pid {pid}")
        return row[1], row[3:3 + width]

    def frame(tag, pid, tick):
        return tuple(v for _ in range(8) for v in take(tag, pid, 13, tick)[1])

    def event(kind, pc, address):
        ev = next(trace, None)
        need(ev and (ev.kind, ev.pc, ev.vector) == (kind, pc, address), "external " + kind)

    roots, ttbrs, private, controls = [], [], set(), []
    initial = user["e_entry"]
    segments = [s for s in user.iter_segments() if s["p_type"] == "PT_LOAD"]
    need([s["p_flags"] for s in segments] == [5, 4, 6], "user LOAD permissions")
    for pid in range(3):
        _, (ttbr, object_pa, stack_pa, pc, sp) = take(1, pid, 5)
        root = ttbr & PA_MASK
        need(ttbr == ((pid + 1) << 48) | root and root not in roots
             and pc == initial == 0x400000 and sp == TOP - 80, "initial mapping")
        ttbrs.append(ttbr)
        roots.append(root)
        vas = []
        for seg in segments:
            begin = seg["p_vaddr"]
            vas.extend(range(begin, begin + seg["p_memsz"], 4096))
            for page in range(begin, begin + seg["p_memsz"], 4096):
                permissions(mem, root, page, {4: "user r", 5: "user rx", 6: "user rw"}[seg["p_flags"]])
            if seg["p_flags"] != 6:
                need(vread(mem, root, begin, seg["p_filesz"]) == seg.data(), "loaded immutable bytes")
        vas.extend(range(0x01001000, TOP, 4096))
        for page in range(0x01001000, TOP, 4096):
            permissions(mem, root, page, "user rw")
        for page in vas:
            desc = mem.leaf(root, page)
            physical = desc & PA_MASK
            need(desc & (1 << 11) and physical not in private, "distinct private image/stack pages")
            private.add(physical)
        need(object_pa == mem.leaf(root, va) & PA_MASK and stack_pa == mem.leaf(root, sp) & PA_MASK,
             "result and stack page provenance")
        permissions(mem, root, 0x01200000, "user r")
        controls.append(mem.leaf(root, 0x01200000) & PA_MASK)
    need(len(set(controls)) == 1 and controls[0] not in private
         and struct.unpack("<2Q", mem.read(controls[0], 16)) == (1, slots + 2), "control page")

    saved, frame_ptrs, initial_ptrs, exits = {}, {}, {}, []
    last_work, last_result = [0, 0, 0], [None] * 3
    nominal = previous_entry = epoch
    released = False
    windows = []
    for seq in range(1, slots + 3):
        drain = seq > slots
        owner, units = (seq - slots - 1, 0) if drain else table[(seq - 1) % len(table)]
        released |= owner == 2
        pid = 2 if owner == 1 and not released else owner
        selected, dispatch = take(21, pid, 12)
        n, opening, closing, declared, ready, phase, space, pc, sp, psr, tls, pointer = dispatch
        need((n, declared, ready, phase, space) == (seq, owner, 3 if released else 1, int(drain), ttbrs[pid])
             and opening == (previous_entry if drain else nominal) and previous_entry <= selected
             and (drain or closing == nominal + units*q), f"dispatch {seq}")
        restored = (retained.selection(seq, pointer, pid not in saved)
                    if retained else frame(24, pid, selected))
        if pid not in saved:
            expected = [0] * 104
            expected[32:36] = [0x340, initial, TOP - 80, 4]
            expected[102] = TOP - 96 - 16*pid
            saved[pid] = tuple(expected)
            need(pointer % 16 == 0 and mem.base <= pointer <= mem.base + len(mem.raw) - 832
                 and all(abs(pointer - prev) >= 832 for prev in initial_ptrs.values()), "private frame backing")
            initial_ptrs[pid] = pointer
            frame_ptrs[pid] = pointer
        need(pointer == frame_ptrs[pid] and restored == saved[pid]
             and (pc, sp, psr, tls) == (restored[33], restored[34], restored[32], restored[102]),
             "full restored frame")
        event("eret", pc, 0)
        entry, stop = take(22, pid, 13 if retained else 12)
        n, start, compare, ctl, iar, vector, epc, esp, epsr, etls, x8, x0 = stop[:12]
        _, result = take(23, pid, 7, entry)
        if retained:
            bank, frame_ptrs[pid] = retained.interruption(seq, stop[12])
        else:
            bank = frame(25, pid, entry)
        saved[pid] = bank
        need((bank[33], bank[34], bank[32], bank[102], bank[8], bank[0]) ==
             (epc, esp, epsr, etls, x8, x0) and epsr & 0x3ff == 0x340 and bank[35] == 4
             and esp % 16 == 0 and 0x01001000 <= esp <= TOP
             and initial <= epc < initial + segments[0]["p_filesz"] and epc % 4 == 0,
             "interrupted user frame")
        need(n == seq and selected <= start < compare == closing and start < entry
             and result[:2] == (seq, pid + 1), "timer and result provenance")
        seen, first, work, checksum, status = result[2:]
        last_result[pid] = result[1:]
        if drain:
            event("svc", epc, vbar + 0x400)
            need(vector == 0x400 and iar == U64_MAX and ctl & 3 == 1 and entry < closing <= selected + q
                 and x8 == 1 and x0 == status == 41 + pid and 0 < last_work[pid] <= work
                 and checksum == work * (3 if pid == 0 else 5)
                 and int.from_bytes(vread(mem, roots[pid], epc-4, 4), "little") == SVC, "bounded normal exit")
            exits.append(dict(pid=pid, chunks=work, checksum=checksum, status=status))
        else:
            event("irq", epc, vbar + 0x480)
            need(vector == 0x480 and iar == 30 and ctl & 7 == 5 and entry >= closing
                 and seen == seq and nominal <= previous_entry <= start <= first < closing
                 and status == 0, "release, in-context observation and closing IRQ")
            need((pid == 2 and work == checksum == 0) or
                 (pid < 2 and work > last_work[pid] and
                  checksum in ((work-1) * (3 if pid == 0 else 5), work * (3 if pid == 0 else 5))),
                 "worker progress in the owned window")
            windows.append(dict(generation=seq, owner=owner, pid=pid, release=nominal,
                                selected=selected, return_sample=start, user_tick=first, irq_entry=entry,
                                close=closing, entry_to_return=start-previous_entry,
                                return_bracket=first-start, observed_to_entry=entry-first,
                                spill=entry-closing, latency=first-nominal, extension=seq == slots))
            last_work[pid] = work
            nominal = closing
            if seq == slots:
                need(head[9] == entry, "completion after observation extension")
        previous_entry = entry
    need(next(items, None) is None and next(trace, None) is None, "complete event streams")
    for pid, root in enumerate(roots):
        need(RESULT.unpack(vread(mem, root, va, RESULT.size)) == last_result[pid],
             "stopped private result")
        need(struct.unpack("<104Q", mem.read(frame_ptrs[pid], 832)) == saved[pid],
             "stopped private frame")
    misses = [w["generation"] for w in windows if w["latency"] > freq // 1000]
    elapsed = sum(w["entry_to_return"] + w["return_bracket"] + w["observed_to_entry"] for w in windows)
    need(elapsed == head[9] - epoch and len(misses) == head[12], "complete accounting and timing verdict")
    return dict(arm=run["arm"], clock=run["clock"], revision=run["revision"], session_sha256=digest,
                frequency=freq, windows=windows, exits=exits, timing_misses=misses)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("session_sha256")
    args = parser.parse_args()
    report = replay(args.folder, args.session_sha256)
    print(json.dumps(report, indent=2))
    raise SystemExit(int(bool(report["timing_misses"])))
