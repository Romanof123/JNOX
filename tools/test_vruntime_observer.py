"""Standalone positive and adversarial transcripts for the weighted guest."""

from fractions import Fraction
import unittest

from vruntime_observer import MAX_BYTES, validate


def witness(freq=5000, origin=1000, gap=7, extra=None, code=0x40201400, sp=0x40403FF0):
    """Generate UART from an independent Fraction model of v += elapsed/weight."""
    weights = (1, 2)
    v = [Fraction(0), Fraction(0)]
    totals = [0, 0]
    progress = [2, 7]
    q = freq // 50
    start = origin
    extra = extra or {}
    lines = ["JNOX M1 BOOT", "JNOX M1 READY", "VR BEGIN", f"VR CLOCK freq={freq} quantum={q}"]
    for t in range(21):
        if t == 12:
            a, b = totals
            share = f"VR SHARE slots=12 a={a} b={b} error={abs(2*a-b)} tolerance={4*q}"
            sleep = f"VR SLEEP t=12 id=1 before2={2*v[1]} floor2={2*v[0]}"
        if t == 15:
            before, floor = v[1], v[0]
            v[1] = max(v[1], floor)
            wake = (f"VR WAKE t=15 id=1 before2={2*before} floor2={2*floor} "
                    f"after2={2*v[1]} lag_before={weights[1]*(floor-before)} lag_after=0")
        ready = (0,) if 12 <= t < 15 else (0, 1)
        task = min(ready, key=lambda i: (v[i], i))
        cval = start + q
        irq = cval + extra.get(t, 0)
        elapsed = irq - start
        totals[task] += elapsed
        v[task] += Fraction(elapsed, weights[task])
        floor = min(v[i] for i in ready)
        progress[task] += t + 1
        lines.append(f"VR SLOT t={t} id={task} ready={sum(1 << i for i in ready)} "
                     f"start={start} cval={cval} irq={irq} charge={elapsed} "
                     f"a2={2*v[0]} b2={2*v[1]} floor2={2*floor} "
                     f"elr={code+32*task+4*(t%2)} sp={sp+16384*task} progress={progress[task]}")
        start = irq + gap
    end = (f"VR END slots=21 a={totals[0]} b={totals[1]} floor2={2*floor} "
           f"lag_b={weights[1]*(floor-v[1])}")
    return "\n".join(lines + [share, sleep, wake, end, "VR STOP", "VR PASS", ""]).encode("ascii")


def change(data, row, key, value):
    lines = data.split(b"\n")
    parts = lines[row].split(b" ")
    prefix = key.encode() + b"="
    index, = [i for i, part in enumerate(parts) if part.startswith(prefix)]
    parts[index] = prefix + str(value).encode()
    lines[row] = b" ".join(parts)
    return b"\n".join(lines)


class VruntimeObserverTests(unittest.TestCase):
    def test_accepts_varied_winners_fractional_service_and_signed_recovery_lag(self):
        self.assertTrue(validate(witness()))
        data = witness(extra={20: 17})
        self.assertIn(b"lag_b=-17\n", data)
        self.assertTrue(validate(data))
        data = witness(freq=5037, origin=9999, code=0x40300000, sp=0x40600000,
                       extra={0: 50, 8: 11, 15: 7, 20: 33})
        self.assertIn(b"VR SLOT t=3 id=1 ", data)
        self.assertTrue(validate(data))
        # Clock can start high and handler gap can be zero; timer interval is positive.
        self.assertTrue(validate(witness(freq=50, origin=(1 << 64) - 22, gap=0)))

    def test_rejects_partial_duplicate_reordered_trailing_and_overlong(self):
        data = witness()
        lines = data.splitlines(keepends=True)
        for i in range(len(lines)):
            self.assertFalse(validate(b"".join(lines[:i] + lines[i + 1:])))
            self.assertFalse(validate(b"".join(lines[:i] + [lines[i]] + lines[i:])))
        for i in range(len(lines) - 1):
            swapped = list(lines)
            swapped[i], swapped[i+1] = swapped[i+1], swapped[i]
            self.assertFalse(validate(b"".join(swapped)))
        for invalid in (b"", data[:-1], data + b"\n", data + b"VR FAIL\n",
                        data.replace(b"\n", b"\r\n"), b"x" * (MAX_BYTES + 1)):
            self.assertFalse(validate(invalid))

    def test_rejects_numeric_and_field_syntax_outside_the_guest_format(self):
        data = witness()
        for invalid in ("-1", "-0", "+1", "05000", "", "18446744073709551616",
                        "1_0", "1e2", "１２", "50  quantum=100", " 5000", "9" * 500):
            self.assertFalse(validate(change(data, 3, "freq", invalid)))
        for invalid in ("-0", "+0", "00", "-18446744073709551616"):
            self.assertFalse(validate(change(data, 28, "lag_b", invalid)))
        for invalid in (b"cval=1100 extra=0", b"cv=1100", b"cval=1100\t"):
            self.assertFalse(validate(data.replace(b"cval=1100", invalid, 1)))

    def test_rejects_inconsistent_timer_charge_priority_floor_and_context(self):
        data = witness()
        for row, key, value in [
            (3, "freq", 1 << 32), (3, "quantum", 99), (4, "cval", 1101),
            (4, "irq", 1099), (4, "irq", 1200), (4, "charge", 0), (5, "start", 1099),
            (4, "id", 1), (5, "a2", 0), (5, "b2", 50), (5, "floor2", 0),
            (16, "ready", 3), (16, "id", 1), (19, "ready", 1), (19, "id", 1),
            (4, "progress", 1), (6, "progress", 9), (5, "sp", 0x40403ff0),
            (4, "elr", 0x40201402), (5, "elr", 0x40201400),
        ]:
            with self.subTest(row=row, key=key):
                self.assertFalse(validate(change(data, row, key, value)))
        self.assertFalse(validate(witness(freq=49)))
        self.assertFalse(validate(witness(origin=(1 << 64) - 2)))
        self.assertFalse(validate(witness(code=0x30000000)))
        self.assertFalse(validate(witness(sp=0x47fffff0)))

    def test_replays_each_reported_share_placement_and_signed_lag(self):
        data = witness(extra={20: 17})
        for row, key, value in [
            (25, "a", 401), (25, "b", 799), (25, "error", 1), (25, "tolerance", 401),
            (26, "id", 0), (26, "before2", 0), (26, "floor2", 0),
            (27, "before2", 0), (27, "floor2", 0), (27, "after2", 0),
            (27, "lag_before", -600), (27, "lag_after", 1),
            (28, "a", 0), (28, "b", 0), (28, "floor2", 0), (28, "lag_b", 17),
        ]:
            with self.subTest(row=row, key=key):
                self.assertFalse(validate(change(data, row, key, value)))


if __name__ == "__main__":
    unittest.main()
