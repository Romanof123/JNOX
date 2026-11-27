"""Check the actual guest UART receipt and exercise independent single-field mutations."""
import argparse
import re
from pathlib import Path

# Native ABI values. The wrapped fcntl caller emits these before sending its real
# read-only alias through the existing SCM_RIGHTS/display transfer scenario.
EXPECTED = {
    "minimum_101": 101, "lowest_after_96": 97, "cloexec": 1, "ordinary_flags": 0,
    "shared_seal": 2, "negative_min_errno": 22, "readonly_write_errno": 13,
    "closed_source_errno": 9, "table_full_errno": 24, "reused_hole": 98,
    "map_after_close": 165, "socket_dup_errno": 95, "closed_socket_errno": 9,
    "original_right": 98, "outgoing_alias": 97, "outgoing_cloexec": 1,
}

def check(data):
    rows = re.findall(rb"(?m)^JNOX DUPFD ([0-9a-f]{32})\r?$", data)
    if len(rows) != 1:
        raise ValueError("missing or repeated producer receipt")
    actual = dict(zip(EXPECTED, bytes.fromhex(rows[0].decode()), strict=True))
    for key, value in EXPECTED.items():
        if actual[key] != value:
            raise ValueError(key)
    for pid, status in ((0,70),(1,71),(2,72)):
        pattern = rb"(?m)^NATIVE EL0 EXIT PID %d %d\r?$" % (pid,status)
        if not re.search(pattern, data):
            raise ValueError(f"missing final exit {pid}/{status}")
    return rows[0]

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart", type=Path)
    parser.add_argument("--mutations", action="store_true")
    args = parser.parse_args()
    data = args.uart.read_bytes()
    original = check(data)
    if args.mutations:
        for i, field in enumerate(EXPECTED):
            altered = bytearray.fromhex(original.decode())
            altered[i] ^= 1
            changed = data.replace(b"JNOX DUPFD " + original, b"JNOX DUPFD " + altered.hex().encode())
            try:
                check(changed)
            except ValueError as err:
                if str(err) != field:
                    raise
            else:
                raise ValueError(f"accepted mutation of {field}")
    print("Verified 16 guest observations and all three terminal client exits.")
