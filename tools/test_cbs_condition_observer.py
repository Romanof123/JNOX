"""Host-only reader controls with synthetic bytes; no guest fault injection."""
import unittest

from cbs_condition_observer import classify, sites
from qemu_boot import CBS_TRANSCRIPT


class ConditionControls(unittest.TestCase):
    def setUp(self):
        self.dictionary = sites(b"site,mask,name,a,b,c,condition_en,condition_ko\n11,5,example,left,,right,example,example\n")
        self.prefix = b"".join(CBS_TRANSCRIPT.splitlines(True)[:6])
        self.line = b"CBS DIAG1 11 5 0 0 2\n"
        self.fail = b"CBS FAIL\n"

    def score(self, data, diagnostic=True):
        return classify(data,self.dictionary,diagnostic)

    def test_original_success_and_failures(self):
        for data in (CBS_TRANSCRIPT,CBS_TRANSCRIPT.replace(b"\n",b"\r\n")):
            self.assertEqual(self.score(data)["uart"],"ordinary_success")
        result = self.score(self.prefix+self.line+self.fail)
        self.assertEqual((result["uart"],result["prefix_lines"]),("diagnosed_failure",6))
        self.assertEqual(result["operands"],[{"name":"left","value":0},{"name":"right","value":2}])
        self.assertEqual(self.score(self.prefix+self.fail)["uart"],"ordinary_failure_cause_unobserved")
        for raw in (self.prefix,self.prefix+self.line[:-2],self.prefix+self.line):
            self.assertEqual(self.score(raw)["uart"],"partial")

    def test_comparison_and_policy_variant(self):
        data = (b"site,mask,name,a,b,c,condition_en,condition_ko\n"
                b"22,7,irq_window,counter,begin,end,b <= a < c,window\n"
                b"42,7,tick_missed_deadline,slot,error,server,tick returned missed deadline,error\n")
        dictionary = sites(data)
        for row in (b"22 7 30 20 30",b"42 7 8 4 0"):
            result = classify(self.prefix+b"CBS DIAG1 "+row+b"\n"+self.fail,dictionary,True)
            self.assertEqual(result["uart"],"diagnosed_failure")
        for row in (b"22 7 29 20 30",b"42 7 8 1 0"):
            with self.assertRaises(ValueError):
                classify(self.prefix+b"CBS DIAG1 "+row+b"\n"+self.fail,dictionary,True)

    def test_reject_fabricated_records(self):
        for line in (self.line.replace(b"11",b"12"),self.line.replace(b" 5 ",b" 3 "),
                     self.line.replace(b"0 0",b"0 8"),self.line.replace(b"11",b"011"),
                     self.line.replace(b"0 0",b"-1 0"),self.line.replace(b" 2\n",b" 18446744073709551616\n"),
                     self.line+self.line,self.line.replace(b"\n",b"\r\n")):
            with self.subTest(line=line), self.assertRaises(ValueError):
                self.score(self.prefix+line+self.fail)
        for data in (self.prefix+self.line+self.fail+b"x",CBS_TRANSCRIPT+self.line+self.fail,
                     self.prefix.replace(b"CBS IDLE\n",b"")+self.line+self.fail):
            with self.subTest(data=data), self.assertRaises(ValueError):
                self.score(data)
        with self.assertRaises(ValueError):
            self.score(self.prefix+self.line+self.fail,False)


if __name__ == "__main__":
    unittest.main()
