"""Production decoder controls. These synthetic fixtures do not assess guest behavior."""
import struct
import tempfile
import unittest
from pathlib import Path

import sched_app_trace_decode as reader


REF = 1 << 32


def trace(records=(), *, mode="active", dk=0, da=0, flags=0):
    raw = bytearray(reader.EXTENT)
    struct.pack_into("<8s15Q", raw, 0, b"JNXTRC01", 1, 128, 64, 128,
                     4 if mode == "saturate" else 128, 0 if mode == "disabled" else 2,
                     10000, 1, 1, len(records), dk, da, flags, 0, 0)
    for i, record in enumerate(records):
        struct.pack_into("<8Q", raw, 128+64*i, i+1, *record, 0)
    return raw


# One row per modeled close/select, with no timer in this short join fixture.
RECORDS = [
    (3, 10, 0, REF, 0, 0),
    (1, 20, REF, REF, 0, 0),
    (3, 21, REF, REF+1, 0, 0),
    (2, 32, REF+1, REF, 30, 0),
    (3, 34, REF+1, REF, 0, 0),
    (5, 40, REF, REF, 77, 0),
]


def witness():
    header = [0]*40
    header[0:6] = [4, 100, 2300, 2300, 4, 30]
    header[8:10] = [34, 40]
    header[15:17] = [10, 6]
    header[28] = 77
    rows = []
    for kind, owner, stop, dest, start in [(0,4,0,0,10), (2,0,20,1,21),
                                            (3,1,30,0,34), (1,0,42,4,0)]:
        rows.extend([kind, int(kind == 3), owner, 0, 0, stop, 0, 0, 0, dest, start, 0, 0, 0, 0])
    return struct.pack("<1000Q", *(header + rows + [0]*(960-len(rows))))


class DecoderControls(unittest.TestCase):
    def test_join_and_absent_receipt(self):
        report = reader.parse_trace(trace(RECORDS), "active")
        reader.match_witness(witness(), report, "active")
        self.assertEqual(report["interval"]["S-Q"], 2)
        self.assertEqual(report["interval"]["E-S"], 6)
        for i in range(len(RECORDS)):
            with self.subTest(i=i), self.assertRaises(ValueError):
                partial = reader.parse_trace(trace(RECORDS[:i] + RECORDS[i+1:]), "active")
                reader.match_witness(witness(), partial, "active")

    def test_modes_and_overflow(self):
        disabled = reader.parse_trace(trace(mode="disabled"), "disabled")
        reader.match_witness(witness(), disabled, "disabled")
        with self.assertRaises(ValueError):
            reader.parse_trace(trace(RECORDS, mode="disabled"), "disabled")
        for flags, dk in [(0, 1), (0, reader.MAX), (1, reader.MAX)]:
            raw = trace(RECORDS[:4], mode="saturate", dk=dk, da=1, flags=flags)
            report = reader.parse_trace(raw, "saturate")
            if dk == 1:
                reader.match_witness(witness(), report, "saturate")
                self.assertEqual(report["coverage"], "incomplete-loss")
                self.assertNotIn("interval", report)
        with self.assertRaises(ValueError):
            reader.parse_trace(trace(RECORDS[:4], mode="saturate", dk=1, flags=1), "saturate")

    def test_malformed(self):
        for word, value in [(0, 0), (1, 2), (7, 0), (8, 2), (10, 129), (11, 1),
                            (13, 2), (15, 1), (16, 2), (17, 9), (20, REF*2),
                            (23, 1), (16+8*len(RECORDS), 1)]:
            raw = trace(RECORDS)
            struct.pack_into("<Q", raw, word*8, value)
            with self.subTest(word=word), self.assertRaises(ValueError):
                reader.parse_trace(raw, "active")
        with self.assertRaises(ValueError):
            reader.parse_trace(trace()[:-1], "active")

    def test_timer_pair_and_interrupted_publication(self):
        enter = (4, 50, REF, REF, 30, 48)
        leave = (6, 60, REF, REF, 30, 48)
        late = (5, 45, REF, REF, 77, 0)
        parsed = reader.parse_trace(trace([enter, leave, late]), "active")
        self.assertEqual(parsed["timer_services"][0]["rust_service_ticks"], 10)
        for events in ([enter], [leave], [enter, enter], [enter, leave, enter, leave],
                       [enter, (6, 60, REF+1, REF+1, 30, 48)]):
            with self.subTest(events=events), self.assertRaises(ValueError):
                reader.parse_trace(trace(events), "active")

    def test_integrity(self):
        with tempfile.TemporaryDirectory() as td:
            out = Path(td)
            elf = out / "kernel.elf"
            elf.write_bytes(b"first")
            reader.put(out / "manifest.json", {
                "schema": 1, "dictionary": reader.EVENTS[1:], "seal_errors": [],
                "artifacts": {"kernel.elf": reader.digest(elf)}})
            (out / "manifest.sha256").write_text("wrong")
            with self.assertRaisesRegex(ValueError, "manifest digest"):
                reader.decode(out)
            (out / "manifest.sha256").write_text(reader.digest(out/"manifest.json")+"  manifest.json")
            elf.write_bytes(b"other")
            with self.assertRaisesRegex(ValueError, "artifact digest: kernel.elf"):
                reader.decode(out)


if __name__ == "__main__":
    unittest.main()
