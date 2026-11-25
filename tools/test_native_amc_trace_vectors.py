"""Focused offline controls for the AMC QEMU exception/vector join."""
import unittest

from native_amc_report import trace_vectors


class VectorControls(unittest.TestCase):
    def test_linked_destinations(self):
        base = 0x40400000
        for cause, src, slot in ((1, 0, 9), (1, 2, 5), (2, 0, 8)):
            with self.subTest(cause=cause, src=src):
                row = [0] * 23
                row[0], row[4], row[10] = cause, src, 0x400080
                head = "5 [IRQ]" if cause == 1 else "2 [SVC]"
                origin = int(src == 2)
                prefix = (f"Taking exception {head}\n...from EL{origin} to EL1\n"
                          "...with ESR 0x15/0x56004a01\n...with ELR 0x400080\n")
                def destination(index):
                    return f"...to EL1 PC {base + 128 * index:#x} PSTATE 0x3c5\n"
                self.assertEqual(trace_vectors((prefix + destination(slot)).encode(), [row], 2, base, 0),
                                 int(src != 2))
                for replacement in ("", destination(8 if slot != 8 else 9)):
                    with self.assertRaises(ValueError):
                        trace_vectors((prefix + replacement).encode(), [row], 2, base, 0)
