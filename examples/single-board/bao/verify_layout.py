"""Check the raw QEMU load address against Bao's actual link-time image positions."""
from pathlib import Path
import re
import sys

directory = Path(sys.argv[1])
binary = (directory / "bao.bin").read_bytes()
report = (directory / "bao.elf.txt").read_text()
symbols = {}
for line in report.splitlines():
    match = re.match(r"\s*\d+:\s+([0-9a-fA-F]+)\s+\d+\s+\S+\s+\S+\s+\S+\s+\S+\s+(\S+)\s*$", line)
    if match:
        symbols[match[2]] = int(match[1], 16)

load_pa = 0x40080000
link_base = symbols["_image_start"]
if binary[56:60] == b"ARM\x64":
    raise ValueError("QEMU may select a different load offset for an arm64 Image header")
# Include Bao's BSS and bootstrap tables after the embedded images, then allow
# two MiB for both CPU structures, their private page tables and the RAM bitmap.
hyp_end = load_pa + symbols["_dmem_phys_beg"] - link_base
if hyp_end + 0x200000 > 0x48000000:
    raise ValueError(f"Bao bootstrap exceeds host reservation (static end {hyp_end:#x})")

for name, ipa, physical, limit in (
    ("linux", 0x50000000, 0x50000000, 0x5c000000),
    ("jnox", 0x40200000, 0x48200000, 0x50000000),
):
    offset = symbols[f"_{name}_guest_vm_beg"] - link_base
    size = symbols[f"_{name}_guest_vm_size"]
    payload = (Path("configs/domains") / f"{name}.bin").read_bytes()
    if offset < 0 or offset % 4096 or not size or size != len(payload):
        raise ValueError(f"{name} linker symbol mismatch")
    if binary[offset:offset + size] != payload:
        raise ValueError(f"{name} linked bytes differ from guest payload")
    src = load_pa + offset
    if physical + size > limit:
        raise ValueError(f"{name} exceeds its RAM region")
    # v2's unchanged vm_install_image compares the source PA against the guest
    # IPA. Also verify the actual backing PA used by mem_map_cpy.
    for dest in (ipa, physical):
        if src < dest + size and dest < src + size:
            raise ValueError(f"{name} source {src:#x} overlaps {dest:#x}, size {size:#x}")
    print(f"{name} offset={offset:#x} source_pa={src:#x} guest_ipa={ipa:#x} "
          f"dest_pa={physical:#x} bytes={size:#x}")
