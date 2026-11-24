"""Exact integer accounting for two native processes with CPU weights one and two."""
from fractions import Fraction

from native_el0_replay import PA_MASK, Snapshot, need

RING = (3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3)
MASK64 = (1 << 64) - 1


def product(task, samples):
    need(
        task in (0, 1) and 0 < samples <= MASK64 and samples % 256 == 0,
        "complete work chunks",
    )
    chunks = samples // 256
    return (
        samples,
        chunks * 16 * sum(RING) & MASK64,
        chunks * 16 * sum((i + 1) * RING[(i + 5 * task) % 16] for i in range(16))
        & MASK64,
    )


class Service:
    """Retain the monotone ready floor and doubled task run times (half-tick units)."""

    def __init__(self, quantum):
        need(quantum > 0 and quantum % 4 == 0, "quarter quantum")
        self.q = quantum
        self.ready = {0, 1}
        self.vr = [0, 0]
        self.service = [0, 0]
        self.floor = 0
        self.gaps = []
        self.misses = []
        self.last_stop = None

    def owner(self):
        return min(self.ready, key=lambda i: (self.vr[i], i))

    def fix_floor(self):
        if self.ready:
            self.floor = max(self.floor, min(self.vr[i] for i in self.ready))

    def interval(self, owner, start, stop):
        need(owner in self.ready and owner == self.owner(), "minimum virtual runtime")
        gap = 0 if self.last_stop is None else start - self.last_stop
        elapsed = stop - start
        need(gap >= 0 and elapsed > 0, "overlapping lease or nonpositive charge")
        if gap > self.q // 4 or elapsed > 5 * self.q // 4:
            self.misses.append(
                f"lease {len(self.gaps)+1}: gap {gap}, elapsed {elapsed}"
            )
        self.service[owner] += elapsed
        self.vr[owner] += elapsed * (2 - owner)
        self.fix_floor()
        self.last_stop = stop
        self.gaps.append(gap)

    def remove(self, owner):
        self.ready.remove(owner)
        self.fix_floor()

    def wake_b(self):
        need(self.ready == {0}, "wake membership")
        self.vr[1] = max(self.vr[1], self.floor)
        self.ready.add(1)
        self.fix_floor()

    def prefix(self, index):
        a, b = self.service
        h = 5 * self.q // 4
        diff = 2 * a - b
        need(self.ready == {0, 1}, "share ready set")
        if not -h <= diff <= 2 * h:
            self.misses.append(f"closure {index}: discrepancy {diff}")
        if index == 36:
            if a + b < 36 * self.q or abs(diff) * 36 * self.q > 2 * h * (a + b):
                self.misses.append("36-closure service/share tolerance")
        return {
            "closure": index,
            "service": list(self.service),
            "discrepancy": diff,
            "a_fraction": str(Fraction(a, a + b)),
        }


def root_info(ttbr):
    need(
        0 <= ttbr <= MASK64 and ttbr & ~(PA_MASK | (0xFFFF << 48)) == 0,
        "invalid ASID or translation table base",
    )
    return ttbr >> 48, ttbr & PA_MASK


def private_page(mem: Snapshot, ttbrs, va):
    info = [root_info(ttbr) for ttbr in ttbrs]
    need(
        info[0][0] != info[1][0] and info[0][1] != info[1][1],
        "ASID/address-space identity",
    )
    leaves = [mem.leaf(root, va) for _, root in info]
    need(
        all(
            desc is not None
            and desc & (1 << 10)
            and desc >> 6 & 3 == 1
            and desc >> 53 & 3 == 3
            and desc >> 11 & 1
            for desc in leaves
        ),
        "valid non-global user RW NX pages",
    )
    pages = [desc & PA_MASK for desc in leaves]
    need(pages[0] != pages[1], "aliased private user pages")
    return pages
