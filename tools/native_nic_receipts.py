"""Read only the fixture's declared process-private receipts using saved AArch64 page walks."""
import hashlib
import json

from native_amc_roots import linked_roots
from qemu_capture import journal_location
from verify_native_uart_recovery import walk


ELFS = ("user.elf", "native_peer.elf", "native_peer2.elf")
SYMBOLS = (
    ("dma_phase", "dma_echo_count"),
    ("dma_phase", "dma_peer_ticks", "dma_peer_ns", "dma_peer_finished", "dma_peer_rx", "dma_peer_tx", "dma_peer_claim"),
    ("dma_phase", "dma_echo_count", "dma_spare_preclaim", "dma_spare_claim", "dma_spare_duplicate"),
)


def layout(pkg):
    kernel = pkg / "kernel.elf"
    raw = kernel.read_bytes()
    manifest = json.loads((pkg / "manifest.json").read_bytes())
    if hashlib.sha256(raw).hexdigest() != manifest["kernel_elf_sha256"]:
        raise ValueError("kernel manifest digest")
    roots = linked_roots(kernel, 3)
    probes = []
    for idx, name in enumerate(ELFS):
        elf = pkg / name
        image = elf.read_bytes()
        slot = manifest["slots"][idx]
        if (slot["slot"] != idx or slot["elf"] != name or slot["expected_ttbr0_el1"] != roots[idx]
                or slot["elf_sha256"] != hashlib.sha256(image).hexdigest() or image not in raw):
            raise ValueError(f"slot {idx} package mismatch")
        for sym in SYMBOLS[idx]:
            va, size = journal_location(elf, sym)
            if size != 8 or va % 8 or not 0x400000 <= va < 0x800000:
                raise ValueError(f"invalid receipt {sym}")
            probes.append((idx, sym, roots[idx], va))
    return probes


def decode(probes, raw, fetch=None):
    used = set()
    def read(pa, size):
        key = str(pa)
        if key not in raw and fetch is not None:
            raw[key] = fetch(pa, size).hex()
        data = bytes.fromhex(raw[key])
        if len(data) != size:
            raise ValueError("short physical receipt")
        used.add(key)
        return data

    output = [{}, {}, {}]
    private_pages = [set(), set(), set()]
    for slot, sym, root, va in probes:
        pa = walk(root, va, read)
        private_pages[slot].add(pa >> 12)
        signed = sym in {"dma_peer_rx", "dma_peer_tx", "dma_peer_claim", "dma_spare_preclaim",
                         "dma_spare_claim", "dma_spare_duplicate"}
        output[slot][sym] = int.from_bytes(read(pa, 8), "little", signed=signed)
    if used != set(raw) or any(private_pages[a] & private_pages[b] for a, b in ((0, 1), (1, 2), (0, 2))):
        raise ValueError("extraneous physical reads or process receipt aliases")
    return output
