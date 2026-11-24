"""Linked identity for the six MG3 CPU-local and activation-local objects."""
import io
import re


OBJECTS = ("PI4_MIG_CPU0", "PI4_MIG_CPU1", "PI4_MIG_TLS_M", "PI4_MIG_TLS_P0",
           "PI4_MIG_TLS_P1_OPEN", "PI4_MIG_TLS_P1_REPLACEMENT")
IDENTITY = "pi4_mg3_identity"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def declared_wire(raw):
    versions = set(re.findall(rb"MG ([0-9]+) BEGIN", raw))
    require(len(versions) == 1 and versions <= {b"1", b"2", b"3"},
            "missing, unsupported or mixed MG image wire markers")
    return int(versions.pop())


def mg3_symbols(elf, raw, case, workload):
    """Return the ELF-verified MG3 map or None for an earlier wire format."""
    wire = declared_wire(raw)
    if wire != 3:
        require(b"MG3|" not in raw and all(name.encode() + b"\0" not in elf
                for name in (*OBJECTS, IDENTITY)), "MG3 declarations in a legacy image")
        return None
    from elftools.elf.elffile import ELFFile
    e = ELFFile(io.BytesIO(elf))
    require(e.elfclass == 64 and e.little_endian and e["e_machine"] == "EM_AARCH64"
            and e["e_type"] == "ET_EXEC" and e["e_entry"] == 0x80000, "invalid MG3 ELF")
    table = e.get_section_by_name(".symtab")
    require(table is not None, "MG3 requires linked symbols")
    expected = f"MG3|{case}|{workload}".encode("ascii")
    require(raw.count(b"MG3|") == 1, "missing or mixed MG3 image identities")
    result = {}
    ranges = []
    for name in (*OBJECTS, IDENTITY):
        marker = name == IDENTITY
        size = len(expected) if marker else 64
        hits = table.get_symbol_by_name(name) or []
        require(len(hits) == 1, "missing or ambiguous object " + name)
        s = hits[0]
        addr, idx = s["st_value"], s["st_shndx"]
        require(s["st_info"]["type"] == "STT_OBJECT" and s["st_info"]["bind"] == "STB_GLOBAL"
                and s["st_size"] == size and isinstance(idx, int), "invalid MG3 object " + name)
        sec = e.get_section(idx)
        offset = addr - sec["sh_addr"]
        flags = sec["sh_flags"] & 7
        require(flags == (2 if marker else 3) and 0 <= offset <= sec["sh_size"] - size
                and 0x80000 <= addr <= 0x100000-size, "invalid MG3 section for " + name)
        segments = [p for p in e.iter_segments() if p["p_type"] == "PT_LOAD"
                    and p["p_vaddr"] == p["p_paddr"] and p["p_vaddr"] <= addr
                    and addr + size <= p["p_vaddr"] + p["p_memsz"]]
        require(len(segments) == 1, "object outside unique load mapping " + name)
        p = segments[0]
        if marker:
            require(sec["sh_type"] == "SHT_PROGBITS" and
                    addr + size <= p["p_vaddr"] + p["p_filesz"] and
                    sec["sh_offset"] + offset == p["p_offset"] + addr - p["p_vaddr"] and
                    sec.data()[offset:offset+size] == raw[addr-0x80000:addr-0x80000+size]
                    == expected, "MG3 identity disagrees with requested case or workload")
        else:
            segment_offset = addr - p["p_vaddr"]
            file_backed = (sec["sh_type"] == "SHT_PROGBITS" and
                           segment_offset + size <= p["p_filesz"] and
                           p["p_offset"] + segment_offset == sec["sh_offset"] + offset and
                           sec.data()[offset:offset+size] == raw[addr-0x80000:addr-0x80000+size])
            zero_fill = sec["sh_type"] == "SHT_NOBITS" and segment_offset >= p["p_filesz"]
            require(file_backed or zero_fill, "object section and load image disagree for " + name)
            require(sec["sh_type"] in ("SHT_NOBITS", "SHT_PROGBITS") and p["p_flags"] & 2
                    and sec["sh_addralign"] >= 64 and addr % 64 == 0,
                    "unaligned or unwritable MG3 object " + name)
            result[name] = dict(address=addr, size=size, alignment=64)
        ranges.append((addr, addr + size))
    ranges.sort()
    require(all(a[1] <= b[0] for a, b in zip(ranges, ranges[1:])), "overlapping MG3 objects")
    return result
