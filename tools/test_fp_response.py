"""Host acceptance tests for fixed-priority response-time analysis."""

import itertools
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
from fp_response import analyze


def task(name, period, deadline, wcet, **extra):
    return dict(id=name, period=period, deadline=deadline, wcet=wcet, **extra)


def config(*tasks, policy="rm", **extra):
    return dict(policy=policy, tasks=list(tasks), irqs=[], **extra)


def simulate(tasks, policy):
    """Execute each integer tick, tracking completion minus release."""
    rank = "period" if policy == "rm" else "deadline"
    horizon = 2 * math.lcm(*(t["period"] for t in tasks))
    pending = []
    worst = {t["id"]: 0 for t in tasks}
    for tick in range(horizon):
        for t in tasks:
            if tick % t["period"] == 0:
                pending.append([t[rank], tick, t["id"], t["wcet"]])
        if pending:
            job = min(pending)
            job[3] -= 1
            if job[3] == 0:
                worst[job[2]] = max(worst[job[2]], tick + 1 - job[1])
                pending.remove(job)
    if pending:
        raise AssertionError("Expected feasible fixture to drain by hyperperiod")
    return worst


class ResponseTests(unittest.TestCase):
    def check_result(self, cfg, overall, expected):
        result = analyze(cfg)
        self.assertEqual(result["status"], overall)
        rows = result["tasks"]
        self.assertEqual(len(rows), len(expected))
        self.assertEqual({r["id"]: (r["status"], r["bound"]) for r in rows}, expected)

    def test_policy_changes_feasibility(self):
        tasks = [task("short", 4, 4, 2), task("urgent", 8, 2, 1)]
        ok = "within_bounds"
        miss = "deadline_exceeded"
        self.check_result(config(*tasks), miss, {"short": (ok, 2), "urgent": (miss, None)})
        self.check_result(config(*tasks, policy="dm"), ok, {"short": (ok, 3), "urgent": (ok, 1)})

    def test_inclusive_deadline_and_release_boundary(self):
        self.check_result(config(task("a", 4, 4, 1), task("b", 8, 4, 3)), "within_bounds",
                          {"a": ("within_bounds", 1), "b": ("within_bounds", 4)})

    def test_combined_interference(self):
        cfg = config(task("h", 6, 6, 1, dispatch=1, jitter=1),
                     task("l", 24, 16, 3, dispatch=1, blocking=1, jitter=2))
        cfg["irqs"] = [dict(id="timer", period=5, wcet=1, jitter=1)]
        # Low task busy window: 5 -> 9 -> 11 -> 12 -> 14 -> 14.
        self.check_result(cfg, "within_bounds",
                          {"h": ("within_bounds", 4), "l": ("within_bounds", 16)})

    def test_iteration_limit(self):
        cfg = config(task("a", 4, 4, 1), task("b", 8, 8, 2), iteration_limit=1)
        self.check_result(cfg, "inconclusive",
                          {"a": ("within_bounds", 1), "b": ("inconclusive", None)})

    def test_invalid(self):
        invalid = [
            config(task("a", 5, 3, 1), task("b", 5, 4, 1)),
            config(task("a", 5, 3, 1), task("b", 7, 3, 1), policy="dm"),
            config(task("a", 4, 4, 1), task("a", 6, 6, 1)),
            config(task("a", 4, 4, True)),
            config(task("a", 4.0, 4, 1)),
            config(task("a", 0, 1, 1)),
            config(task("a", 4, 0, 1)),
            config(task("a", 4, 4, 0)),
        ]
        for cfg in invalid:
            with self.subTest(cfg=cfg), self.assertRaises(ValueError):
                analyze(cfg)

    def test_tick_simulation(self):
        for policy, periods, costs in itertools.product(
                ("rm", "dm"), ((4, 7, 10), (5, 8, 12)), ((1, 1, 2), (1, 2, 1))):
            deadlines = (periods[0], periods[0] - 1, periods[2])
            tasks = [task(str(i), p, d, c) for i, (p, d, c) in
                     enumerate(zip(periods, deadlines, costs))]
            with self.subTest(policy=policy, periods=periods, costs=costs):
                observed = simulate(tasks, policy)
                result = analyze(config(*tasks, policy=policy))
                self.assertEqual(result["status"], "within_bounds")
                self.assertCountEqual((r["id"] for r in result["tasks"]), observed.keys())
                for row in result["tasks"]:
                    self.assertLessEqual(observed[row["id"]], row["bound"])

    def test_cli_exit_codes(self):
        fixtures = [
            (config(task("a", 4, 4, 1)), 0),
            (config(task("a", 4, 4, 2), task("b", 8, 2, 1)), 1),
            (config(task("a", 4, 4, 1), task("b", 8, 8, 2), iteration_limit=1), 2),
            (config(task("a", 0, 1, 1)), 2),
        ]
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "case.json"
            for cfg, expected in fixtures:
                with self.subTest(cfg=cfg):
                    path.write_text(json.dumps(cfg), encoding="utf-8")
                    run = subprocess.run([sys.executable, str(TOOLS / "fp_response.py"), str(path)],
                                         capture_output=True, text=True, timeout=10)
                    self.assertEqual(run.returncode, expected, run.stdout + run.stderr)

    def test_cli_rejects_duplicate_wcet(self):
        raw = ('{"policy":"rm","tasks":[{"id":"a","period":4,"deadline":4,'
               '"wcet":5,"wcet":1}],"irqs":[]}')
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "duplicate.json"
            path.write_text(raw, encoding="utf-8")
            run = subprocess.run([sys.executable, str(TOOLS / "fp_response.py"), str(path)],
                                 capture_output=True, text=True, timeout=10)
            self.assertEqual(run.returncode, 2, run.stdout + run.stderr)


if __name__ == "__main__":
    unittest.main()
