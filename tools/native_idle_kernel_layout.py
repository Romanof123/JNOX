"""Resolve only the immutable three-slot wake kernel's private capture layout."""
import re


# NativeFrame is 832 bytes; each Space is 0x415000 bytes on this kernel.
OBJECTS = {
    "spaces": ("_ZN11jnox_kernel14native_el0_mmu6SPACES", 3*0x415000, "b"),
    "frames": ("_ZN11jnox_kernel10native_el06FRAMES", 3*832, "d"),
}


def layout(items):
    """items yields (full name, address, byte size, storage class)."""
    found = {}
    for name, address, size, storage in items:
        key = name if name in ("_start", "__stack_top") else None
        for label,(path,expected,kind) in OBJECTS.items():
            if re.fullmatch(path + r"(?:17h[0-9a-f]{16})?E(?:\.llvm\.[0-9]+)?", name):
                if storage != kind or size != expected:
                    raise ValueError(f"incorrect {label} storage class or size")
                key = label
        if key is not None:
            if key in found:
                raise ValueError(f"ambiguous {key}")
            found[key] = address
    if set(found) != {"_start","__stack_top",*OBJECTS}:
        raise ValueError("missing wake kernel capture symbols: " +
                         ", ".join(sorted({"_start","__stack_top",*OBJECTS} - found.keys())))
    for key,(_,size,_) in OBJECTS.items():
        if not found["_start"] <= found[key] <= found["__stack_top"]-size:
            raise ValueError(f"{key} outside captured RAM")
    return found


def nm_layout(text):
    def items():
        for line in text.splitlines():
            cols = line.split()
            if len(cols) in (3,4) and re.fullmatch(r"[0-9a-fA-F]+", cols[0]):
                size = int(cols[1],16) if len(cols)==4 else 0
                yield cols[-1],int(cols[0],16),size,cols[-2].lower()
    return layout(items())


def elf_layout(elf):
    symbols = elf.get_section_by_name(".symtab")
    if symbols is None:
        raise ValueError("missing kernel .symtab")
    def items():
        for sym in symbols.iter_symbols():
            section = sym["st_shndx"]
            storage = None
            if sym["st_info"]["type"]=="STT_OBJECT" and isinstance(section,int):
                header = elf.get_section(section).header
                if header["sh_flags"] & 3 == 3:
                    storage = {"SHT_NOBITS":"b","SHT_PROGBITS":"d"}.get(header["sh_type"])
            yield sym.name,sym["st_value"],sym["st_size"],storage
    return layout(items())
