#!/usr/bin/env python3
"""Stage 1 only: read one extracted, sealed #355 first. See pi4_native_smp_stage1.md."""
import argparse
import hashlib
import itertools
import json
import re
import struct
import sys
from pathlib import Path
from types import SimpleNamespace

from pi4_native_smp_replay import Ram, inspect, one, require, sha

SOURCE = "60475a0ba7c2b1092305043a65d1f6182625a8ed"
MODEL = "651f3f1b5bb6f388d29f4542f9a3d142f40ab78811807db955903edf81b7a211"
SESSIONS = {
    "p1": "d0b189700a93b9f184641303999cf269a23f293790adf9735f6d302a39ead7dc",
    "p2": "5875e85633a68623eee42486660012d84464a4362efdc86445214c238b17c720",
    "p3": "10df083a1bd5dd9131753e6233020260faa6419df3d5195538054252e3a7b3bf",
    "p4": "e16eb8114e751bf21cd20683e23a647d956dbd47cc177bb699414b4d013fc5ce",
    "p5": "b25963204192f9cd8aab604ce65ac16204f15001e7221ca50d13211c856c6948",
}
APPS = ["4e106b3e8355870dcc41bb015be760233d4eeb145de8654d8569e56513e386fa",
        "915e86f291706cbb460dadd8b0e21905cab534285bfd8c61ea5aba75f4e7b0c8"]
# ELR addresses are BEFORE the named instruction. W8 has the
# current odd A candidate after its initialization and until the final ret.
# W11 in B is defined by MOV at 40004c, then overwritten by LDR at 40007c.
# Other PCs are conservatively excluded, including B's backward CBZ target.
LIVE = [(0x400484, 0x4004c8, 8, 3, 999999),
        (0x400050, 0x40007c, 11, 1000001, 1999999)]


def frame_sha(frame):
    return hashlib.sha256(struct.pack("<102Q", *frame)).hexdigest()


def get_rows(root, ram):
    uart = (root/"uart.bin").read_text("ascii")
    def records(tag):
        return [tuple(map(int, s.split())) for s in re.findall(rf"(?m)^NATIVE SMP {tag} ([0-9 ]+)$", uart)]
    tasks = sorted(records("TASK"))
    allrows = []
    for cpu, addr, count, width, lost, frames, size in sorted(records("JOURNAL")):
        require(width == 37 and size == 816 and not lost and 0 < count <= 256, "bounded complete journal")
        per_cpu = []
        for i in range(count):
            r = ram.words(addr + i*width*8, width)
            f = ram.words(frames + 2*i*size, 102)
            g = ram.words(frames + (2*i+1)*size, 102)
            require(r[:2] == (cpu, i), "row order")
            p = dict(cpu=cpu, row=i, src=r[6], dst=r[7], cause=r[2], op=r[3], resume=r[8],
                     entry=r[4], now=r[5], pc=r[30], pstate=r[31], prior_cval=r[32],
                     prior_ctl=r[33], raw=r, saved=f, selected=g, progress=None)
            if r[6] < 2:
                lo, hi, reg, least, greatest = LIVE[r[6]]
                value = f[reg] & 0xffffffff
                if lo <= f[32] <= hi and f[32] != 0x40049c and f[32] % 4 == 0 and least <= value <= greatest and value % 2:
                    p["progress"] = value
                require(tasks[r[6]][5] == r[21] and f[32:34] == r[30:32], "root or frame order")
            per_cpu.append(p)
        allrows.append(per_cpu)
    require(len(allrows) == 2, "two journals required")
    return allrows, one(records("CONFIG")), one(records("CAL"))


def pairs(rows):
    out = [[], []]
    for per_cpu in rows:
        for x, y in zip(per_cpu, per_cpu[1:]):
            pid = x["src"]
            if pid >= 2 or pid != x["dst"] or pid != y["src"]:
                continue
            if not all(p["cause"] == 1 and p["op"] & 1023 == 30 and p["progress"] is not None
                       and p["prior_cval"] == p["raw"][10] and p["prior_ctl"] & 7 == 5
                       and p["prior_cval"] <= p["entry"] for p in (x, y)):
                continue
            if x["progress"] < y["progress"]:
                require(x["selected"] == x["saved"] and x["now"] <= y["resume"] <= y["entry"],
                        "continued same frame/return operand")
                out[pid].append((x, y))
    return out


def margin(left, right, offset):
    # Both endpoints use the SAME unknown CPU1-minus-to-CPU0 correction.
    t, cpu = left
    u, peer = right
    return min(u-t+(peer-cpu)*d for d in offset)


def evaluate(samples, offset):
    best = None
    for a, b in itertools.product(*samples):
        for (x, y), (v, w) in ((a,b), (b,a)):
            def stamp(p, key):
                return p[key], p["cpu"]
            tests = [(stamp(x,"entry"),stamp(v,"entry")), (stamp(y,"resume"),stamp(v,"entry")),
                     (stamp(v,"entry"),stamp(y,"entry")), (stamp(w,"resume"),stamp(y,"entry")),
                     (stamp(y,"entry"),stamp(w,"entry"))]
            margins = [margin(left,right,offset) for left,right in tests]
            # Actual EL0 exception sits between the active compare/return
            # and entry MRS. R precedes ERET. No bounded instruction latency
            # is inferred from the number of instructions in either stub.
            def earliest(p):
                return max(p["resume"],p["prior_cval"]), p["cpu"]
            physical = [margin(stamp(i,"entry"),earliest(j),offset) for i,j in ((x,v),(v,y),(y,w))]
            # The next EL0-origin exception bounds actual resumption from
            # above. The recorded pre-ERET operand only bounds it from below.
            closure = [margin(stamp(y,"entry"),earliest(v),offset),
                       margin(stamp(v,"entry"),earliest(y),offset),
                       margin(stamp(w,"entry"),earliest(y),offset),
                       margin(stamp(y,"entry"),earliest(w),offset)]
            candidate = dict(events=[[p["cpu"],p["row"]] for p in (x,v,y,w)],
                             timestamp_margins=margins, exception_order_margins=physical,
                             continued_user_margins=closure, architectural_proof=min(physical+closure+margins)>0)
            score = (candidate["architectural_proof"], min(margins), min(physical+closure))
            if best is None or score > best[0]:
                best = score,candidate
    return None if best is None else best[1]


def trace(root, rows, n):
    # Each header carries CPU. ERET lines in this version carry only the PC.
    text = (root/"exceptions.log").read_text()
    headers = list(re.finditer(r"(?m)^Taking exception \d+ \[(\w+)\] on CPU (\d+)\s*$", text))
    counts = [0,0]
    gaps = []
    for j,h in enumerate(headers):
        cpu = int(h[2])
        require(cpu < 2, "exception on parked CPU")
        fragment = text[h.end():headers[j+1].start() if j+1<len(headers) else len(text)]
        if counts[cpu] >= len(rows[cpu]):
            gaps.append([cpu,counts[cpu],"extra exception"])
            continue
        p = rows[cpu][counts[cpu]]
        counts[cpu] += 1
        origin = re.search(r"(?m)^\.\.\.from EL(\d+) to EL1$",fragment)
        pc = re.search(r"(?m)^\.\.\.with ELR (0x[0-9a-fA-F]+)$",fragment)
        ps = re.search(r"(?m)^\.\.\.with SPSR (0x[0-9a-fA-F]+)$",fragment)
        if not (origin and pc and ps and int(pc[1],16)==p["pc"] and int(ps[1],16)==p["pstate"]
                and int(origin[1])==(p["pstate"] & 15)//4
                and h[1]==("IRQ" if p["cause"]==1 else "SVC")):
            gaps.append([cpu,p["row"],"unbound header or source PC/PSTATE"])
    returns = [int(s,16) for s in re.findall(r"(?m)^Exception return from AArch64 EL1 to AArch64 EL0 PC (0x[\da-fA-F]+)$",text)]
    expected = [[p["selected"][32] for p in core if p["dst"] < n] for core in rows]
    # Bounded interleaving of two local ordered PC sequences. Repeated VAs
    # admit multiple assignments; never assume adjacent log lines share a CPU.
    states = {(0,0):1}
    for pc in returns:
        nxt = {}
        for key, ways in states.items():
            for cpu in (0,1):
                i=key[cpu]
                if i<len(expected[cpu]) and expected[cpu][i]==pc:
                    dest=(key[0]+(cpu==0),key[1]+(cpu==1))
                    nxt[dest]=min(2,nxt.get(dest,0)+ways)
        states=nxt
    ways=states.get(tuple(map(len,expected)),0)
    return dict(headers=counts, missing_rows=[len(c)-n for c,n in zip(rows,counts)],
                source_gaps=gaps, el0_returns=len(returns), pc_stream_assignments_capped_at_two=ways)


def remote(rows, epoch, q, offset):
    senders = [p for p in rows[0] if p["raw"][14] & 4]
    receivers = [p for p in rows[1] if p["op"] == 1 and p["cause"] == 1 and p["raw"][17] & 4]
    s, r = one(senders), one(receivers)
    data = s["raw"]
    require(data[16] and epoch+q <= data[15] <= data[16] <= data[36] and (r["src"],r["dst"]) == (1,2),
            "calendar release/targeted SGI")
    c = next(p for p in rows[1][r["row"]+1:] if p["src"] == 2 and p["cause"] == 2)
    require(c["op"] == 6, "first calendar SVC")
    # Only independent earlier batches may tighten the time offset. A
    # coalesced or unidentified mail has no unique causally paired send.
    earlier=[]
    for p in rows[0][:s["row"]]:
        row=p["raw"]
        if not (row[14]&2 and row[16] and row[16]<=row[36]):
            continue
        hits=[h for h in rows[1][:r["row"]] if h["cause"]==1 and h["op"]==1 and h["raw"][17]==2]
        if len(hits)==1:
            earlier.append(row[16]-hits[0]["entry"])
    refined = [max([offset[0],*earlier]),offset[1]]
    require(refined[0]<=refined[1],"independent SGI bound contradicts calibration")
    return dict(publish=data[15], pre_send=data[16], post_send=data[36], receiver=r["entry"],
                receiver_index=r["row"], candidate=r["progress"], first_clock=c["entry"],
                original_receive_margin=r["entry"]+offset[0]-data[16],
                independent_prior_bounds=earlier, offset_after_independent_constraint=refined,
                supplemental_receive_margin=r["entry"]+refined[0]-data[16],
                strict_deadline_margin=epoch+2*q-(c["entry"]+refined[1]))


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument("position",choices=SESSIONS)
    ap.add_argument("capture",type=Path)
    a=ap.parse_args()
    root=a.capture
    require(sha(root/"session.json")==SESSIONS[a.position],"position/session identity")
    m=json.loads((root/"manifest.json").read_bytes())
    launch=json.loads((root/"launch.json").read_bytes())
    require(m["commit"]==SOURCE and launch["qemu_sha256"]==MODEL and
            [s["elf_sha256"] for s in m["slots"][:2]]==APPS,"original source/model/ELF identity")
    first=inspect(SimpleNamespace(capture=root,session_sha256=SESSIONS[a.position],candidate_register=[8,11]))
    rows,cfg,cal=get_rows(root,Ram(root,launch["ram_base"]))
    _,freq,q,epoch,_,_=cfg
    require(q==freq//50 and freq>0,"original timer grid")
    offset=[cal[0]-cal[1],cal[2]-cal[1]]
    qualified=pairs(rows)
    report=dict(position=a.position,source=SOURCE,session_sha256=SESSIONS[a.position],
                original_replay=first,calibration=cal,offset=offset,
                margin_order=["A0<B0","A_resume<B0","B0<A1","B_resume<A1","A1<B1"],
                best_productive_chain=evaluate(qualified,offset),
                ambiguous_instruction_intervals="R precedes ERET; entry MRS follows trap. No independently timed inner EL0 instruction in these original records.",
                evidence=[])
    for cpu in rows:
        for p in cpu:
            if p["src"]<2 or p["dst"]<2:
                report["evidence"].append(dict(cpu=p["cpu"],row=p["row"],src=p["src"],dst=p["dst"],
                    elr=p["pc"],candidate=p["progress"],saved=frame_sha(p["saved"]),
                    prepared=frame_sha(p["selected"]),return_operand=p["resume"],entry=p["entry"],
                    source_root=p["raw"][21],dest_root=p["raw"][22],cause=p["cause"],op=p["op"],
                    compare=p["prior_cval"],control=p["prior_ctl"]))
    if launch["trace"]:
        report["external_trace"]=trace(root,rows,len(m["slots"]))
    if len(m["slots"])==3:
        report["remote"]=remote(rows,epoch,q,offset)
        tight=report["remote"]["offset_after_independent_constraint"]
        report["independently_refined_chain"]=evaluate(qualified,tight)
    report["advancing_ppi_pairs"]=[[[x["cpu"],x["row"],y["row"],x["progress"],y["progress"]]
                                      for x,y in group] for group in qualified]
    best=report.get("independently_refined_chain",report["best_productive_chain"])
    closed=bool(best and best["architectural_proof"])
    if launch["trace"]:
        t=report["external_trace"]
        closed &= not any(t["missing_rows"]) and not t["source_gaps"] and t["pc_stream_assignments_capped_at_two"]==1
    report["strict_architectural_overlap"]="established" if closed else "unobserved"
    print(json.dumps(report,indent=2))
    return 0 if closed else 2


if __name__=="__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, IndexError, StopIteration, struct.error) as e:
        print(f"incomplete or inconsistent stage-1 evidence: {e}",file=sys.stderr)
        sys.exit(1)
