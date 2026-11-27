"""Native AMC virt TTBR0 identities from the final linked kernel's Space array."""
import re
import subprocess


def private_spaces(name):
    return "native_el0_mmu" in name and "6SPACES" in name


def expected_roots(objects, count):
    # native_el0_mmu.rs: repr(C) Space begins with a page-aligned root table.
    # SPACE_CAPACITY is the packaged image count and root_id uses ASID pid+1.
    matches = [(address, size) for name, address, size in objects if private_spaces(name)]
    if len(matches) != 1 or not 1 <= count <= 5:
        raise ValueError("missing or ambiguous linked native MMU Space array")
    base, size = matches[0]
    stride, remainder = divmod(size, count)
    if (remainder or stride < 4096 or stride % 4096 or base % 4096 or
            base < 0x40200000 or base + size > 0x48000000):
        raise ValueError("invalid linked native virt Space array extent")
    return [((slot + 1) << 48) | (base + slot * stride) for slot in range(count)]


def linked_roots(kernel, count):
    symbols = subprocess.check_output(["aarch64-linux-gnu-nm", "-S", "--defined-only", str(kernel)], text=True)
    objects = []
    for line in symbols.splitlines():
        match = re.fullmatch(r"([0-9a-fA-F]+)\s+([0-9a-fA-F]+)\s+[bB]\s+(\S+)", line.strip())
        if match and private_spaces(match[3]):
            objects.append((match[3], int(match[1], 16), int(match[2], 16)))
    return expected_roots(objects, count)
