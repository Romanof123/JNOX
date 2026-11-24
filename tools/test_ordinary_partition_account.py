"""Source-only synthetic controls for the ordinary partition accounting tool."""
import argparse
import sys
import unittest

from ordinary_partition_account import reconstruct

# Enumerate complete nominal slots independently of the production dispatcher.
P = [(0, 2, 0, 0), (2, 3, 1, 2), (3, 4, 2, 2)]
Q = [(0, 1, 1, 2), (1, 2, 2, 2), (2, 4, 0, 0), (4, 5, 1, 1)]


def synthetic(table, case):
    layout = P if table == "P" else Q
    period = layout[-1][1]
    slots = [(f*period+s, f*period+t, owner, owner if f else first)
             for f in range(4) for s,t,owner,first in layout]
    epoch, q, freq = 100000, 1000, 20000
    close = 2 if table == "P" else 4
    rows = []
    def row(kind, **fields):
        rows.append("OP " + kind + " " + " ".join(f"{k}={v}" for k,v in fields.items()))
    def time(tick):
        return epoch + q*tick
    idx = 0
    n = 0
    due = epoch
    entry = 0
    previous = 3
    saved = {}
    work = [0,0,0]
    ends = [0x14000,0x18000,0x1c000]
    while True:
        start, end, owner, chosen = slots[idx]
        live = entry + 1 if n else epoch + 1
        target = time(end)
        offset = (1250 if case == "long" else 21) if end == close and case != "healthy" else 0
        row("TIMER", n=n,d=target,p=live+1,c=target+offset,after=live+2)
        out_pc, out_sp, out_hash = saved.get(previous,(0,0,0))
        pc, sp, fingerprint = saved.get(chosen, (0x4000+chosen*256, ends[chosen], 0))
        row("EDGE",n=n,due=due,entry=entry,live=live,ret=live+3,next=target,owner=owner,chosen=chosen,
            **{"from":previous},last=entry-1 if n else 0,out_count=work[previous] if n else 0,
            out_pc=out_pc,out_sp=out_sp,out_hash=out_hash,in_pc=pc,in_sp=sp,in_hash=fingerprint)
        work[chosen] += 1
        row("OBS",n=n,x=live+4,task=chosen,sp=ends[chosen]-16,count=work[chosen],value=17+chosen)
        previous = chosen
        due = target
        entry = target + offset + 1
        saved[chosen] = 0x4010+chosen*256, ends[chosen]-16, work[chosen]
        work[chosen] += 10
        n += 1
        if start == 3*period:
            out_pc, out_sp, out_hash = saved[chosen]
            row("STOP",due=due,entry=entry,**{"from":chosen},last=entry-1,out_count=work[chosen],
                out_pc=out_pc,out_sp=out_sp,out_hash=out_hash)
            break
        idx += 1
        while True:
            s,t,nominal,ready = slots[idx]
            skipped = time(t) <= entry
            row("BOUND",n=n,tick=s,end=t,owner=nominal,runnable=ready,skipped=int(skipped))
            if not skipped:
                break
            idx += 1
    header = [f"OP HEADER table={table} epoch={epoch} freq={freq} q={q} n={n}"]
    return header + [f"OP STACK id={i} lo={hi-16384} hi={hi}" for i,hi in enumerate(ends)] + rows


class Controls(unittest.TestCase):
    def test_six_synthetic_cases(self):
        for table in ("P","Q"):
            for case in ("healthy","near","long"):
                with self.subTest(table=table,case=case):
                    result = reconstruct(synthetic(table,case),table,case,"synthetic")
                    self.assertEqual(result["timing_envelope_met"], case == "healthy")
                    self.assertEqual(len(result["missed"]), int(case=="long"))
                    if case == "long":
                        lost = "idle" if table == "P" else "B"
                        self.assertGreaterEqual(result["measured_service_loss_ticks"][lost],1250-1)

    def test_exact_limit_and_one_tick_above(self):
        for offset in (20,21):
            rows = synthetic("P","healthy")
            i = next(i for i,row in enumerate(rows) if row.startswith("OP OBS n=0 "))
            rows[i] = rows[i].replace("x=100005", f"x={100000+offset}")
            if offset == 20:
                self.assertTrue(reconstruct(rows,"P","healthy","synthetic")["timing_envelope_met"])
            else:
                with self.assertRaises(ValueError):
                    reconstruct(rows,"P","healthy","synthetic")

    def test_false_resume(self):
        rows = synthetic("Q","healthy")
        # Opening blocked B and first explicit idle select the same idle context.
        i = next(i for i,row in enumerate(rows) if row.startswith("OP EDGE n=1 "))
        rows[i] = " ".join("in_pc=123" if x.startswith("in_pc=") else x for x in rows[i].split())
        with self.assertRaises(ValueError):
            reconstruct(rows,"Q","healthy","synthetic")

    def test_contradictions(self):
        for tag, field, value in [("TIMER","c",0),("EDGE","chosen",1),("OBS","sp",0),
                                  ("EDGE","due",45),("BOUND","skipped",1),("OBS","x",0)]:
            rows = synthetic("P","healthy")
            i = next(i for i,row in enumerate(rows) if row.startswith("OP "+tag+" "))
            rows[i] = " ".join(f"{field}={value}" if x.startswith(field+"=") else x for x in rows[i].split())
            with self.subTest(field=field), self.assertRaises(ValueError):
                reconstruct(rows,"P","healthy","synthetic")
        rows = synthetic("Q","healthy")
        with self.assertRaises(ValueError):
            reconstruct(rows[:-1],"Q","healthy","synthetic")
        with self.assertRaises(ValueError):
            reconstruct(rows,"Q","long","synthetic")


if __name__ == "__main__":
    if sys.argv[1:2] == ["--emit"]:
        parser = argparse.ArgumentParser()
        parser.add_argument("--emit", choices=("P","Q"), required=True)
        parser.add_argument("case", choices=("healthy","near","long"))
        args = parser.parse_args()
        print("\n".join(synthetic(args.emit, args.case)))
    else:
        unittest.main()
