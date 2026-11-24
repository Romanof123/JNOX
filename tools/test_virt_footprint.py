"""Synthetic two-segment controls for the CHG-090 engineering footprint contract."""
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

import scheduling_footprint as elf
import virt_footprint as checker


def fixture(code=29, ram=None):
    base = checker.BASE
    writable = (code + 4095) // 4096 * 4096
    ram = writable + 32 + checker.STACK_SIZE if ram is None else ram
    names = b"\0.text\0.rodata\0.data\0.bss\0.stack\0.shstrtab\0"
    data_offset = 4096 + writable
    data = bytearray(data_offset + 3)
    elf.HEADER.pack_into(data, 0, b"\x7fELF\x02\x01\x01", 2, 183, 1, base, 64, 256,
                         0, 64, 56, 2, 64, 7, 6)
    elf.PROGRAM.pack_into(data, 64, 1, 5, 4096, base, base, code, code, 4096)
    elf.PROGRAM.pack_into(data, 120, 1, 6, data_offset, base + writable, base + writable,
                          3, ram - writable, 4096)
    data[768:768 + len(names)] = names
    sections = [
        (".text", 1, 6, base, 4096, 16, 4),
        (".rodata", 1, 2, base + 24, 4096 + 24, code - 24, 1),
        (".data", 1, 3, base + writable, data_offset, 3, 1),
        (".bss", 8, 3, base + writable + 8, data_offset + 3, 8, 8),
        (".stack", 8, 3, base + ram - checker.STACK_SIZE, data_offset + 3, checker.STACK_SIZE, 16),
        (".shstrtab", 3, 0, 0, 768, len(names), 1),
    ]
    for i, (name, kind, flags, va, off, size, align) in enumerate(sections, 1):
        elf.SECTION.pack_into(data, 256 + i * 64, names.index(name.encode()), kind, flags,
                              va, off, size, 0, 0, align, 0)
    return data


class FootprintTests(unittest.TestCase):
    def test_padding_domains_and_separate_file_telemetry(self):
        actual = checker.profile(fixture())["bytes"]
        self.assertEqual(actual["immutable_span"], 29)
        self.assertEqual(actual["immutable_padding"], 8)
        self.assertEqual(actual["segment_gap"], 4096 - 29)
        self.assertEqual(actual["writable_padding"], 21)
        self.assertEqual(actual["load_file"], 32)
        self.assertEqual(actual["load_memory"], 29 + 32 + 16384)
        self.assertEqual(actual["ram_span"], 4096 + 32 + 16384)

    def test_cli_thresholds_and_comparator(self):
        with tempfile.TemporaryDirectory() as td:
            minimal, rr = (Path(td) / name for name in ("minimal", "rr"))
            rr.write_bytes(fixture(code=checker.CODE_LIMIT + 1, ram=checker.RAM_LIMIT + 16))
            for code, ram, rc in [(checker.CODE_LIMIT, checker.RAM_LIMIT, 0),
                                  (checker.CODE_LIMIT + 1, checker.RAM_LIMIT, 1),
                                  (checker.CODE_LIMIT, checker.RAM_LIMIT + 16, 1)]:
                with self.subTest(code=code, ram=ram):
                    minimal.write_bytes(fixture(code, ram))
                    output = io.StringIO()
                    with contextlib.redirect_stdout(output):
                        self.assertEqual(checker.main(["--minimal", minimal.as_posix(),
                                                       "--fixed-rr", rr.as_posix()]), rc)
                    report = json.loads(output.getvalue())
                    self.assertEqual(report["delta_bytes"]["immutable_span"], checker.CODE_LIMIT + 1 - code)
                    self.assertEqual(report["delta_bytes"]["ram_span"], checker.RAM_LIMIT + 16 - ram)
            rr.write_bytes(b"truncated")
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as invalid:
                checker.main(["--minimal", str(minimal), "--fixed-rr", str(rr)])
            self.assertEqual(invalid.exception.code, 2)

    def test_invalid_layouts(self):
        cases = {
            "wrong machine": (18, 2, 62),
            "entry outside text": (24, 8, checker.BASE + 4),
            "nonidentity physical address": (64 + 24, 8, checker.BASE + 4096),
            "RX zero fill": (64 + 40, 8, 30),
            "writable executable": (120 + 4, 4, 7),
            "file-backed overlap": (120 + 8, 8, 4096),
            "window exceeded": (120 + 40, 8, checker.LIMIT - checker.BASE),
            "section outside immutable span": (256 + 2 * 64 + 32, 8, 10),
            "incorrect section file offset": (256 + 2 * 64 + 24, 8, 4096 + 23),
            "overlapping sections": (256 + 64 + 32, 8, 25),
            "unrecognized allocated section": (256 + 3 * 64, 4, 33),
            "NOBITS in file data": (256 + 4 * 64 + 16, 8, checker.BASE + 4096),
            "invalid stack extent": (256 + 5 * 64 + 32, 8, 16),
        }
        for name, (off, size, value) in cases.items():
            with self.subTest(name=name), self.assertRaises(ValueError):
                data = fixture()
                data[off:off + size] = value.to_bytes(size, "little")
                checker.profile(data)


if __name__ == "__main__":
    unittest.main()
