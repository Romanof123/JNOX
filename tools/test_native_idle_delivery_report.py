"""Offline mathematical controls; no guest or host acquisition."""
import unittest

from native_idle_delivery_report import contribution, projection


def anchor(t, due, outcome=1):
    return {"due_ticks":due,"outcome":outcome,"samples":[
        {"seq":[t+j,t+j+1],"guest_ticks":v,"host_ns":[1000+16*v-1,1000+16*v+1]}
        for j,v in enumerate((t,t+8))]}


class IdleDeliveryBounds(unittest.TestCase):
    def test_held_out_fit_never_replaces_causal_interval(self):
        rows=[anchor(10,12),anchor(30,34,2),anchor(50,53)]
        causal=projection(rows,1,1)
        self.assertEqual(causal["basis"],"causal only")
        self.assertGreater(causal["due_ns"][1],causal["irq_ns"][0])
        self.assertTrue(all(causal["held_out_overlap"]))
        self.assertEqual(causal["due_ns"],causal["causal_due_ns"])
        rows[2]["samples"][0]["host_ns"]=[5000,5001]
        inconsistent=projection(rows,1,1)
        self.assertFalse(all(inconsistent["held_out_overlap"]))
        self.assertEqual(inconsistent["due_ns"],inconsistent["causal_due_ns"])

    def test_deferred_wait_booking_and_uncovered_edges(self):
        s={"tid":11,"starttime":20,"rows":[(97,101,[0,0,0,0,0]),(103,104,[2,4,1,0,0]),
                                          (117,118,[5,16,2,0,0]),(119,125,[6,27,3,0,0])]}
        for witness,cal,lower in ((None,True,0),([80,81],True,0),([95,96],False,0),([95,96],True,7)):
            result=contribution(s,100,120,witness,cal)
            self.assertEqual(result["inside_samples"],2)
            self.assertEqual(result["booked_wait_ns"],12)
            self.assertEqual(result["runnable_wait_lower_ns"],lower)
            self.assertEqual(result["uncovered_edges_upper_ns"],7)


if __name__=="__main__":
    unittest.main()
