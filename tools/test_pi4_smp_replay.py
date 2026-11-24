"""Synthetic journal controls, independent of board execution."""
import unittest

from pi4_smp_replay import FRAME, STACKS, TASKS, score


def fixture():
    symbols = {name: (0x81000 + role*256, 128) for role, name in enumerate(TASKS)}
    symbols[STACKS] = (0x90000, 3*16416)
    out = ["SW HEADER v=1 cap=32 frame=832"]
    stacks = [0x90010+i*16416 for i in range(3)]
    for role, lo in enumerate(stacks):
        out.append(f"SW STACK {role} {lo} {lo+16384} 1")
    journals = [[], []]
    def add(cpu,k,g,*args):
        seq = len(journals[cpu])+1
        journals[cpu].append((cpu, seq, k, g, seq*100, *args))
    def frame(k,g,role,offset=0,progress=0):
        cpu = int(role != 0)
        sp = stacks[role]+16384-(64 if offset else 0)
        add(cpu,k,g,role,sp-FRAME,symbols[TASKS[role]][0]+offset,sp,progress)
    def work(g,role,value):
        cpu = int(role != 0)
        add(cpu,3,g,role,symbols[TASKS[role]][0]+4,stacks[role]+16320,value,cpu)
    for cpu in range(2):
        add(cpu,1,0,cpu,4,62500000,1,FRAME)
    frame(2,0,0)
    frame(2,0,2)
    work(0,2,0)
    for g,total in ((1,136),(2,392),(3,0)):
        before = (g-1)*48 + 16
        frame(12,g-1,2,8)
        add(1,13,g,1,4*g,0,0,0)
        frame(14,g-1,1,0 if g==1 else 12)
        work(g-1,1,before)
        if g==3:
            break
        work(g,0,total)
        add(0,4,g,total,before,before+16,0,0)
        t = len(journals[0])*100
        add(0,5,g,t+1,t+2,0x28001,2,total)
        add(0,6,g,total,total*2,0,0,0)
        frame(8,g,1,12,before+32)
        add(1,9,g,1,0,total,4*g,4*g+1)
        frame(10,g,2,8)
        add(1,11,g,1,2,0,0,0)
        work(g,2,total*2)
        add(1,7,g,total,total*2,0,0,0)
    frame(15,2,1,16,112)
    for cpu, journal in enumerate(journals):
        fields = [cpu,len(journal),0,0, cpu,8,4,4096,1,1,0x1005,0,
                  0,1<<cpu,2,2,6,255,128,2]
        out.append("SW CORE " + " ".join(map(str,fields)))
        out.extend("SW ROW " + " ".join(map(str,r)) for r in journal)
    out.append("SW END 1 2 784 112")
    return "\n".join(out).encode(), symbols


class Replay(unittest.TestCase):
    def test_complete(self):
        raw, symbols = fixture()
        result = score(raw, symbols)
        self.assertEqual([p["result"] for p in result["products"]],[272,784])
        self.assertEqual(result["software_gaps"],[])

    def test_wrong_cpu_generation_and_missing_irq(self):
        raw, symbols = fixture()
        lines = raw.splitlines()
        idx = next(i for i,line in enumerate(lines) if line.startswith(b"SW ROW 1 ") and line.split()[4:6] == [b"9",b"1"])
        for field, value in ((2,b"0"),(5,b"2"),(None,None)):
            damaged = lines.copy()
            if field is None:
                del damaged[idx]
            else:
                words = damaged[idx].split()
                words[field] = value
                damaged[idx] = b" ".join(words)
            with self.subTest(field=field), self.assertRaises(ValueError):
                score(b"\n".join(damaged), symbols)
