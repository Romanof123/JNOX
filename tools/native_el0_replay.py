"""Replay one native process snapshot and exception stream: DIR TRUSTED_SESSION_SHA256."""
import argparse
import io
import json
from pathlib import Path
import re
import struct

from ordinary_amc_capture import FLAGS, sha
from ordinary_amc_replay import bounded, obj

PAGE = 4096
PA_MASK = 0x0000FFFFFFFFF000
U64 = (1 << 64) - 1
RX, RO, RW = 5, 4, 6
SVC = 0xD4000001 | (0x4A01 << 5)
ESR = 0x56004A01


def need(condition, message):
    if not condition:
        raise ValueError(message)


def elf_file(blob):
    from elftools.elf.elffile import ELFFile

    elf = ELFFile(io.BytesIO(blob))
    need(
        elf.elfclass == 64
        and elf.little_endian
        and elf["e_machine"] == "EM_AARCH64"
        and elf["e_type"] == "ET_EXEC",
        "AArch64 executable identity",
    )
    return elf


def symbol(elf, name):
    table = elf.get_section_by_name(".symtab")
    hits = table.get_symbol_by_name(name) if table else None
    need(hits and len(hits) == 1, "missing or ambiguous symbol " + name)
    return hits[0]["st_value"], hits[0]["st_size"]


class Snapshot:
    def __init__(self, base, raw):
        self.base = base
        self.raw = raw

    def read(self, pa, size):
        offset = pa - self.base
        need(0 <= offset <= len(self.raw) - size, "physical read outside captured RAM")
        return self.raw[offset : offset + size]

    def leaf(self, root, va):
        need(root == root & PA_MASK and 0 <= va < 1 << 39, "TTBR/VA range")
        table = root
        for shift in (30, 21, 12):
            addr = table + 8 * ((va >> shift) & 511)
            desc = int.from_bytes(self.read(addr, 8), "little")
            if not desc & 1:
                return None
            need(desc & 3 == 3, "unexpected block/table type")
            table = desc & PA_MASK
            if shift != 12:
                need(desc == table | 3, "unsupported intermediate permissions")
        return desc


def vread(mem, root, va, n):
    chunks = []
    while n:
        size = min(n, PAGE - va % PAGE)
        desc = mem.leaf(root, va)
        need(desc is not None, "unmapped user data")
        chunks.append(mem.read((desc & PA_MASK) + va % PAGE, size))
        n -= size
        va += size
    return b"".join(chunks)


def permissions(mem, root, va, access, device=False):
    desc = mem.leaf(root, va)
    if access == "guard":
        need(desc is None, "mapped guard boundary")
        return None
    need(desc is not None and desc & (1 << 10), "missing/access-flag-clear page")
    ap = desc >> 6 & 3
    pxn = desc >> 53 & 1
    uxn = desc >> 54 & 1
    expected = {
        "user rx": (3, 1, 0),
        "user r": (3, 1, 1),
        "user rw": (1, 1, 1),
        "kernel rx": (2, 0, 1),
        "kernel rw": (0, 1, 1),
    }
    need(
        (ap, pxn, uxn) == expected[access] and (desc >> 2 & 7) == int(device),
        f"page permissions at {va:#x}",
    )
    if access.startswith("kernel"):
        need(desc & PA_MASK == va & ~4095, "privileged identity mapping")
    return hex(desc)


def audit(kernel, user, mem):
    k = elf_file(kernel)
    u = elf_file(user)
    start, _ = symbol(k, "_start")
    limit, _ = symbol(k, "__stack_top")
    rw, _ = symbol(k, "__native_rw_start")
    vbar, vsize = symbol(k, "native_el0_vectors")
    need(vsize == 2048 and vbar % 2048 == 0, "linked vector table")
    need(
        mem.base == start == 0x40200000 and len(mem.raw) == limit - start,
        "linked RAM extent",
    )
    ip, ilen = symbol(k, "jnox_native_el0_image")
    need(
        ilen == len(user) and mem.read(ip, ilen) == user,
        "embedded user ELF in stopped RAM",
    )
    # Confirm linked file-backed embedded image independently of the RAM snapshot.
    backing = [
        s
        for s in k.iter_segments()
        if s["p_type"] == "PT_LOAD"
        and s["p_vaddr"] <= ip <= s["p_vaddr"] + s["p_filesz"] - ilen
    ]
    need(len(backing) == 1, "kernel ELF embedded symbol extent")
    off = backing[0]["p_offset"] + ip - backing[0]["p_vaddr"]
    need(kernel[off : off + ilen] == user, "linked embedded bytes")
    jp, jlen = symbol(k, "jnox_native_el0_journal")
    need(jlen == 9 * 16 * 8, "journal layout")
    rows = list(struct.iter_unpack("<16Q", mem.read(jp, jlen)))
    (
        version,
        state,
        fail,
        length,
        entry,
        sp,
        bottom,
        top,
        events,
        exitcode,
        valid,
        stdout,
        el,
        sctlr,
        root,
        argc,
    ) = rows[0]
    need(
        (
            version,
            state,
            fail,
            length,
            entry,
            sp,
            bottom,
            top,
            events,
            exitcode,
            valid,
            stdout,
            el,
            argc,
        )
        == (
            1,
            3,
            0,
            len(user),
            0x400000,
            0x01010FB0,
            0x01001000,
            0x01011000,
            2,
            37,
            1,
            0,
            4,
            3,
        ),
        "terminal journal",
    )
    need(
        sctlr & 1 and entry == u["e_entry"] == symbol(u, "_start")[0],
        "enabled translation and linked entry",
    )
    need(not any(any(row) for row in rows[3:]), "uncommitted journal data")
    mappings = {}
    segments = [
        s for s in u.iter_segments() if s["p_type"] == "PT_LOAD" and s["p_memsz"]
    ]
    need(
        sorted(s["p_flags"] for s in segments) == [RO, RX, RW],
        "three user segment permissions",
    )
    notes = [sec for sec in u.iter_sections() if sec.name == ".note.jnox.abi"]
    need(
        len(notes) == 1
        and notes[0]["sh_type"] == "SHT_NOTE"
        and notes[0]["sh_flags"] == 2,
        "native ABI note identity",
    )
    note = notes[0]
    expected_note = struct.pack("<III8sI", 5, 4, 1, b"JNOX", 1)
    need(
        note.data() == expected_note
        and any(
            seg["p_flags"] == RO
            and 0
            <= note["sh_addr"] - seg["p_vaddr"]
            <= seg["p_filesz"] - len(expected_note)
            and note["sh_addr"] - seg["p_vaddr"] == note["sh_offset"] - seg["p_offset"]
            for seg in segments
        ),
        "native ABI v1 in RO load",
    )
    for seg in segments:
        va = seg["p_vaddr"]
        end = va + seg["p_memsz"]
        flags = seg["p_flags"]
        need(0x400000 <= va < end <= 0x800000 and va % PAGE == 0, "user segment extent")
        for addr in range(va, end, PAGE):
            mappings[hex(addr)] = permissions(
                mem, root, addr, {RX: "user rx", RO: "user r", RW: "user rw"}[flags]
            )
        if flags != RW:
            need(
                vread(mem, root, va, seg["p_filesz"]) == seg.data(),
                "immutable loaded segment",
            )
    for addr in range(bottom, top, PAGE):
        mappings[hex(addr)] = permissions(mem, root, addr, "user rw")
    for addr in (bottom - PAGE, top):
        mappings[hex(addr)] = permissions(mem, root, addr, "guard")
    for addr, access, device in (
        (start, "kernel rx", False),
        (rw, "kernel rw", False),
        (vbar, "kernel rx", False),
        (0x08000000, "kernel rw", True),
        (0x08010000, "kernel rw", True),
        (0x09000000, "kernel rw", True),
    ):
        mappings[hex(addr)] = permissions(mem, root, addr, access, device)
    stack = struct.unpack("<6Q", vread(mem, root, sp, 48))
    need(
        stack[0] == 3 and stack[4:] == (0, 0),
        "bootstrap argc, argv terminator and empty environment",
    )
    for ptr, value in zip(
        stack[1:4], (b"/bin/native_startup_probe\0", b"8\0", b"13\0")
    ):
        need(
            sp + 48 <= ptr <= top - len(value)
            and vread(mem, root, ptr, len(value)) == value,
            "bootstrap argv string",
        )
    need(stack[1:4] == (sp + 48, sp + 74, sp + 76), "exact startup string addresses")
    for va in range(start, limit, PAGE):
        permissions(mem, root, va, "kernel rx" if va < rw else "kernel rw")
    final_data = {}
    for name, expected in (
        ("products", (16, 13)),
        ("bias", (8,)),
        ("__jnox_errno_location.initial_thread_errno", (38,)),
        ("weights", (2, 1)),
    ):
        addr, size = symbol(u, name)
        need(size == 4 * len(expected), "user symbol extent " + name)
        actual = tuple(
            v[0] for v in struct.iter_unpack("<I", vread(mem, root, addr, size))
        )
        need(actual == expected, "final user symbol " + name)
        final_data[name] = actual
    pcs = []
    for i, row in enumerate(rows[1:3]):
        (
            idx,
            vector,
            level,
            esr,
            pstate,
            pc,
            usp,
            nr,
            x0,
            x1,
            x2,
            result,
            disp,
            vbr,
            ttbr,
            far,
        ) = row
        need(
            (idx, vector, level, esr, vbr, ttbr) == (i, 0x400, 4, ESR, vbar, root),
            "SVC provenance",
        )
        need(
            pstate & 0x1F == 0 and bottom <= usp <= sp and usp % 16 == 0,
            "EL0t origin and aligned stack",
        )
        need(
            pc % 4 == 0
            and any(
                s["p_flags"] == RX
                and s["p_vaddr"] <= pc - 4 < pc <= s["p_vaddr"] + s["p_filesz"]
                for s in segments
            )
            and int.from_bytes(vread(mem, root, pc - 4, 4), "little") == SVC,
            "user SVC instruction",
        )
        if i == 0:
            need(
                (nr, x0, x2, result, disp) == (4, 1, 3, (-38) & U64, 1),
                "write syscall result",
            )
            need(vread(mem, root, x1, 3) == b"37\n", "write pointer contents")
        else:
            need((nr, x0, result, disp) == (1, 37, 0, 2), "exit syscall")
        pcs.append(pc)
    return dict(
        entry=entry,
        initial_sp=sp,
        ttbr0=root,
        sctlr=sctlr,
        vector=vbar + 0x400,
        elrs=pcs,
        final_symbols=final_data,
        exit_status=37,
        stdout_bytes=0,
        write_result=-38,
        page_descriptors=mappings,
    )


def trace_check(raw, report):
    lines = iter(line for line in raw.decode("ascii").splitlines() if line.strip())

    def take(pattern):
        match = re.fullmatch(pattern, next(lines, ""))
        need(match is not None, "missing or unexpected QEMU transition: " + pattern)
        return match

    def eret(pc):
        match = take(
            r"Exception return from AArch64 EL1 to AArch64 EL0 PC (0x[\da-fA-F]+)"
        )
        need(int(match[1], 16) == pc, "EL0 return PC")

    eret(report["entry"])
    for i, pc in enumerate(report["elrs"]):
        take(r"Taking exception 2 \[SVC\](?: on CPU 0)?")
        take(r"\.\.\.from EL0 to EL1")
        syndrome = take(r"\.\.\.with ESR 0x15/(0x[\da-fA-F]+)")
        saved = take(r"\.\.\.with ELR (0x[\da-fA-F]+)")
        vector = take(r"\.\.\.to EL1 PC (0x[\da-fA-F]+) PSTATE (0x[\da-fA-F]+)")
        need(
            (int(syndrome[1], 16), int(saved[1], 16), int(vector[1], 16))
            == (ESR, pc, report["vector"])
            and int(vector[2], 16) & 15 == 5,
            "QEMU SVC syndrome/ELR/vector",
        )
        if i == 0:
            eret(pc)
    need(not list(lines), "unexpected transition after process exit")


def check_uart(raw, user_digest):
    expected = (
        b"JNOX M1 BOOT\nJNOX M1 READY\nNATIVE EL0 ELF "
        + user_digest.encode("ascii")
        + b"\nNATIVE EL0 EXIT 37\n"
    )
    need(raw == expected, "terminal markers and application stdout")


def replay(folder, digest):
    need(sha(folder / "session.json") == digest, "trusted session digest mismatch")
    rec = json.loads(bounded(folder / "session.json", 65536), object_pairs_hook=obj)
    required = {
        "launch.json",
        "kernel.elf",
        "user.elf",
        "probe.c",
        "stderr",
        "uart.bin",
        "exceptions.log",
        "ram.bin",
    }
    hashes = rec["hashes"]
    need(set(hashes) <= required, "unknown session artifact")
    for filename, value in hashes.items():
        need(sha(folder / filename) == value, "hash mismatch: " + filename)
    if rec["marker"].startswith("FAULT"):
        raise ValueError("captured native process " + rec["marker"])
    if set(hashes) != required or not rec["marker"].startswith("EXIT "):
        raise FileNotFoundError("incomplete capture: " + rec["marker"])
    need(rec["pid"] > 0, "no QEMU PID")
    run = json.loads(bounded(folder / "launch.json", 65536), object_pairs_hook=obj)
    need(
        run["version"] == 1
        and re.fullmatch("[0-9a-f]{40}", run["revision"])
        and re.fullmatch("[0-9a-f]{64}", run["qemu_sha256"])
        and run["qemu_version"].strip(),
        "launch provenance",
    )
    argv = run["argv"]
    fixed = [*FLAGS, "-qmp", "stdio", "-serial"]
    need(
        argv[1 : 1 + len(fixed)] == fixed and len(argv) == len(fixed) + 8,
        "QEMU configuration",
    )
    uart, dflag, mode, logflag, log, kflag, kelf = argv[-7:]
    need(
        (dflag, mode, logflag, kflag) == ("-d", "int", "-D", "-kernel")
        and uart.startswith("file:"),
        "QEMU observers",
    )

    def path_parts(s):
        return s.replace(chr(92), "/").rsplit("/", 1)

    paths = [path_parts(s) for s in (uart[5:], log, kelf)]
    need(
        [p[-1] for p in paths] == ["uart.bin", "exceptions.log", "kernel.elf"]
        and len({p[0] for p in paths}) == 1,
        "QEMU artifact paths",
    )
    for key, name in (
        ("kernel", "kernel.elf"),
        ("user", "user.elf"),
        ("probe", "probe.c"),
    ):
        need(run[key + "_sha256"] == hashes[name], "launch asset digest")
    need(rec["marker"] == "EXIT 37", "terminal capture marker")
    check_uart(bounded(folder / "uart.bin", 1 << 20), hashes["user.elf"])
    memory = bounded(folder / "ram.bin", 16 << 20)
    need(len(memory) == run["ram_bytes"], "physical snapshot size")
    report = audit(
        bounded(folder / "kernel.elf", 64 << 20),
        bounded(folder / "user.elf", 16 << 20),
        Snapshot(run["ram_base"], memory),
    )
    trace_check(bounded(folder / "exceptions.log", 16 << 20), report)
    report.update(
        source_revision=run["revision"],
        user_sha256=hashes["user.elf"],
        kernel_sha256=hashes["kernel.elf"],
        probe_sha256=hashes["probe.c"],
        session_sha256=digest,
        scope="one native startup, unsupported write and process exit on single-vCPU virt",
    )
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("session_sha256")
    args = parser.parse_args()
    try:
        print(json.dumps(replay(args.folder, args.session_sha256), indent=2))
    except (FileNotFoundError, ImportError) as exc:
        parser.exit(2, f"UNOBSERVED {exc}\n")
    except (OSError, ValueError, KeyError, IndexError, TypeError) as exc:
        parser.exit(1, f"INVALID {exc}\n")
