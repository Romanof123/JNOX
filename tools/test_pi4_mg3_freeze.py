"""Wire declaration controls for capture and replay (no guest launch)."""
import unittest

from pi4_mg3_symbols import declared_wire, mg3_symbols


class WireDeclarations(unittest.TestCase):
    def test_legacy(self):
        for wire in (1, 2):
            raw = f"MG {wire} BEGIN ".encode()
            self.assertEqual(declared_wire(raw), wire)
            self.assertIsNone(mg3_symbols(b"legacy", raw, "migration-global", "sum-65536"))

    def test_missing_unsupported_and_mixed(self):
        for raw in (b"", b"MG 4 BEGIN", b"MG 2 BEGIN MG 3 BEGIN"):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                declared_wire(raw)

    def test_legacy_downgrade(self):
        for elf, raw in ((b"PI4_MIG_TLS_M\0", b"MG 2 BEGIN"),
                         (b"legacy", b"MG 1 BEGIN MG3|migration-global|sum-65536")):
            with self.assertRaises(ValueError):
                mg3_symbols(elf, raw, "migration-global", "sum-65536")


if __name__ == "__main__":
    unittest.main()
