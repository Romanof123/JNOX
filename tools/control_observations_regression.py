"""Synthetic UART inputs for scorer integrity regressions (never guest evidence)."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from control_observations import Nonqualifying, negative_receiver

SCORER = Path(__file__).with_name("control_observations.py")
HEADER = b"CONTROL RAW v1\n32 1 0 3327779\n19 2 0 0\n"
SHORT = b"1 100 400000 25\n2 90 0 100\n3 1100 0 1\n21 1120 1 1119\n10 1121 1 1100\n18 1122 0 1\n"
END = b"CONTROL END\n"


class CaptureIntegrity(unittest.TestCase):
    def invoke(self, data):
        with tempfile.TemporaryDirectory() as directory:
            uart = Path(directory) / "cold.uart"
            uart.write_bytes(data)
            command = [sys.executable, str(SCORER), "loss", str(uart)]
            run = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(run.returncode, 0)
        self.assertEqual(run.stderr, "")
        result = json.loads(run.stdout)
        self.assertEqual(result["verdict"], "nonqualifying")
        self.assertTrue(result["failures"])
        self.assertNotIn("fault_budgets", result)
        self.assertNotIn("recurring_windows", result)
        return result

    def test_short_authenticated_terminal(self):
        result = self.invoke(HEADER + SHORT + END)
        self.assertEqual(result["policy"], "RM")
        self.assertEqual(
            result["observed"]["requested_and_actual_irq_phases"], [[0, 1]]
        )

    def test_timing_fault_rejects_subsequent_issued_permit(self):
        for cause in (1, 3):
            rows = [
                [11, 10, 1, 1],
                [12, 11, 1, 10],
                [11, 20, 2, 1],
                [12, 21, 2, 20],
                [14, 22, 0, 1],
                [15, 30, cause, 12],
                [13, 30, cause, 20],
                [11, 30, 3, 1],
                [24, 31, 3, 0],
                [14, 32, 1, 0],
                [18, 40, 0, 1],
            ]
            with self.subTest(cause=cause):
                self.assertEqual(
                    negative_receiver(rows, cause)["last_accepted_issue_at_latch"], 20
                )
                for pos, row in (
                    (8, [12, 31, 3, 30]),
                    (8, [24, 31, 9, 0]),
                    (9, [14, 32, 0, 1]),
                    (10, [18, 40, 1, 1]),
                ):
                    mutant = rows.copy()
                    mutant[pos] = row
                    with self.assertRaises(Nonqualifying):
                        negative_receiver(mutant, cause)

    def test_framing_and_payload(self):
        cases = (
            (HEADER + SHORT, "missing CONTROL END"),
            (HEADER + SHORT + END + END, "after terminal"),
            (HEADER + b"21 bad 1 2\n" + END, "malformed"),
            (HEADER + b"21 90 1 2\n21 90 1 2\n" + END, "duplicate"),
            (HEADER + b"21 90 1 2\n21 80 2 3\n" + END, "nonmonotonic"),
            (HEADER.replace(b"3327779", b"3327778") + SHORT + END, "embedded"),
        )
        for data, reason in cases:
            with self.subTest(reason=reason):
                self.assertIn(reason, self.invoke(data)["failures"][0])


if __name__ == "__main__":
    unittest.main()
