"""Synthetic controls for publication provenance. No guest-time evidence."""
import unittest
from reservation_handoff import decode


def capture(word=3, flags=7):
    epoch = 1000
    rows = [
        [1, 1, 0, 2, 62500000, 3125000, 1, 1],
        [24, 90000, 0, 0, 4, flags, word, 0 if flags & 4 else (1 << 64) - 1],
        [25, 1001, 0, 2, 30, 5, epoch, epoch],
        [37, 1050, 0, 2, 1001, 1050, epoch, 30],
        [26, 1100, 0, 2, 4, 0, epoch, 0],
        [27, 84000, 0, 0, 3, 1, 1, 0],
    ]
    for id in range(2):
        rows.extend(
            [
                [31, 90000, 0, id, 1, 2, id, 2],
                [32, 90000, 0, id, 4, 3 + id, 1, 1 - id],
                [34, 90000, 0, id, 10 + id, 20 + id, 0x345, 30 + id],
                [35, 90000, 0, id, 0, 0, 30 + id, 0],
            ]
        )
    rows.append([13, 90001, 0, 2, 2, 5, 84000, epoch])
    return (
        "RSRV RAW 3\n"
        + "\n".join(" ".join(map(str, row)) for row in rows)
        + "\nRSRV END\n"
    ).encode()


class CompactReceipt(unittest.TestCase):
    def test_published_without_dispatch(self):
        result = decode(capture())
        self.assertEqual(result["ledger_provenance"], "committed descriptor")
        self.assertEqual(result["actual_worker_return"], [])

    def test_mismatched_bank_missing_stage_and_truncation(self):
        for bad in (
            capture(0),
            capture(flags=3),
            capture().replace(b"27 84000", b"26 84000"),
            capture().replace(b"RSRV END", b""),
        ):
            with self.assertRaises(ValueError):
                decode(bad)
