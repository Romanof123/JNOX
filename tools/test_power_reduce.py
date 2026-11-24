"""Ordinary arithmetic examples; all values invented, with no physical acquisition."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from power_reduce import reduce_record


class PowerReduction(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.directory = Path(folder.name)
        self.manifest = {"kind": "fixture", "producer": "ordinary arithmetic example"}
        events = {key: "fixture" for key in (
            "board", "firmware", "workload", "cpu_opp", "thermal", "shared_load",
            "environment", "idle_state", "instrument", "trigger", "limits")}
        events.update(counter_hz=4, trigger_tick=40, trigger_ns=0, release_tick=49,
                      wake_tick=49, first_tick=50, complete_tick=51, deadline_tick=50,
                      idle_start_tick=41, idle_end_tick=43, sync_uncertainty_ns=0,
                      max_gap_ns=1000000000)
        self.source("events", events)
        self.samples = [[i * 1000000000, 1000000, amp * 1000000]
                        for i, amp in enumerate([4, 4, 0, 2])]

    def source(self, key, data):
        name = key + ".json"
        raw = json.dumps(data).encode("utf-8")
        (self.directory / name).write_bytes(raw)
        self.manifest[key] = dict(file=name, sha256=hashlib.sha256(raw).hexdigest())

    def report(self):
        path = self.directory / "record.json"
        path.write_text(json.dumps(self.manifest), encoding="utf-8")
        return reduce_record(path)

    def test_edge_clipped_ramp_and_miss(self):
        self.source("samples", self.samples)
        result = self.report()
        # Idle: 4 W for half a second. Run: 0.5 to 1.5 W for half a second.
        self.assertEqual([result[k] for k in ("idle_J", "run_J", "adjusted_J", "wake_s",
                                              "response_s", "deadline_margin_s")],
                         ["2", "1/2", "-3/2", "1/4", "1/2", "-1/4"])
        self.assertTrue(result["observed_miss"])
        self.assertEqual(result["coverage"]["run"]["covered_ns"], "500000000")

    def test_partial_run(self):
        self.samples[-1][0] = 2500000000
        self.source("samples", self.samples)
        result = self.report()
        self.assertEqual(result["status"], "incomplete sample coverage")
        self.assertEqual(result["coverage"]["run"]["covered_ns"], "250000000")
        self.assertNotIn("adjusted_J", result)

    def test_uncovered_gap(self):
        self.source("samples", self.samples[:1] + self.samples[2:])
        self.assertEqual(self.report()["coverage"]["idle"]["covered_ns"], "0")

    def test_changed_source(self):
        self.source("samples", self.samples)
        (self.directory / "samples.json").write_bytes(b"[]")
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            self.report()

    def test_duplicate_sample_time(self):
        self.source("samples", self.samples + self.samples[-1:])
        with self.assertRaisesRegex(ValueError, "strictly increase"):
            self.report()


if __name__ == "__main__":
    unittest.main()
