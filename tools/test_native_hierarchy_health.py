"""Deterministic UART grammar, arm-cause and retained-boundary controls."""
import tempfile
import unittest
from pathlib import Path

from native_hierarchy_arm_replay import cause, DISTANCE
from read_hierarchy_health import read_uart


def fixture(drop=False):
    # One leaf has five ticks at every level, with a common 40-tick period.
    # Its first six-tick grant creates one tick of debt at each node.
    lines = ["NATIVE HIERARCHY CONFIG 1 100 62500000 40 1 1 1",
             "NATIVE HIERARCHY PARENT 0 5", "NATIVE HIERARCHY CHILD 0 0 5",
             "NATIVE HIERARCHY LEAF 0 0 5"]
    def emit(tag, data):
        lines.append("NATIVE HIERARCHY " + tag + " " + " ".join(map(str, data)))
    stamp = 148 if drop else 141
    prior = 0
    rows = [(100, 0, 3, 0, 1, 0, 4, 105, 5, 0, 0, 0, 5),
            (106, 1, 0, 3, 1, 0, 1, 140, 0, 6, 1, 1, 0),
            (stamp, 1, 3, 0, 1, 0, 4, stamp+4, 4, 0, 0, 0, 4),
            (stamp+1, 2, 0, 4, 0, 1, 6, 0, 0, 1, 0, 0, 3)]
    for i, (now, reason, src, dst, ready, exited, kind, due, reserved, elapsed, over, debt, left) in enumerate(rows):
        event = [0]*47
        event[:5] = [i, reason, 30 if reason == 1 else 0, now, now]
        event[5:8] = [now]*3
        if dst != 4:
            event[45] = now
        if dst == 3:
            event[46] = now
        if reason == 2:
            event[2] = 1
        event[8:15] = [src, dst, ready, 0, exited, prior, due]
        if src == 0:
            event[15:17] = [rows[i-1][0], rows[i-1][7]]
        event[17:20] = [elapsed, over, 2048 if reason == 1 else 0]
        event[28:32] = [reserved, debt, debt, debt]
        event[35:38] = [left]*3
        arm = [i+1, kind, due, now, prior, int(i > 0), now, due or prior, int(kind != 6)]
        entry = ([now, prior, 1, now, 5 if src == 3 else 0, 2048, 30, 0, 0, 0]
                 if reason == 1 else [0]*10)
        if reason == 1 and src == 3:
            entry[7:] = [rows[i-1][0], now, 128]
        outcome = (2 if drop else 1) if i == 2 else 0
        if not drop or i < 2:
            emit("EVENT", event)
            emit("TIMER", [i, *arm, i, *entry, outcome])
        if i == 1:
            refill = arm
        if i == 2:
            first = [1, 2, 2, 2, *refill, *entry, 1, 8]
        prior = due
    emit("DROPPED", [2 if drop else 0])
    emit("HEALTH", [1,40,5,4,2,1,0,0,int(drop),8 if drop else 1,0,4,2 if drop else 4,2 if drop else 0])
    emit("FIRST", first if drop else [0])
    lines.extend(["NATIVE EL0 EXIT 17", "NATIVE HIERARCHY HEALTH END 1"])
    return lines


def read(lines):
    with tempfile.TemporaryDirectory() as root:
        p = Path(root) / "uart"
        p.write_text("\n".join(lines) + "\n")
        return read_uart(p, [17])


class ArmHealthTest(unittest.TestCase):
    def test_complete_and_mutated_startup(self):
        lines = fixture()
        self.assertEqual(read(lines)["timing"], "within_limit")
        for idx, field, replacement in [(0, 2, 6), (1, 2, 3), (1, 2, 2), (2, 2, 5), (3, 2, 4)]:
            altered = lines.copy()
            line = [j for j, v in enumerate(lines) if v.startswith("NATIVE HIERARCHY TIMER ")][idx]
            cols = altered[line].split()
            cols[3+field] = str(replacement)
            altered[line] = " ".join(cols)
            with self.assertRaises(ValueError):
                read(altered)

    def test_first_dropped_after_retained_refill(self):
        lines = fixture(True)
        self.assertEqual(read(lines)["timing"], "late")
        pos = next(i for i, v in enumerate(lines) if v.startswith("NATIVE HIERARCHY FIRST "))
        for field in (2, 4, 8):
            altered = lines.copy()
            cols = altered[pos].split()
            cols[3+field] = str(int(cols[3+field])+1)
            altered[pos] = " ".join(cols)
            with self.assertRaises(ValueError):
                read(altered)

    def test_decision_and_idle_boundaries(self):
        for dropped, tag, index, field, value in [
            (False, "TIMER", 2, 19, 1000000141),
            (False, "TIMER", 2, 15, 133),
            (False, "TIMER", 2, 20, 0),
            (False, "TIMER", 2, 18, 107),
            (False, "TIMER", 2, 5, 139),
            (False, "EVENT", 0, 11, 1),
            (False, "EVENT", 2, 5, 0),
            (False, "EVENT", 0, 45, 107),
            (True, "FIRST", 0, 20, 107),
            (True, "FIRST", 0, 17, 133),
        ]:
            with self.subTest(tag=tag, field=field, dropped=dropped):
                lines = fixture(dropped)
                pos = [j for j, line in enumerate(lines)
                       if line.startswith(f"NATIVE HIERARCHY {tag} ")][index]
                cols = lines[pos].split()
                cols[3+field] = str(value)
                lines[pos] = " ".join(cols)
                with self.assertRaises(ValueError):
                    read(lines)

    def test_wake_tie_checkpoint(self):
        self.assertEqual(cause(0, 10, 50, 50), (3, 50))
        self.assertEqual(cause(3, 10, 50, 50), (2, 50))
        self.assertEqual(cause(3, 10, 50, 40), (3, 40))
        self.assertEqual(cause(3, 10, 40, 50), (1, 40))
        self.assertEqual(cause(3, 10, None, DISTANCE+11), (5, DISTANCE+10))
        self.assertEqual(cause(3, 10, None, DISTANCE+10), (3, DISTANCE+10))


if __name__ == "__main__":
    unittest.main()
