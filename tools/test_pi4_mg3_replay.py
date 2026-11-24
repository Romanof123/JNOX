"""Offline controls for the two MG3 witness kinds, using private fixture copies."""
from copy import deepcopy
import unittest

from pi4_mg3_replay import witnesses


NAMES = ("M", "P0", "P1_OPEN", "P1_REPLACEMENT")
SYMBOLS = {name: {"address": 0x90000 + 64 * i}
           for i, name in enumerate(["PI4_MIG_CPU0", "PI4_MIG_CPU1"] +
                                    ["PI4_MIG_TLS_" + name for name in NAMES])}


def fixture(case):
    rows = {0: [], 1: []}
    workers = [(0, 0, 0, 0, 16, 152), (1, 0, 2, 2, 16, 152),
               (0, 1, 1, 1, 16, 152), (0 if case else 1, 1, 0, 0, 32, 20016),
               (1, 1, 2, 3, 32, 424)]
    for lease, (cpu, epoch, task, activation, index, h) in enumerate(workers, 1):
        token = 0x10601 + activation
        ptr = SYMBOLS["PI4_MIG_TLS_" + NAMES[activation]]["address"]
        base = SYMBOLS[f"PI4_MIG_CPU{cpu}"]["address"]
        irq = (epoch, task) == (0, 0)
        rows[cpu].extend([(11, (epoch, task, lease, 0, 0, 0, 0, 0)),
                          (5, (task, lease, lease, index, 0, 0, 0, 0)),
                          (15, (task, token, lease, base, ptr, token, index, h)),
                          (4, (epoch, task, 1232, 0, 0, 0, 1 if irq else 65535, 0)),
                          (14, (task, token, lease, 1232, ptr, token, base, lease))])
        if not irq:
            rows[cpu].extend([(13, (task, epoch, lease, 0, 0, 0, 0, 0)),
                              (9, (task, 0, lease, epoch, 0, 0, 0, 0))])
        rows[cpu].append((3, (epoch, task, lease, 0, 0, 0, 0, int(irq))))
    return rows


class LocalWitnesses(unittest.TestCase):
    def test_registered_routes(self):
        for case in range(3):
            self.assertEqual(len(witnesses(fixture(case), SYMBOLS, case, False)), 5)

    def test_modified_and_missing_rows(self):
        original = fixture(0)
        for lease, kind, field, replacement in (
                (4, 15, 3, SYMBOLS["PI4_MIG_CPU0"]["address"]),
                (4, 15, 4, 0), (4, 15, 4, SYMBOLS["PI4_MIG_TLS_P0"]["address"]),
                (5, 15, 1, 0x10603), (5, 14, 4, SYMBOLS["PI4_MIG_TLS_P1_OPEN"]["address"]),
                (4, 15, 2, 1), (4, 14, 7, 1), (4, 15, 7, 20017),
                (4, 15, None, None), (4, 14, None, None)):
            rows = deepcopy(original)
            seq = next(i for i, (k, d) in enumerate(rows[1]) if k == kind and d[2] == lease)
            if field is None:
                del rows[1][seq]
            else:
                data = list(rows[1][seq][1])
                data[field] = replacement
                rows[1][seq] = (kind, tuple(data))
            with self.subTest(lease=lease, kind=kind, field=field), self.assertRaises(ValueError):
                witnesses(rows, SYMBOLS, 0, False)

    def test_idle_and_saved_only(self):
        for forged in (False, True):
            rows = fixture(0)
            if forged:
                rows[1] = [(k, d) for k, d in rows[1] if not (k in (5, 15) and d[2] == 4)]
            else:
                rows[1].extend([(4, (1, 3, 1232, 0, 0, 0, 1, 0)), rows[1][4]])
            with self.assertRaises(ValueError):
                witnesses(rows, SYMBOLS, 0, False)
