"""Independent arithmetic and parser regression cases for the segmented fixture."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
from fs_m3_admission import analyze


class AdmissionCases(unittest.TestCase):
    def setUp(self):
        self.data = json.loads((TOOLS / "fs_m3_example.json").read_text())

    def test_independent_small_window(self):
        # Exactly one intersecting timer service and one of each control segment.
        for policy in ("rm", "dm"):
            out = analyze(self.data, policy)
            self.assertEqual(out["status"], "conditional")
            self.assertEqual(out["a"]["upper_with_uncertainty"], 40+100+10+10+20+5)
            self.assertEqual(out["b"]["upper_with_uncertainty"], 40+200+10+10+20+5)
            self.assertEqual(out["sender"]["upper_with_uncertainty"], 40+310+20+110+210+5)
            self.assertEqual((out["job_finish"], out["check_gap"]), (2785, 2785))
            self.assertEqual(out["worker_service_plus_dispatch"]["per_nominal_10ms"], 950)

    def test_gap_across_job_boundary(self):
        self.data["a"]["service"] = 915
        out = analyze(self.data, "dm")
        self.assertEqual((out["check_gap"], out["job_finish"]), (3500, 2785))
        self.assertEqual(out["status"], "conditional")
        self.data["a"]["service"] += 1
        out = analyze(self.data, "dm")
        self.assertEqual(out["unproved_limits"], ["a_decision_1000us"])
        self.assertEqual(out["status"], "refused")

    def test_carry_in_and_repeated_interference(self):
        self.data["sender"]["service"] = 4490
        # First iterate 4500. At 4860, each phase's finish window crosses
        # 5000. At 5180, three timer services intersect. Final window 5200.
        out = analyze(self.data, "rm")
        self.assertEqual(out["sender"]["bound"], 40+4500+3*20+2*(110+210))

    def test_invalid_or_missing(self):
        for key, value in (("blocking", False), ("service", 0), ("extra", 1)):
            with self.subTest(key=key):
                data = dict(self.data, b=dict(self.data["b"], **{key: value}))
                with self.assertRaises(ValueError):
                    analyze(data, "rm")
        with self.assertRaises(ValueError):
            analyze(self.data, "edf")
        self.data["a"]["service"] = 99999
        del self.data["sender"]["jitter"]
        with self.assertRaises(ValueError):
            analyze(self.data, "dm")

    def test_lateness_limit(self):
        self.data["irq_entry_lateness"] = 495
        self.assertEqual(analyze(self.data, "dm")["status"], "conditional")
        self.data["irq_entry_lateness"] += 1
        self.assertEqual(analyze(self.data, "rm")["unproved_limits"], ["irq_entry_500us"])

    def test_duplicate_json_key(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.json"
            path.write_text('{"a": 1, "a": 2}')
            run = subprocess.run([sys.executable, str(TOOLS / "fs_m3_admission.py"),
                                  str(path), "rm"], capture_output=True, text=True)
            self.assertEqual(run.returncode, 2)
            out = json.loads(run.stdout)
            self.assertEqual(out["status"], "unknown")
            self.assertIn("duplicate", out["reason"])


if __name__ == "__main__":
    unittest.main()
