"""Synthetic ELF boundary cases for the scheduling release footprint report."""

import contextlib
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest

import scheduling_footprint as footprint


def elf(machine=183, stack_size=8):
    data = bytearray(600)
    names = b"\0.bss\0.stack\0.shstrtab\0"
    header = (b"\x7fELF\x02\x01\x01", 2, machine, 1, 0, 64, 176, 0,
              64, 56, 2, 64, 4, 3)
    footprint.HEADER.pack_into(data, 0, *header)
    # The second file range is contained in the first, but maps to another VMA.
    footprint.PROGRAM.pack_into(data, 64, 1, 6, 512, 4096, 4096, 16, 32, 1)
    footprint.PROGRAM.pack_into(data, 120, 1, 6, 520, 4120, 4120, 8,
                                8 + stack_size, 1)
    sections = [
        (0, 0, 0, 0, 0, 0),
        (1, 8, 3, 4112, 99999, 8),
        (6, 8, 3, 4128, 99999, stack_size),
        (13, 3, 0, 0, 448, len(names)),
    ]
    for index, fields in enumerate(sections):
        footprint.SECTION.pack_into(data, 176 + index * 64, *fields, 0, 0, 1, 0)
    data[448:448 + len(names)] = names
    return data


class FootprintTests(unittest.TestCase):
    def test_load_overlap_backed_precedence_and_nobits(self):
        data = elf()
        report = footprint.measure(data)
        self.assertEqual(report["sha256"], hashlib.sha256(data).hexdigest())
        self.assertEqual(report["bytes"], {
            "file": 600, "load_file": 16, "load_memory": 40, "load_backed": 24,
            "zero_fill": 16, "alloc": 16, "padding": 24, "bss": 8, "linked_stack": 8,
        })

    def test_large_sparse_reservation(self):
        report = footprint.measure(elf(62, 1 << 40))
        self.assertEqual(report["machine"], "x86_64")
        self.assertEqual(report["bytes"]["linked_stack"], 1 << 40)

    def test_malformed_inputs(self):
        cases = [b"", elf()[:200], elf(99)]
        for offset, value in [(120 + 32, 100), (120 + 16, (1 << 64) - 1),
                              (240 + 16, 100), (368 + 24, 800)]:
            data = elf()
            data[offset:offset + 8] = value.to_bytes(8, "little")
            cases.append(data)
        for i, data in enumerate(cases):
            with self.subTest(i=i), self.assertRaises(ValueError):
                footprint.measure(data)

    def test_cli_deltas_and_failures(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder) / "minimal.elf"
            feature = Path(folder) / "extended.elf"
            base.write_bytes(elf(stack_size=16))
            feature.write_bytes(elf())
            args = ["--baseline", str(base), "--feature", f"extended={feature}"]
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                footprint.main(args)
            self.assertEqual(json.loads(output.getvalue())["features"]["extended"]
                             ["delta_bytes"]["linked_stack"], -8)
            for contents in [b"", elf(62)]:
                feature.write_bytes(contents)
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as err:
                    footprint.main(args)
                self.assertEqual(err.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
