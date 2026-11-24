"""Exercise host observation with scripted reads and a synthetic clock."""

import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from pi4_serial_capture import EXPECTED, SECOND, observe


class CaptureTests(unittest.TestCase):
    def capture(self, events):
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        folder = Path(temporary.name)
        chunks = iter(events)
        elapsed = 0

        def read(size):
            nonlocal elapsed
            delay, data = next(chunks)
            elapsed += delay
            self.assertLessEqual(len(data), size)
            return data

        result = observe(folder, read, now=lambda: elapsed, sleep=lambda delay: None)
        data = (folder / "uart.bin").read_bytes()
        self.assertEqual(result["uart_sha256"], hashlib.sha256(data).hexdigest())
        timestamps = (folder / "reads.jsonl").read_text(encoding="utf-8").splitlines()
        return result, data, [json.loads(line) for line in timestamps]

    def test_deadline_and_quiet_boundaries(self):
        result, data, log = self.capture([(SECOND, EXPECTED[:4]),
                                          (29 * SECOND, EXPECTED[4:]), (SECOND, b"")])
        self.assertEqual(result["uart_verdict"], "matched")
        self.assertEqual(data, EXPECTED)
        self.assertEqual(log[1], dict(elapsed_ns=30 * SECOND, offset=4,
                                      length=len(EXPECTED) - 4))

    def test_extra_byte_preserved(self):
        result, data, _ = self.capture([(1, EXPECTED), (SECOND // 2, b"\x00")])
        self.assertEqual(result["uart_verdict"], "unexpected_bytes")
        self.assertEqual(data, EXPECTED + b"\x00")

    def test_late_complete_and_missing_input(self):
        for data in [EXPECTED, b""]:
            with self.subTest(data=data):
                result, _, _ = self.capture([(30 * SECOND + 1, data)])
                self.assertEqual(result["uart_verdict"], "timeout")


if __name__ == "__main__":
    unittest.main()
