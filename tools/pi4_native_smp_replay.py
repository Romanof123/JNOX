#!/usr/bin/env python3
"""Read a Pi native SMP session without attaching to a running CPU."""
import argparse
import itertools
import json
import re
import struct
import sys
from pathlib import Path

from ordinary_amc_capture import sha

EXPECTED = [(78498, 12), (70435, 77), (32, 32)]
WORK = ["count_low_primes", "count_high_primes"]
PAGE_MASK = 0xfffff000


def require(value, message):
    if not value:
        raise ValueError(message)


def one(items):
    a, = items
    return a


class Ram:
    def __init__(self, root, base):
        self.base = base
        self.data = (root / "ram.bin").read_bytes()

    def words(self, addr, count):
        off = addr - self.base
        require(off >= 0 and off + 8 * count <= len(self.data), "out-of-range physical RAM")
        return struct.unpack_from(f"<{count}Q", self.data, off)

    def walk(self, ttbr, va):
        table = ttbr & PAGE_MASK
        for shift in (30, 21, 12):
            entry, = self.words(table + ((va >> shift) & 511) * 8, 1)
            if entry & 3 != 3:
                return 0, 0
            table = entry & PAGE_MASK
        return table + (va & 4095), entry


def symbol(root, name, elf):
    lines = (root / (elf + ".nm")).read_text().splitlines()
    a, size, _, _ = one([line.split() for line in lines if line.split()[-1:] == [name]])
    return int(a, 16), int(size, 16)


def inspect(a):
    root = a.capture
    require(sha(root / "session.json") == a.session_sha256, "session digest")
    session = json.loads((root / "session.json").read_bytes())
    for name, digest in session["files"].items():
        require(Path(name).name == name and sha(root / name) == digest, f"artifact digest {name}")
    require(session["reason"] == "terminal", session["reason"])
    manifest = json.loads((root / "manifest.json").read_bytes())
    launch = json.loads((root / "launch.json").read_bytes())
    require(sha(root / "kernel8.img") == manifest["kernel8_img_sha256"], "raw image hash")
    slots = manifest["slots"]
    for slot in slots:
        require(sha(root / slot["elf"]) == slot["elf_sha256"], "slot hash")
    ram = Ram(root, launch["ram_base"])
    uart = (root / "uart.bin").read_text("ascii")
    def rows(tag):
        return [tuple(map(int, s.split())) for s in re.findall(
            rf"(?m)^NATIVE SMP {tag} ([0-9 ]+)$", uart)]
    n = len(slots)
    exits = sorted((int(i), int(code)) for i, code in re.findall(
        r"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)$", uart))
    require(exits == [(i, exitcode) for i, (_, exitcode) in enumerate(EXPECTED[:n])], "signed exits")
    if manifest["policy"] == "fifo":
        space = one([line.split() for line in (root / "kernel.elf.nm").read_text().splitlines()
                     if len(line.split()) == 4 and "native_el0_mmu" in line and "SPACES" in line])
        base, size = [int(x, 16) for x in space[:2]]
        roots = [base + i * (size // n) for i in range(n)]
    else:
        tasks = sorted(rows("TASK"))
        require([t[0] for t in tasks] == list(range(n)), "terminal task slots")
        roots = [t[5] for t in tasks]
    products = []
    backing = set()
    for i, slot in enumerate(slots):
        va, size = symbol(root, "private_result", slot["elf"])
        pa, desc = ram.walk(roots[i], va)
        require(size == 4 and pa and desc & 64 and desc & (1 << 11) and not desc & 128,
                "writable private result")
        require(pa not in backing, "aliased result")
        backing.add(pa)
        product = struct.unpack_from("<I", ram.data, pa-ram.base)[0]
        require(product == EXPECTED[i][0], f"slot {i} private result")
        products.append(dict(slot=i, va=va, pa=pa, value=product))
    if manifest["policy"] == "fifo":
        return dict(products=products, exits=exits, legacy=True,
                    secondary_execution="not observed by terminal RAM")
    version, freq, q, epoch, count, idle = one(rows("CONFIG"))
    require(version == 1 and count == n == idle and q > 0 and freq > 0, "configuration")
    require(one(rows("END")) == (1, (1 << n)-1, 1, 1), "terminal rendezvous")
    before, sample, after = one(rows("CAL"))
    require(before <= after and sample > 0, "clock bracket")
    lo, hi = before-sample, after-sample
    masks = [s["cpu_mask"] for s in slots]
    require(n in (2, 3) and sorted(set(masks)) == [1, 2], "affinity")
    require(len(set(r & PAGE_MASK for r in roots)) == n, "root alias")
    stacks = rows("STACK")
    require(len(stacks) == 2 and len(set(itertools.chain.from_iterable(
        (s[0], s[3]) for s in stacks))) == 4, "distinct stack guards")
    for guard, bottom, top in [s[i:i+3] for s in stacks for i in (0, 3)]:
        for r in roots:
            require(bottom == guard+4096 and top == bottom+16384 and not ram.walk(r,guard)[1],
                    "guard mapping")
            for va in range(bottom,top,4096):
                pa, desc = ram.walk(r, va)
                require(pa == va and not desc & 64 and desc & (1 << 53), "kernel stack mapping")
    seen = [0]*n
    released = 0
    allrows = []
    productive = [[], []]
    continuations = [None]*n
    uncertainty = []
    for cpu, journal in enumerate(sorted(rows("JOURNAL"))):
        c, addr, used, words, dropped, frames, fsize = journal
        require(c == cpu < 2 and 0 < used <= 256 and words == 37 and fsize == 816,
                "journal layout")
        require(dropped == 0, "journal loss")
        owner, previous = idle, 0
        for i in range(used):
            r = ram.words(addr + i*words*8, words)
            f = ram.words(frames + i*2*fsize, 102)
            g = ram.words(frames + (i*2+1)*fsize, 102)
            src, dst = r[6:8]
            require(r[:2] == (cpu,i) and src == owner and previous <= r[4], "owner chain")
            require(src <= idle and dst <= idle and r[4] <= r[5], "event bounds")
            require(not released & r[14] and (not r[14] or cpu == 0), "double/foreign release")
            released |= r[14]
            require(stacks[cpu][1] <= r[29] <= stacks[cpu][2], "handler stack")
            require(r[34] == r[11] and r[35]&3 == 1, "banked timer rearm")
            if r[2] == 1 and r[3]&1023 == 30:
                require(r[32] == r[10] and r[33]&7 == 5, "asserted physical comparator")
            if src < n:
                require(masks[src] == 1 << cpu and r[21] == roots[src] and
                        roots[src] >> 48 == src+1 and r[8] >= previous and
                        r[9] == r[4]-r[8], "source context or service")
                seen[src] += r[9]
                require(seen[src] == r[28], "cumulative service")
                pc, sp = f[32], f[31]
                require((pc, f[33]) == r[30:32] and f[33] & 31 == 0 and
                        0x01001000 <= sp <= 0x01011000 and sp % 16 == 0, "EL0 frame")
                pcpa, desc = ram.walk(roots[src], pc)
                require(pcpa and desc & 64 and desc & 128 and not desc & (1<<54),
                        "private executable mapping")
                ret = list(f)
                if r[2] == 2:
                    require(f[8] == r[3] and f[0] == r[23], "SVC arguments")
                    ret[0] = r[24]
                else:
                    require(r[2] == 1, "exception kind")
                continuations[src] = tuple(ret)
                if src < 2 and r[2] == 1 and r[3] & 1023 == 30:
                    start, length = symbol(root, WORK[src], slots[src]["elf"])
                    if start <= pc < start+length:
                        productive[src].append((cpu,i,r[8],r[4],pc,f))
            if dst < n:
                require(masks[dst] == 1 << cpu and r[22] == roots[dst], "destination TTBR")
                if continuations[dst] is not None:
                    require(g == continuations[dst], "saved vs selected frame (including TLS/SIMD)")
            owner, previous = dst, r[5]
            allrows.append(r)
    require(len(rows("JOURNAL")) == 2 and released == (1<<n)-1 and
            all(t[4] == seen[i] and masks[i] == 1<<t[1] and t[3] == slots[i]["release_cell"]
                for i,t in enumerate(tasks)), "terminal accounting")
    pairs = [[], []]
    for pid, samples in enumerate(productive):
        if len(samples) < 2 or not a.candidate_register:
            uncertainty.append(f"slot {pid}: missing successive productive PPI samples or linked index register")
            continue
        reg = a.candidate_register[pid]
        for x,y in zip(samples,samples[1:]):
            least = (1, 1000001)[pid]
            greatest = (1000000, 2000000)[pid]
            first, last = x[5][reg], y[5][reg]
            if y[1] == x[1]+1 and least <= first < last < greatest and first%2 and last%2:
                pairs[pid].append((x,y))
    def bounds(x):
        return (x[3]+lo,x[3]+hi) if x[0] else (x[3],x[3])
    overlap = None
    for (x,y),(v,w) in itertools.product(*pairs):
        for a0,a1,b0,b1 in ((x,y,v,w),(v,w,x,y)):
            # The opposite core must have returned to EL0 before the inner sample.
            a_resume = a1[2] + (hi if a1[0] else 0)
            b_resume = b1[2] + (hi if b1[0] else 0)
            if (max(bounds(a0)[1], a_resume) < bounds(b0)[0]
                    and max(bounds(b0)[1], b_resume) < bounds(a1)[0]
                    and bounds(a1)[1] < bounds(b1)[0]):
                overlap = [[s[0],s[1],s[2],s[3],s[4]] for s in (a0,b0,a1,b1)]
                break
        if overlap:
            break
    if not overlap:
        uncertainty.append("no unambiguous interleaved productive PPI observations")
    if n == 3:
        sender = [r for r in allrows if r[14]&4 and r[16]]
        receive = [r for r in allrows if r[0]==1 and r[2]==1 and r[3]&8191==1 and r[17]&4 and r[6:8]==(1,2)]
        svc = [r for r in allrows if r[0]==1 and r[6]==2 and r[2]==2]
        sleep = [r for r in svc if r[3]==7 and r[7]==1 and r[24]==0]
        wake = [r for r in allrows if r[0]==1 and r[19]&4 and r[6:8]==(1,2)]
        if not (len(sender)==len(receive)==len(sleep)==len(wake)==1 and svc and svc[0][3]==6
                and epoch+q<=sender[0][15]<=sender[0][16]<=receive[0][4]+lo
                and receive[0][4]<=svc[0][4] and svc[0][4]+hi<epoch+2*q
                and sleep[0][4]<wake[0][4] and wake[0][9]>0
                and any(r[3]==1 and r[7]==1 for r in svc)):
            uncertainty.append("remote release, sleep/wake and return chronology incomplete")
    trace_irqs = None
    if launch["trace"]:
        trace_irqs = [0,0]
        with (root / "exceptions.log").open(errors="replace") as trace:
            for line in trace:
                hit = re.fullmatch(r"Taking exception [0-9]+ \[IRQ\] on CPU ([01])\s*", line)
                if hit:
                    trace_irqs[int(hit[1])] += 1
        if not all(trace_irqs):
            uncertainty.append("external CPU-indexed IRQ headers missing")
    return dict(products=products, exits=exits, service=seen, clock_offset=[lo,hi],
                external_cpu_irq_headers=trace_irqs,
                productive_ppi=[len(p) for p in productive], overlap=overlap,
                unresolved=uncertainty, nonzero_tls_control="separate control required")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("capture", type=Path)
    p.add_argument("--session-sha256", required=True)
    p.add_argument("--candidate-register", type=int, nargs=2, choices=range(31),
                   metavar=("LOW", "HIGH"), help="candidate registers from linked worker disassembly")
    try:
        result = inspect(p.parse_args())
        print(json.dumps(result, indent=2))
        sys.exit(2 if result.get("unresolved") else 0)
    except (OSError, ValueError, KeyError, IndexError, struct.error) as exc:
        sys.exit(f"Invalid or incomplete evidence: {exc}")
