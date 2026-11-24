"""Synthetic controls for publication provenance. No guest-time evidence."""
from pathlib import Path
import unittest
from reservation_handoff import decode


def capture(word=3, flags=7):
    epoch = 3125100
    na = (1 << 64) - 1
    rows = [
        [1, 101, na, 2, 62500000, 3125000, 1, 1],
        [24, 102, na, 2, 99, epoch, 0, 0],
        [2, 103, na, 2, epoch, 10, 0, 0],
        [24, 90000 + epoch, 0, 0, 4, flags, word, 0 if flags & 4 else (1 << 64) - 1],
        [25, epoch + 1, 0, 2, 30, 5, epoch, epoch],
        [37, epoch + 50, 0, 2, epoch + 1, epoch + 50, epoch, 30],
        [26, epoch + 100, 0, 2, 4, 0, epoch, 0],
        [27, epoch + 84000, 0, 0, 3, 1, 1, 0],
    ]
    for id in range(2):
        rows.extend(
            [
                [31, epoch + 90000, 0, id, 1, 2, id, 2],
                [32, epoch + 90000, 0, id, 4, 3 + id, 1, 1 - id],
                [34, epoch + 90000, 0, id, 10 + id, 20 + id, 0x345, 30 + id],
                [35, epoch + 90000, 0, id, 0, 0, 30 + id, 0],
            ]
        )
    rows.append([33, epoch + 90000, 0, 2, 128, 128, 264, 184])
    rows.append([12, 104, na, 2, 10, 20, 0, 0])
    rows.append([13, epoch + 90001, 0, 2, 2, 5, 84000, epoch])
    return (
        "RSRV RAW 3\n"
        + "\n".join(" ".join(map(str, row)) for row in rows)
        + "\nRSRV END\n"
    ).encode()


class CompactReceipt(unittest.TestCase):
    def test_retained_cutoff_readback_before_selection_failure(self):
        raw = (
            Path(__file__).with_name("fixtures")
            / "reservation-compact-aligned-saturated-uart.txt"
        )
        result = decode(raw.read_bytes())
        self.assertEqual(result["stages"], 15)
        self.assertEqual(result["result"][:2], [2, 5])
        self.assertEqual(result["boot_anchor"][2], (1 << 64) - 1)
        self.assertEqual(result["actual_worker_return"], [])

    def test_published_without_dispatch(self):
        result = decode(capture())
        self.assertEqual(result["ledger_provenance"], "committed descriptor")
        self.assertEqual(result["actual_worker_return"], [])

    def test_mismatched_bank_missing_stage_and_truncation(self):
        for bad in (
            capture(0),
            capture(flags=3),
            capture().replace(b"27 3209100", b"26 3209100"),
            capture().replace(b"RSRV END", b""),
        ):
            with self.assertRaises(ValueError):
                decode(bad)

    def test_tag24_roles(self):
        raw = capture().splitlines(keepends=True)
        a, b = [i for i, r in enumerate(raw) if r.startswith(b"24 ")]

        def slot(row, value):
            f = row.split()
            f[2] = value
            return b" ".join(f) + b"\n"

        for rows in (
            raw[:a] + raw[a + 1 :],
            raw[:b] + raw[b + 1 :],
            raw[:b] + [raw[a]] + raw[b:],
            raw[:b] + [raw[b]] + raw[b:],
            raw[:a] + [raw[b]] + raw[a + 1 : b] + [raw[a]] + raw[b + 1 :],
            raw[:a] + [slot(raw[a], b"1")] + raw[a + 1 :],
            raw[:b] + [slot(raw[b], b"1")] + raw[b + 1 :],
        ):
            with self.assertRaises(ValueError):
                decode(b"".join(rows))
