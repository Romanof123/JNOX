import struct
import unittest

from pi4_mg4_terminal import JOURNAL_BYTES, TERMINAL, ram_journals


class DiagnosticTests(unittest.TestCase):
    def test_terminal_is_a_complete_line(self):
        self.assertIsNotNone(TERMINAL.fullmatch(b"PI4 MIG FAIL 000000000000001d"))
        self.assertIsNotNone(TERMINAL.fullmatch(b"MG END 1 32 123 4 5"))
        self.assertIsNone(TERMINAL.fullmatch(b"MG END "))
        self.assertIsNone(TERMINAL.fullmatch(b"PI4 MIG FAIL 000000000000001d extra"))

    def test_ram_journal_reads_only_committed_rows(self):
        data = bytearray(2 * JOURNAL_BYTES)
        struct.pack_into("<9Q", data, JOURNAL_BYTES, *range(8), 17)
        struct.pack_into("<Q", data, JOURNAL_BYTES + 96 * 72, 1)
        self.assertEqual(ram_journals(data), {0: [], 1: [(17, tuple(range(8)))]})
        struct.pack_into("<Q", data, JOURNAL_BYTES + 96 * 72 + 8, 1)
        with self.assertRaises(ValueError):
            ram_journals(data)
        with self.assertRaises(ValueError):
            ram_journals(data[:-1])


if __name__ == "__main__":
    unittest.main()
