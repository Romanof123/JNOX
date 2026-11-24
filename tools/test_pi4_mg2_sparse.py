"""Offline controls for atomic causality and terminal RAM joins; no guest needed."""
import unittest

from pi4_mg2_sparse import causal_order, reconcile


class SparseEvidence(unittest.TestCase):
    def setUp(self):
        def row(seq, kind, *words):
            return dict(seq=seq, kind=kind, words=words + (0,) * (8 - len(words)))

        self.rows = [row(0, 11, 0, 0, 3), row(1, 5, 0, 3, 3, 65536, 2147516416, 0, 0x801f0)]
        p1 = [row(0, 3, 0, 2, 4), row(1, 7, 0, 0x18001)]
        self.snap = dict(flags={"INVALID": 0}, journals=[dict(rows=r, lost=0) for r in (self.rows, p1)])
        self.events = {0: (0, 2, 3), 1: (1, 0x18001, 0)}

    def test_both_directions_and_overlap(self):
        self.assertEqual(reconcile(self.events, self.snap), "final M commit before SGIR")
        self.events.update({0: (1, 2, 3), 1: (0, 0x18001, 0)})
        self.assertEqual(reconcile(self.events, self.snap), "SGIR before final M commit")
        self.events[0] = (0, 2, 3)
        self.assertEqual(reconcile(self.events, self.snap), "unresolved")

    def test_missing_and_contradiction(self):
        self.assertEqual(causal_order({}), "missing actor")
        self.events[0] = (1, 2, 3)
        with self.assertRaises(ValueError):
            reconcile(self.events, self.snap)

    def test_wrong_slot_or_lease(self):
        for event in ((0, 1, 3), (0, 2, 0), (0, 2, 2)):
            self.events[0] = event
            with self.assertRaises(ValueError):
                reconcile(self.events, self.snap)

    def test_tampered_arithmetic(self):
        self.rows[1]["words"] = (0, 3, 3, 65536, 0, 0, 0x801f0, 0)
        with self.assertRaises(ValueError):
            reconcile(self.events, self.snap)

    def test_wrong_sgir(self):
        self.events[1] = (1, 0x28001, 0)
        with self.assertRaises(ValueError):
            reconcile(self.events, self.snap)


if __name__ == "__main__":
    unittest.main()
