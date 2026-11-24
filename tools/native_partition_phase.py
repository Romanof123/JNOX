"""Compare a matched pair of ordinary and irq-v1 native P/Q cold captures."""
import argparse
import json
import struct
from pathlib import Path

from native_partition_replay import Snapshot, bounded, captured, elf_file, need, replay, symbol

PHASE = "native_partition_irq_phase_v1"
FIELDS = ("irq", "observe_begin", "result_begin", "audit_begin", "audit_end",
          "schedule_begin", "mmu_begin", "publish_begin", "publish_end", "selected")


def compare(plain, plain_sha, phase, phase_sha):
    a, b = replay(plain, plain_sha), replay(phase, phase_sha)
    meta_a, meta_b = captured(plain, plain_sha), captured(phase, phase_sha)
    keys = ("revision", "arm", "clock", "user_sha256", "probe_sha256", "qemu_sha256",
            "qemu_version", "ram_base")
    need(all(meta_a[k] == meta_b[k] for k in keys), "matched launch and user image")
    need(a["frequency"] == b["frequency"], "matched counter frequency")
    frame_profiles = []
    for folder, expected in ((plain, False), (phase, True)):
        kernel = elf_file(bounded(folder / "kernel.elf", 64 << 20))
        syms = kernel.get_section_by_name(".symtab").get_symbol_by_name(PHASE)
        frame_profiles.append(bool(kernel.get_section_by_name(".symtab").get_symbol_by_name(
            "native_partition_traps_v1")))
        need(bool(syms) == expected, "plain/phase symbol identity")
    need(frame_profiles[0] == frame_profiles[1], "matched frame profile")
    addr, size = symbol(kernel, PHASE)
    need(size == 12 * 17 * 8, "phase v1 extent")
    mem = Snapshot(meta_b["ram_base"], bounded(phase / "ram.bin", 16 << 20))
    rows = list(struct.iter_unpack("<17Q", mem.read(addr, size)))
    windows = b["windows"]
    transitions = []
    for index, (previous, current) in enumerate(zip(windows, windows[1:])):
        row = rows[index]
        need(row[:3] == (previous["generation"], previous["pid"], previous["irq_entry"])
             and row[7:10] == (0x480, current["generation"], current["pid"])
             and row[14:] == (current["selected"], current["release"], current["close"]),
             "phase identity and journal boundaries")
        stamps = row[2:7] + row[10:15]
        need(tuple(sorted(stamps)) == stamps, "ordered raw phase samples")
        def intervals(report):
            prev, cur = report["windows"][index:index + 2]
            return dict(irq_to_selection=cur["selected"]-prev["irq_entry"],
                        selection_to_return=cur["return_sample"]-cur["selected"],
                        return_to_user=cur["return_bracket"], opening_to_user=cur["latency"],
                        margin=report["frequency"]//1000-cur["latency"])
        pa, pb = intervals(a), intervals(b)
        transitions.append(dict(old_generation=row[0], old_pid=row[1], new_generation=row[8],
                                new_pid=row[9], raw=dict(zip(FIELDS, stamps)), plain=pa, phase=pb,
                                delta={key: pb[key]-value for key, value in pa.items()}))
    need(not any(any(row) for row in rows[len(transitions):]), "zero unused phase rows")
    return dict(plain_sha256=plain_sha, phase_sha256=phase_sha, arm=b["arm"], clock=b["clock"],
                frequency=b["frequency"], plain_misses=a["timing_misses"],
                phase_misses=b["timing_misses"], transitions=transitions)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for side in ("plain", "phase"):
        parser.add_argument(side, type=Path)
        parser.add_argument(side + "_sha256")
    args = parser.parse_args()
    print(json.dumps(compare(args.plain, args.plain_sha256, args.phase, args.phase_sha256), indent=2))
