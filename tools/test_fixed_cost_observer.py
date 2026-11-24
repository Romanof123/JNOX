"""Synthetic decoder controls; no guest is started."""
import struct
import unittest

from fixed_cost_observer import FRAME, STACK, decode


class DecoderControls(unittest.TestCase):
    def setUp(self):
        self.base = 0x41000000
        block = b"\xa5" * (STACK - FRAME) + bytes(FRAME)
        self.stacks = block * 3 + bytes(STACK) + block
        self.records = bytearray((5 + 128 * 7) * 8)
        rows = [(3, 0, 1), (0, 1, 2), (2, 2, 0), (2, 0, 0), (1, 0, 4)]
        struct.pack_into("<5Q", self.records, 0, 62500000, 1, 2, len(rows), 0)
        for i, (op, a, b) in enumerate(rows):
            fp = self.base + STACK - FRAME
            struct.pack_into("<7Q", self.records, 40+i*56, op, fp+a*STACK, fp+b*STACK,
                             *range(4+i*4, 8+i*4))

    def decode(self):
        return decode("fixed-fifo", self.records, self.stacks, self.base)

    def test_complete(self):
        self.assertEqual(self.decode()["groups"]["sleep_return"]["event_max_ticks"], 1)

    def test_empty(self):
        struct.pack_into("<Q", self.records, 24, 0)
        with self.assertRaises(ValueError):
            self.decode()

    def test_overflow(self):
        struct.pack_into("<Q", self.records, 32, 1)
        with self.assertRaises(ValueError):
            self.decode()

    def test_inverted_time(self):
        struct.pack_into("<Q", self.records, 40+5*8, 0)
        with self.assertRaises(ValueError):
            self.decode()


if __name__ == "__main__":
    unittest.main()
