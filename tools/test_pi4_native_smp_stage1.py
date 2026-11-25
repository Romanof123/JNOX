"""Synthetic timestamps exercise the partial order only."""
import unittest

from pi4_native_smp_stage1 import evaluate


def row(cpu, index, compare, post=None):
    result = dict(cpu=cpu, row=index, resume=compare-12,
                  prior_cval=compare, entry=compare+1)
    if post is not None:
        result["el0_after_return"] = post
    return result


class ContinuedUserTests(unittest.TestCase):
    def setUp(self):
        self.x = row(0, 0, 10)
        self.v = row(1, 0, 30)
        self.y = row(0, 1, 50, 24)
        self.w = row(1, 1, 70, 44)
        self.y["resume"] = 20
        self.w["resume"] = 40

    def chain(self):
        return evaluate([[(self.x, self.y)], [(self.v, self.w)]], [-2, 2])

    def test_strict_containment(self):
        self.assertTrue(self.chain()["architectural_proof"])
        self.assertEqual(self.chain()["continued_user_margins"], [4, 17, 4, 17])

    def test_uncertainty_touches_peer_trap(self):
        self.y["el0_after_return"] = 28
        self.assertFalse(self.chain()["architectural_proof"])

    def test_pre_eret_only(self):
        del self.w["el0_after_return"]
        self.assertIsNone(self.chain()["continued_user_margins"])
        self.assertFalse(self.chain()["architectural_proof"])

    def test_intervening_irq(self):
        self.y["row"] += 1
        self.assertIsNone(self.chain()["continued_user_margins"])
        self.assertFalse(self.chain()["architectural_proof"])
