"""Derive and verify a native Xwayland runtime ELF from the immutable link receipt."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

REQUIRED = {"dix_main", "InitNativeAuthorization", "XkmReadFile", "jnox_selected_xkb_identify",
            "jnox_socket_broker", "wl_display_connect", "xwl_shm_create_pixmap"}
NOTE = struct.pack("<III8sI", 5, 4, 1, b"JNOX", 1)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def layout(data):
    if data[:7] != b"\x7fELF\x02\x01\x01":
        raise ValueError("expected a little-endian ELF64 executable")
    (kind, arch, version, entry, phoff, shoff, flags, ehsize, phsize, phnum, shsize, shnum,
     shstr) = struct.unpack_from("<HHIQQQIHHHHHH", data, 16)
    if (kind, arch, version, entry, flags, ehsize, phsize, phnum, shsize) != (
            2, 183, 1, 0x400000, 0, 64, 56, 3, 64) or not 0 <= shstr < shnum <= 128:
        raise ValueError("unexpected JNOX AArch64 executable header")
    loads = []
    previous = entry
    for i, required in enumerate((5, 4, 6)):
        ptype, perms, offset, addr, phys, filesz, memsz, align = struct.unpack_from(
            "<IIQQQQQQ", data, phoff + i * phsize)
        if (ptype, perms, align) != (1, required, 4096) or addr != phys or addr % align or offset % align:
            raise ValueError("unexpected native load segment")
        if addr < previous or addr + memsz > 0x800000 or not 0 < filesz <= memsz or offset + filesz > len(data):
            raise ValueError("invalid native segment extent")
        if i == 0 and addr != entry:
            raise ValueError("entry does not start the first executable segment")
        previous = (addr + memsz + align - 1) // align * align
        loads.append(dict(va=addr, flags=perms, filesz=filesz, memsz=memsz, align=align,
                          sha256=sha(data[offset:offset + filesz])))
        if required == 4:
            ro_addr, ro_offset, ro_size = addr, offset, filesz
    sections = [struct.unpack_from("<IIQQQQIIQQ", data, shoff + i * shsize) for i in range(shnum)]
    names_offset, names_size = sections[shstr][4:6]
    if sections[shstr][1] != 3 or names_offset + names_size > len(data):
        raise ValueError("invalid ELF section name table")
    names = data[names_offset:names_offset + names_size]
    notes = []
    for index, skind, sflags, addr, offset, size, *_ in sections:
        if names[index:names.index(b"\0", index)] == b".note.jnox.abi":
            delta = addr - ro_addr
            if ((skind, sflags, size) != (7, 2, len(NOTE)) or delta < 0 or delta + size > ro_size
                    or offset != ro_offset + delta or data[offset:offset + size] != NOTE):
                raise ValueError("invalid native ABI note")
            notes.append(addr)
    if len(notes) != 1:
        raise ValueError("expected exactly one native ABI note")
    return {"entry": entry, "abi_note_va": notes[0], "loads": loads}


def code_symbols(elf):
    rows = subprocess.check_output(["llvm-nm", "-P", "--defined-only", elf], text=True).splitlines()
    code = sorted(row for row in rows if len(row.split()) >= 3 and row.split()[1] in ("T", "t", "W", "w"))
    if not REQUIRED <= {row.split()[0] for row in code}:
        raise ValueError("required Xwayland diagnostic symbols absent")
    return code


def derive(source, dest, expected):
    data = source.read_bytes()
    if sha(data) != expected:
        raise ValueError("server differs from the normal verified Xwayland build receipt")
    expected_layout = layout(data)
    with tempfile.TemporaryDirectory(dir=dest.parent) as temp:
        original, derived = Path(temp) / "original", Path(temp) / "derived"
        original.write_bytes(data)
        subprocess.run(["llvm-objcopy", "--strip-debug", original, derived], check=True)
        runtime = derived.read_bytes()
        symbols = code_symbols(original)
        if layout(runtime) != expected_layout or code_symbols(derived) != symbols:
            raise ValueError("derived ELF changed native loads, entry, ABI note or code symbols")
    if len(runtime) > 8 * 1024 * 1024:
        raise ValueError("derived ELF exceeds the native package file limit")
    with dest.open("xb") as output:
        output.write(runtime)
    return dict(original_sha256=expected, original_bytes=len(data), runtime_sha256=sha(runtime),
                runtime_bytes=len(runtime), code_symbols=len(symbols), **expected_layout)


def from_receipt(server_dir, dest):
    target = "hw/xwayland/Xwayland"
    fields = (server_dir / "Xwayland.sha256").read_text().split()
    if len(fields) != 2 or fields[1] != target:
        raise ValueError("unexpected server link receipt")
    return derive(server_dir / "xwayland-build" / target, dest, fields[0])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("server_dir", type=Path, help="xkb-build.py output containing Xwayland.sha256")
    parser.add_argument("dest", type=Path, help="new ELF file in an existing directory")
    args = parser.parse_args()
    print(json.dumps(from_receipt(args.server_dir, args.dest), indent=2))
