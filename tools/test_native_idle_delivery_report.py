"""Offline mathematical controls; no guest or host acquisition."""
import unittest

from native_idle_delivery_report import contribution, projection


def anchor(t, due, outcome=1):
    return {"due_ticks":due,"outcome":outcome,"samples":[
        {"seq":[t+j,t+j+1],"guest_ticks":v,"host_ns":[1000+16*v-1,1000+16*v+1]}
        for j,v in enumerate((t,t+8))]}


class IdleDeliveryBounds(unittest.TestCase):
    def test_no_interpolation_without_external_across_idle_bound(self):
        rows=[anchor(10,12),anchor(30,34,2),anchor(50,53)]
        causal=projection(rows,1,None,1)
        self.assertEqual(causal["basis"],"causal only")
        self.assertGreater(causal["due_ns"][1],causal["irq_ns"][0])
        matched=projection(rows,1,{"rate_error_ppb":0,"offset_error_ns":0},1)
        self.assertTrue(all(matched["held_out_overlap"]))
        self.assertLess(matched["due_ns"][1],matched["irq_ns"][0])
        rows[2]["samples"][0]["host_ns"]=[5000,5001]
        self.assertEqual(projection(rows,1,{"rate_error_ppb":0,"offset_error_ns":0},1)["basis"],"causal only")

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
