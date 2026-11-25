"""Compare ELF64 LE executable PT_LOAD descriptions and bytes for two ordinary builds."""
from pathlib import Path
import hashlib
import struct
import sys


def loads(name):
    raw = Path(name).read_bytes()
    if raw[:6] != b"\x7fELF\x02\x01":
        raise ValueError("expected ELF64 little endian")
    offset, = struct.unpack_from("<Q", raw, 32)
    size, count = struct.unpack_from("<HH", raw, 54)
    result = []
    for i in range(count):
        typ, flags, off, va, pa, fs, ms, align = struct.unpack_from("<IIQQQQQQ", raw, offset + size*i)
        if typ == 1:
            if off + fs > len(raw) or fs > ms:
                raise ValueError("invalid PT_LOAD range")
            result.append((flags, va, pa, fs, ms, align, hashlib.sha256(raw[off:off+fs]).hexdigest()))
    if not result:
        raise ValueError("missing PT_LOAD")
    return result


if __name__ == "__main__":
    before, after = map(loads, sys.argv[1:])
    print("baseline", before, "\ncandidate", after)
    sys.exit(before != after)
