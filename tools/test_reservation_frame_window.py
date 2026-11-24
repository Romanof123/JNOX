"""Attribution controls for a synthetic frame pair in a retained RAW3 opening."""
from dataclasses import replace
import hashlib
from pathlib import Path
import struct
import unittest

from reservation_frame_window import Launch, replay


def digest(b):
    return hashlib.sha256(b).hexdigest()


class FrameWindow(unittest.TestCase):
    def setUp(self):
        self.baseline = (Path(__file__).parent / "fixtures/reservation-compact-aligned-saturated-uart.txt").read_bytes()
        # Two distinct MRS instructions inside a synthetic executable PT_LOAD.
        self.elf = bytearray(128)
        self.elf[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", self.elf, 18, 183)
        struct.pack_into("<Q", self.elf, 32, 64)
        struct.pack_into("<HH", self.elf, 54, 56, 1)
        struct.pack_into("<IIQQQQ", self.elf, 64, 1, 5, 120, 4096, 0, 8)
        struct.pack_into("<II", self.elf, 120, 0xd53be020, 0xd53be021)
        self.first = b"39 3210000 0 2 2 1 4096 3179251\n"
        self.last = b"40 3220000 0 2 2 1 4100 3179251\n"
        self.raw = self.baseline.replace(b"RSRV RAW 3", b"RSRV RAW 5").replace(
            b"36 153632", self.first + self.last + b"36 153632")
        self.launch = Launch(2, "aligned-saturated", "P", True, digest(self.elf),
                             digest(self.raw), 4096, 4100)

    def test_one_same_boot_interval_and_original_stop(self):
        result, base = replay(self.raw, self.elf, self.launch)
        self.assertEqual(result["pieces_ticks"], [159, 10000, 10098])
        self.assertEqual(result["whole_ticks"], 20257)
        self.assertEqual(base.splitlines(), self.baseline.splitlines())
        baseline_launch = replace(self.launch, ordinal=1, arm="B", before_pc=None, after_pc=None,
                                  uart_sha256=digest(self.baseline))
        untouched, _ = replay(self.baseline, self.elf, baseline_launch)
        self.assertEqual(result["terminal"], untouched["terminal"])
        self.assertIsNone(untouched["frame_interval_ticks"])

    def test_diagnostically_invalid_even_under_new_uart_digest(self):
        pair = self.first + self.last
        for bad in (self.last + self.first, self.first + self.first, self.first, b"",
                    pair.replace(b" 4096 ", b" 4100 "), pair.replace(b" 3179251", b" 3179252"),
                    pair.replace(b"39 3210000", b"39 3190000"),
                    pair.replace(b"40 3220000", b"40 3210000").replace(b"39 3210000", b"39 3210001"),
                    pair.replace(b" 0 2 2 1 ", b" 0 2 4 1 "),
                    b"39 0 0 2 2 0 0 0\n40 0 0 2 2 0 0 0\n"):
            raw = self.raw.replace(pair, bad)
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                replay(raw, self.elf, replace(self.launch, uart_sha256=digest(raw)))

    def test_pinned_artifact_and_position(self):
        for bad in (replace(self.launch, elf_sha256="0" * 64), replace(self.launch, uart_sha256="0" * 64),
                    replace(self.launch, ordinal=3), replace(self.launch, after_pc=4104),
                    replace(self.launch, before_pc=4100, after_pc=4096)):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                replay(self.raw, self.elf, bad)
