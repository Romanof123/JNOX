"""Host-only controls for CHG-071 diagnostic receipt attribution."""
from dataclasses import replace
import hashlib
import struct
import unittest

from reservation_stage import Launch, replay
from test_reservation_handoff import capture


def receipt():
    # Synthetic CBS-S post-publication guard stop, with a registered E-site PC.
    elf = bytearray(124)
    elf[:6] = b"\x7fELF\x02\x01"
    struct.pack_into("<H", elf, 18, 183)
    struct.pack_into("<Q", elf, 32, 64)
    struct.pack_into("<HH", elf, 54, 56, 1)
    struct.pack_into("<IIQQQQ", elf, 64, 1, 5, 120, 4096, 0, 4)
    struct.pack_into("<I", elf, 120, 0xD53BE020)
    raw = capture().replace(b"RSRV RAW 3", b"RSRV RAW 4")
    raw = raw.replace(b"24 3215100", b"38 3125175 0 2 1 4 1 4096\n24 3215100")
    launch = Launch(5, "cbs-saturated", "E", hashlib.sha256(elf).hexdigest(),
                    hashlib.sha256(raw).hexdigest(), 4096)
    return raw, elf, launch


class StageReader(unittest.TestCase):
    def test_guard_stop_preserves_operands(self):
        raw, elf, launch = receipt()
        out = replay(raw, elf, launch)
        self.assertEqual((out["prefix"], out["suffix"], out["whole"]), (25, 25, 50))
        self.assertEqual(out["terminal"][4:], [2, 5, 84000, 3125100])
        self.assertEqual(out["actual_return"], [])

    def test_missing_wrong_site_wrong_pc_and_wrong_order(self):
        raw, elf, launch = receipt()
        for replacement in (b"", b"38 3125175 0 2 2 4 1 4096\n",
                            b"38 3125175 0 2 1 4 1 4100\n",
                            b"38 3125999 0 2 1 4 1 4096\n",
                            b"38 0 0 2 1 4 0 0\n"):
            edited = raw.replace(b"38 3125175 0 2 1 4 1 4096\n", replacement)
            with self.subTest(replacement=replacement), self.assertRaises(ValueError):
                replay(edited, elf, replace(launch, uart_sha256=hashlib.sha256(edited).hexdigest()))
