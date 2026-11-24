"""QEMU 6.2 single-vCPU brackets around linked weighted CNTPCT_EL0 reads."""
import json
import struct

from native_el0_replay import elf_file, need, symbol
from ordinary_amc_capture import save
from ordinary_amc_replay import bounded, obj
from qemu_capture import request

CAPACITY = 2048
HEADER = struct.Struct("<4Q")
ROW = struct.Struct("<6Q")


def marker_sites(blob):
    elf = elf_file(blob)
    vectors, size = symbol(elf, "native_vruntime_vectors")
    eret, _ = symbol(elf, "native_vruntime_eret")
    need(size == 2048 and vectors + size < eret <= vectors + 8192, "vector extent")
    code = [s for s in elf.iter_segments() if s["p_type"] == "PT_LOAD"
            and s["p_flags"] & 1 and s["p_vaddr"] <= vectors
            and eret + 4 <= s["p_vaddr"] + s["p_filesz"]]
    need(len(code) == 1, "unique linked vector text")
    raw = code[0].data()
    base = code[0]["p_vaddr"]

    def word(pc):
        need(vectors <= pc <= eret and pc % 4 == 0, "aligned vector text")
        return struct.unpack_from("<I", raw, pc - base)[0]

    returns = [pc for pc in range(vectors + size, eret, 4) if word(pc) == 0xD53BE031]
    need(len(returns) == 1 and word(returns[0] + 4) == 0xF9000211, "return MRS x17/store")
    reads = returns[:]
    for slot in (0x400, 0x480):
        branch = word(vectors + slot)
        need(branch >> 26 == 5, "vector direct branch")
        displacement = branch & 0x3ffffff
        if displacement & (1 << 25):
            displacement -= 1 << 26
        pc = vectors + slot + displacement * 4 + 12
        need(word(pc - 4) == 0xD5033FDF and word(pc) == 0xD53BE020
             and word(pc + 4) == (0xD2800001 | (slot << 5)), "entry ISB/MRS x0/vector offset")
        reads.append(pc)
    sites = [(p, word(p)) for pc in reads for p in (pc, pc + 4)]
    need(len(set(p for p, _ in sites)) == 6, "distinct marker PCs")
    return sites


def plugin_args(root, sites):
    need("," not in str(root), "plugin output path contains comma")
    fields = [str(root / "marker.so"), "out=" + str(root / "markers.bin")]
    fields.extend(f"site{i}={pc:x}:{word:x}" for i, (pc, word) in enumerate(sites))
    return ["-S", "-plugin", ",".join(fields)]


class MarkerObserver:
    def __init__(self, root):
        self.root = root
        self.cpu = None

    def start(self, pipe, pid):
        cpus = request(pipe, "query-cpus-fast")
        need(len(cpus) == 1 and cpus[0]["cpu-index"] == 0, "one marker vCPU")
        self.cpu = cpus[0]
        request(pipe, "cont")

    def sample(self):
        pass

    def finish(self):
        save(self.root / "marker-cpu.json", self.cpu)


def marker_report(folder, grants):
    raw = bounded(folder / "markers.bin", HEADER.size + CAPACITY * ROW.size)
    need(len(raw) == HEADER.size + CAPACITY * ROW.size, "marker extent")
    count, invalid, rw, rc = HEADER.unpack_from(raw)
    need(not invalid and count == 4 * len(grants) <= CAPACITY and count, "complete marker sequences")
    need(not any(raw[HEADER.size + count * ROW.size:]), "uncommitted marker rows")
    ident = json.loads(bounded(folder / "marker-cpu.json", 65536), object_pairs_hook=obj)
    need(ident["cpu-index"] == 0 and ident["thread-id"] > 0, "QMP vCPU identity")
    need(rw > 0 and rc > 0, "positive clock resolutions")
    qualified = max(rw, rc) <= 100000
    rows = list(ROW.iter_unpack(raw[HEADER.size:HEADER.size + count * ROW.size]))
    last_wall = last_cpu = 0
    for _, cpu, tid, w0, c, w1 in rows:
        need(cpu == 0 and tid == ident["thread-id"], "marker/QMP thread mismatch")
        need(last_wall <= w0 <= w1 and last_cpu <= c, "nonmonotone marker clock")
        last_wall, last_cpu = w1, c

    intervals = []
    for index, grant in enumerate(grants):
        a, b, c, d = rows[4*index:4*index+4]
        stop = 4 if grant["cause"] == "irq" else 2
        need([r[0] for r in (a, b, c, d)] == [0, 1, stop, stop + 1], "journal marker order")
        widths = [b[5] - a[3] + 2*rw, d[5] - c[3] + 2*rw]
        wall = [max(0, c[3] - b[5] - 2*rw), d[5] - a[3] + 2*rw]
        cpu = [max(0, c[4] - b[4] - 2*rc), d[4] - a[4] + 2*rc]
        allowance = (wall[1] + 999) // 1000
        consistent = cpu[0] <= wall[1] + allowance
        qualified &= max(widths) <= 1000000 and consistent
        outside = [max(0, wall[0] - cpu[1] - allowance),
                   max(0, wall[1] - cpu[0] + allowance)] if consistent else None
        intervals.append(dict(lease=grant["lease"], cause=grant["cause"], start_tick=grant["start"],
                              compare_tick=grant["compare"], entry_tick=grant["stop"],
                              bracket_ns=widths, wall_ns=wall, cpu_ns=cpu, outside_cpu_ns=outside))
    return dict(qualified=qualified, tid=ident["thread-id"], resolutions_ns=[rw, rc], intervals=intervals)
