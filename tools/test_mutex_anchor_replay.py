"""Host-only invented records exercise decoding; no kernel or emulator is invoked."""
import unittest

from mutex_anchor_replay import APPS, MODES, NA, WIDTH, Invalid, check_anchor, parse


def fixture(mode="selected-return", app="ordinary-pi"):
    ce = APPS[app]
    phase = MODES[mode]
    rows = {key: [] for key in WIDTH}
    rows["H"] = [[ce, 0, 2, 2, 256]]
    start, restore = (100,90) if phase == 3 else (100,110)
    rows["T"] = [[62500000,start,6350,6350,1,30,5,6350]]
    rows["W"] = [[1,phase,4,{1:271,2:383,3:399}[phase],1,2,0x1000,0x4000,0x345,1,0,ce,6250]]
    rows["P"] = [[101,102,105,restore,0,NA] if phase == 2 else
                 [0,0,0,restore,0,NA] if phase == 1 else [0,0,0,restore,105,0]]
    rows["S"] = [[i, p, 0x1000+i*16384, 0x1000+(i+1)*16384,832]
                 for i,p in enumerate([2,5,9,0])]
    e = [0]*26
    e[:9] = [50,60,1,ce,0,0,0x1000,0x4000,0x345]
    e[10:12] = [0x1000,0x4000]
    e[23:26] = [1,1,0]
    irq = e.copy()
    irq[:6] = [6500,6510,5,0,0,2]
    irq[20:22] = [1,1]
    rows["E"] = [e,irq]
    rows["R"] = [[0x1000,0x4100,40],[0x1000,0x4000,restore]]
    rows["C"] = [[k*256,k*2176] for k in range(1,257)]
    rows["B"] = rows["HIGH"] = [[1,65536,557056]]
    rows["A"] = [[ce,256*ce]]
    rows["M"] = [[ce,256*ce,32896]]
    rows["K"] = [[0]*9]
    return rows


def wire(rows):
    lines = ["JNOX M1 BOOT", "JNOX M1 READY", "MUTEX RAW 2"]
    for tag in WIDTH:
        for row in rows[tag]:
            lines.append(tag + " " + " ".join(f"{v:016x}" for v in row))
    return ("\n".join(lines) + "\nMUTEX END\n").encode()


class ArchiveOnly(unittest.TestCase):
    def test_variants(self):
        for app in APPS:
            for mode in MODES:
                with self.subTest(app=app, mode=mode):
                    self.assertTrue(check_anchor(parse(wire(fixture(mode,app))),app,mode)["anchor_verified"])

    def test_flags_and_identity(self):
        for tag, index, field, value in [
            ("W",0,1,1), ("W",0,2,2), ("W",0,3,271), ("W",0,4,2), ("W",0,5,1),
            ("W",0,6,0x1010), ("W",0,7,0x4100), ("E",0,14,2), ("E",0,25,1),
            ("T",0,3,6349), ("T",0,4,3), ("T",0,1,NA), ("P",0,0,1), ("P",0,5,1)
        ]:
            with self.subTest(tag=tag,field=field):
                rows=fixture()
                rows[tag][index][field]=value
                with self.assertRaises(Invalid):
                    check_anchor(parse(wire(rows)), "ordinary-pi", "selected-return")

    def test_duplicate_acquire_and_stages(self):
        rows=fixture()
        rows["E"].append(rows["E"][0].copy())
        rows["H"][0][2]+=1
        with self.assertRaises(Invalid):
            check_anchor(parse(wire(rows)), "ordinary-pi", "selected-return")
        rows=fixture("rust-stages")
        rows["P"][0][2]=0
        with self.assertRaises(Invalid):
            check_anchor(parse(wire(rows)), "ordinary-pi", "rust-stages")

    def test_failure_and_missing_footer(self):
        rows=fixture()
        rows["H"][0][1]=16
        result=check_anchor(parse(wire(rows)),"ordinary-pi","selected-return")
        self.assertEqual((result["guest_reason"],result["anchor_verified"]),(16,False))
        with self.assertRaises(Invalid):
            parse(wire(rows)[:-1])

    def test_elapsed_is_observed(self):
        rows=fixture()
        rows["P"][0][4:6]=[6400,1]
        rows["T"][0][4]=5
        result=check_anchor(parse(wire(rows)),"ordinary-pi","selected-return")
        self.assertTrue(result["elapsed_at_final_sample"])


if __name__ == "__main__":
    unittest.main()
