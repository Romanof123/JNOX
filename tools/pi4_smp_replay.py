"""Replay the retained Pi 4 directed-wake SW v1 journal (one cold launch)."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import sys

TASKS = ("pi4_smp_producer", "pi4_smp_background", "pi4_smp_receiver")
STACKS = "pi4_smp_task_stacks"
FRAME = 832
U64 = (1 << 64) - 1


def need(condition, message):
    if not condition:
        raise ValueError(message)


def bounded(path, maximum):
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
    need(len(data) <= maximum, f"{path}: size limit exceeded")
    return data


def unique_fields(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, f"duplicate manifest field {key}")
        result[key] = value
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def linked(elf, image, helpers=()):
    from elftools.elf.elffile import ELFFile
    binary = ELFFile(io.BytesIO(elf))
    need(binary.elfclass == 64 and binary.little_endian and binary["e_machine"] == "EM_AARCH64"
         and binary["e_entry"] == 0x80000, "unexpected ELF")
    for seg in binary.iter_segments():
        if seg["p_type"] == "PT_LOAD" and seg["p_memsz"]:
            need(0x80000 <= seg["p_paddr"] == seg["p_vaddr"] and
                 seg["p_paddr"] + seg["p_memsz"] <= 0x100000, "load outside board window")
    expected = bytearray()
    for section in sorted(binary.iter_sections(), key=lambda s: s["sh_addr"]):
        if section["sh_flags"] & 2 and section["sh_type"] != "SHT_NOBITS" and section["sh_size"]:
            offset = section["sh_addr"] - 0x80000
            need(len(expected) <= offset and offset + section["sh_size"] <= 0x80000, "section range")
            expected.extend(bytes(offset - len(expected)))
            expected.extend(section.data())
    need(expected == image and image, "ELF sections and raw image differ")
    table = binary.get_section_by_name(".symtab")
    need(table is not None, "missing ELF symbols")
    symbols = {}
    for name in (*TASKS, STACKS, *helpers):
        matches = table.get_symbol_by_name(name) or []
        need(len(matches) == 1, f"ambiguous or missing {name}")
        sym = matches[0]
        size = sym["st_size"]
        addr = sym["st_value"]
        section = binary.get_section(sym["st_shndx"])
        need(section["sh_addr"] <= addr < addr + size <= section["sh_addr"] + section["sh_size"]
             and section["sh_flags"] & (1 if name == STACKS else 4), f"bad symbol extent {name}")
        symbols[name] = (addr, size)
    return symbols


def numbers(tokens, count):
    need(len(tokens) == count and all(re.fullmatch(rb"[0-9]{1,20}", n) for n in tokens), "row shape")
    result = tuple(map(int, tokens))
    need(max(result) <= U64, "field overflow")
    return result


def parse(raw):
    lines = [line for line in raw.splitlines() if line.startswith(b"SW ")]
    failure = [line for line in lines if line.startswith(b"SW FAIL ")]
    need(not failure, f"guest local failure: {failure!r}")
    need(lines and lines[0] == b"SW HEADER v=1 cap=32 frame=832", "missing/incomplete SW v1 header")
    pos = 1
    def take(tag, n):
        nonlocal pos
        need(pos < len(lines), f"missing SW {tag}")
        words = lines[pos].split()
        pos += 1
        need(words[:2] == [b"SW", tag.encode()], f"expected SW {tag}")
        return numbers(words[2:], n)
    stacks = [take("STACK", 4) for _ in TASKS]
    cores, journals = [], []
    for cpu in range(2):
        core = take("CORE", 20)
        need(core[0] == cpu and core[1] <= 32 and core[2:4] == (0, 0), f"CPU{cpu} failure/loss")
        cores.append(core)
        rows = [take("ROW", 10) for _ in range(core[1])]
        for i, row in enumerate(rows):
            need(row[:2] == (cpu, i + 1) and (not i or row[4] >= rows[i-1][4]), "CPU/sequence/time mismatch")
        journals.append(rows)
    end = take("END", 4)
    need(pos == len(lines), "additional SW stream")
    return stacks, cores, journals, end


def order(cpu):
    if cpu == 0:
        return [(1, 0), (2, 0)] + [(k, g) for g in (1, 2) for k in (3, 4, 5, 6)]
    result = [(1, 0), (2, 0), (3, 0)]
    for g in (1, 2, 3):
        result.extend([(12, g-1), (13, g), (14, g-1), (3, g-1)])
        if g <= 2:
            result.extend((k, g) for k in (8, 9, 10, 11, 3, 7))
    return result + [(15, 2)]


def score(raw, symbols, helpers=None):
    stacks, cores, journals, end = parse(raw)
    start, size = symbols[STACKS]
    need(size == 3 * (16384 + 32), "stack object size")
    for role, stack in enumerate(stacks):
        lo = start + role * (16384 + 32) + 16
        need(stack == (role, lo, lo + 16384, 1), "bad stack bounds/guard")
    need(cores[0][7:11] == cores[1][7:11], "different translation configuration")
    targets, frequencies, groups = [], [], []
    helpers = helpers or {}
    attributed_helpers = []
    gaps = []
    external = []
    for cpu, (core, rows) in enumerate(zip(cores, journals)):
        need([(r[2], r[3]) for r in rows] == order(cpu), f"CPU{cpu} workload order/generation")
        mpidr, entry, el, ttbr, tcr, mair, ctl, ext = core[4:12]
        typer, target, group, dist, interface, pmr, pri, enabled = core[12:]
        secure = bool(typer & 1024)
        need(mpidr & 0xff00ffffff == cpu and el == 4 and entry in (4, 8)
             and ttbr and tcr and mair and ctl & 0x1005 == 0x1005, "boot state")
        need(0 < target < 256 and target & (target-1) == 0 and pri == 128 and pri < pmr <= 255
             and enabled & 2 and not interface & 512, "banked GIC state")
        need((ext & 64 and dist & 1 and interface & 1) if secure else
             (group & 2 and dist & 2 and interface & 6 == 6), "group access")
        boot = rows[0]
        need(boot[5:7] == (mpidr, el) and boot[7] > 0 and boot[8] <= boot[4] and boot[9] == FRAME,
             "missing local frequency/clock/identity")
        targets.append(target)
        frequencies.append(boot[7])
        records = {}
        previous = None
        for r in rows:
            _, _, k, g, tick, *a = r
            role = a[0]
            if k in (2, 8, 10, 12, 14, 15, 3):
                if k == 3:
                    want = 0 if cpu == 0 else (1 if previous == 14 else 2)
                else:
                    want = {2: (0 if cpu == 0 else 2), 8:1, 10:2, 12:2, 14:1, 15:1}[k]
                need(role == want, "wrong task role")
                _, low, high, _ = stacks[role]
                if k == 3:
                    pc, sp = a[1:3]
                    need(a[4] == mpidr, "wrong executing CPU")
                else:
                    frame, pc, sp = a[1:4]
                    need(low <= frame and frame + FRAME == sp, "saved frame bounds")
                need(low < sp <= high and sp % 16 == 0 and pc % 4 == 0, "PC/SP alignment")
                base, length = symbols[TASKS[role]]
                if k == 8 or (k == 14 and g):
                    if not base <= pc < base + length:
                        name = helpers.get(pc)
                        if name is None:
                            gaps.append(f"unattributed background continuation 0x{pc:x} generation {g}")
                        else:
                            lo, length = symbols[name]
                            need(lo <= pc < lo + length, "reviewed helper PC outside symbol")
                            attributed_helpers.append(dict(pc=pc, symbol=name, generation=g))
                else:
                    need(base <= pc < base + length, "PC outside linked task")
                if k == 2 or k == 14 and g == 0:
                    need(pc == base and sp == high, "initial task frame")
            key = (k, g, role if k == 3 else None)
            need(key not in records, "duplicate task row")
            records[key] = r
            previous = k
        groups.append(records)
    need(targets[0] != targets[1], "shared GIC target bit")
    def row(cpu, k, g, role=None):
        key = (k, g, role)
        need(key in groups[cpu], f"missing CPU{cpu} kind {k} generation {g} role {role}")
        return groups[cpu][key]
    def args(cpu,k,g,role=None):
        return row(cpu,k,g,role)[5:]
    need(args(1,3,0,2)[3] == 0, "initial receiver value")
    results = []
    for g, total in ((1,136),(2,392)):
        pub, send, ack = [args(0,k,g) for k in (4,5,6)]
        save, claim, selected, eoi = [args(1,k,g) for k in (8,9,10,11)]
        before = args(1,3,g-1,1)[3]
        after = args(1,3,g,1)[3]
        need(0 < before <= pub[1] < pub[2] <= save[4] < after and
             all(n % 16 == 0 for n in (before,pub[1],pub[2],save[4],after)), "background progress")
        need(pub[0] == total and pub[3:] == (0,0) and args(0,3,g,0)[3] == total, "producer work")
        need(row(0,4,g)[4] <= send[0] <= send[1] <= row(0,5,g)[4]
             and send[2:] == ((targets[1]<<16) | 0x8001, targets[1], total), "SGIR")
        iar, source = claim[:2]
        need(iar < 2**32 and iar & 1023 == 1 and source == (iar>>10)&7 and 1<<source == targets[0]
             and claim[2:] == (total,4*g,4*g+1), "IRQ/source/claim")
        need(eoi == (iar,2,0,0,0) and selected[:4] == args(1,12,g-1)[:4], "EOI or receiver frame")
        need(args(1,14,g)[:4] == save[:4], "incorrect background continuation")
        need(args(1,3,g,2)[3] == 2*total and args(1,7,g) == ack == (total,2*total,0,0,0),
             "receiver result or acknowledgement")
        results.append(dict(generation=g, payload=total, result=2*total, iar=iar, source=source,
                            send_call_ticks=send[1]-send[0], before=before, resumed=after))
    for g in (1,2,3):
        need(args(1,13,g) == (1,4*g,0,0,0), "block/exit state")
    for cpu, rows in enumerate(journals):
        for r in rows:
            k,g = r[2:4]
            if k in (2,8,10,12,14,15):
                external.append([cpu, {2:"eret",8:"irq",10:"eret",12:"svc",14:"eret",15:"svc"}[k], r[7]])
    need(end == (1,2,784,args(1,3,2,1)[3]) and args(1,15,2)[4] == end[3], "terminal progress")
    return dict(products=results, core_frequencies=frequencies, expected_external=external,
                rows=journals, software_gaps=gaps, reviewed_helpers=attributed_helpers)
def external_score(raw, expected):
    """Read a CPU-tagged observer's complete task interval, with one event per line."""
    by_cpu = {0: [], 1: []}
    for line in raw.splitlines():
        match = re.fullmatch(rb"CPU ([0-3]) (irq|svc|eret) (0x[0-9a-fA-F]+)", line)
        if not match:
            return "independent CPU-tagged exception history required"
        cpu = int(match[1])
        need(cpu in by_cpu, "unexpected participating observer CPU")
        by_cpu[cpu].append([cpu, match[2].decode(), int(match[3],16)])
    for cpu in by_cpu:
        need(by_cpu[cpu] == [r for r in expected if r[0] == cpu], f"CPU{cpu} external history mismatch")
    return "CPU-tagged observed PCs matched"


def replay(folder, session_digest):
    data = bounded(folder / "session.json", 65536)
    need(digest(data) == session_digest, "session hash mismatch")
    session = json.loads(data, object_pairs_hook=unique_fields)
    need(isinstance(session, dict) and session.get("mode") in ("model","physical")
         and isinstance(session.get("run_id"), str) and session["run_id"].strip(), "session identity")
    files = session["sha256"]
    def read(name, limit):
        raw = bounded(folder / name, limit)
        need(digest(raw) == files[name], f"hash mismatch for {name}")
        return raw
    elf = read("kernel.elf", 64*1024*1024)
    image = read("kernel8.img", 512*1024)
    uart = read("uart.bin", 1024*1024)
    external = read("external.log", 64*1024*1024) if "external.log" in files else None
    helpers = {}
    if "background-review.json" in files:
        review = json.loads(read("background-review.json", 65536), object_pairs_hook=unique_fields)
        disasm = read("disassembly.txt", 16*1024*1024)
        need(review["elf_sha256"] == digest(elf) and review["disassembly_sha256"] == digest(disasm)
             and isinstance(review["reviewer"],str) and review["reviewer"].strip(), "helper review identity")
        for item in review["sites"]:
            pc, name = item["pc"], item["symbol"]
            need(type(pc) is int and isinstance(name,str) and name and name not in (*TASKS, STACKS)
                 and pc not in helpers and item["exclusive_root"] == TASKS[1]
                 and item["continuation_preserved"] is True and item["all_callers_accounted"] is True
                 and isinstance(item["proof"],str) and item["proof"].strip(), "incomplete helper proof")
            helpers[pc] = name
    try:
        from elftools.common.exceptions import ELFError
        symbols = linked(elf, image, set(helpers.values()))
    except ImportError as error:
        raise ValueError("pyelftools is required for ELF validation") from error
    except ELFError as error:
        raise ValueError(f"invalid ELF: {error}") from error
    result = score(uart, symbols, helpers)
    for journal in result["rows"]:
        for row in journal:
            if row[2] == 3:
                offset = row[6] - 0x80000
                instruction = int.from_bytes(image[offset:offset+4], "little")
                need(instruction & 0xffffffe0 == 0x10000000, "work PC is not the linked ADR-self probe")
    need(type(session.get("elapsed_ns")) is int and type(session.get("quiet_ns")) is int
         and 1_000_000_000 <= session["quiet_ns"] <= session["elapsed_ns"] <= 30_000_000_000,
         "capture ceiling/quiet evidence missing")
    result.update(run_id=session["run_id"], mode=session["mode"], session_sha256=session_digest,
                  artifacts=files, software="incomplete" if result["software_gaps"] else "observed",
                  external=(external_score(external,result["expected_external"]) if external is not None
                            else "independent CPU-tagged exception history required"),
                  qualification="separate cold-run and platform evidence required")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("session_sha256", help="trusted digest from the retained acquisition ledger")
    args = parser.parse_args()
    try:
        result = replay(args.folder,args.session_sha256)
    except (ImportError, KeyError, OSError, TypeError, ValueError, IndexError) as error:
        print(json.dumps({"outcome":"unverified","reason":str(error)}))
        return 1
    print(json.dumps(result, indent=2))
    return int(bool(result["software_gaps"]))


if __name__ == "__main__":
    sys.exit(main())
