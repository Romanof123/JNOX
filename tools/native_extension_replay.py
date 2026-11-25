#!/usr/bin/env python3
"""Offline replay of the three-slot native extension deployment from native_el0_capture.py."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

from native_el0_replay import PA_MASK, Snapshot, argument_stacks, elf_file, permissions, symbol, vread
from native_extension_program import parse_program
from ordinary_amc_capture import FLAGS, sha

NONE = 2**64 - 1
GEN = [7, 11, 17]
COLS = ("cause op entry now old_cell cell start cutoff end src dst old_compare compare skipped old_eval "
        "evaluating published canceled reason steps heartbeat mask grant woke exited src_pc src_sp dst_pc "
        "dst_sp src_root dst_root pre_return idle_landing arg sleep0 sleep1 sleep2 cval ctl result").split()


def check(ok, message):
    if not ok:
        raise ValueError(message)


def load_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def journal(uart):
    config, loss, index, rows, exits = None, None, None, [], {}
    for line in uart.splitlines(keepends=True):
        if line.startswith(b"NATIVE EXTENSION "):
            check(line.endswith(b"\n"), "truncated extension output")
            _, _, tag, *fields = line.split()
            check(all(re.fullmatch(rb"[0-9]{1,20}", f) for f in fields), "invalid UART number")
            data = list(map(int, fields))
            check(all(v <= NONE for v in data), "UART u64 overflow")
            if tag == b"CONFIG":
                check(config is None and len(data) == 7, "CONFIG")
                config = data
            elif tag == b"INDEX":
                check(config and loss is None and index is None and data == [len(rows)] and len(rows)<512, "INDEX")
                index = data[0]
            elif tag == b"EVENT":
                check(index == len(rows) and len(data) == len(COLS), "EVENT")
                rows.append(dict(zip(COLS,data)))
                index = None
            elif tag == b"DROPPED":
                check(config and index is None and loss is None and len(data)==1, "DROPPED")
                loss = data[0]
            else:
                raise ValueError("unknown extension record")
        elif line.startswith(b"NATIVE EL0 EXIT "):
            m = re.fullmatch(rb"NATIVE EL0 EXIT PID ([012]) (-?[0-9]+)\r?\n",line)
            check(m and loss is not None and int(m[1]) not in exits, "invalid process exit")
            exits[int(m[1])] = int(m[2])
    check(config and loss is not None and len(exits)==3, "incomplete terminal UART")
    return config, rows, loss, [exits[i] for i in range(3)]


def nominate(ops, mask):
    pc = 0
    for steps in range(1,6):
        op,*args = ops[pc].split()
        args = list(map(int,args))
        if op == "jump":
            pc = args[0]
        elif op == "ready":
            task,yes,no = args
            pc = yes if mask & (1<<task) else no
        elif op == "yield":
            return 6,steps,None
        else:
            task,generation,cpu,ticks = args
            accepted = task < 3 and mask & (1<<task) and generation==GEN[task] and cpu==0 and ticks==1
            return (0 if accepted else 1),steps,args
    return 2,5,None


def products(mem, images, roots, slots):
    sleep = slots[0]["argv"] == ["/bin/app","sleep-400"]
    four = slots[2]["argv"] == ["/bin/app","four-batches"]
    counts = [[2,1],[1,1,1],[4096*256]*(4 if four else 3)]
    sums = [[6596,3304],[18,23,21],[65536]*len(counts[2])]
    result = []
    for i in range(3):
        def read(name, fmt):
            va, size = symbol(images[i],name)
            check(size==struct.calcsize("<"+fmt), "unexpected product width: "+name)
            permissions(mem,roots[i],va,"user rw")
            return list(struct.unpack("<"+fmt,vread(mem,roots[i],va,size)))
        n = len(counts[i])
        actual = dict(batches=read("private_batches","I")[0], counts=read("batch_count","4I"),
                      sums=read("batch_sum","4Q"), cumulative=read("cumulative_sum","4Q"),
                      total=read("private_result","Q")[0])
        pad = [0]*(4-n)
        actual["matches"] = (actual["batches"]==n and actual["counts"]==counts[i]+pad
                             and actual["sums"]==sums[i]+pad and
                             actual["cumulative"]==[sum(sums[i][:j+1]) for j in range(n)]+pad
                             and actual["total"]==sum(sums[i]))
        if i==0 and sleep:
            actual["requested_ns"], = read("request_ns","Q")
            actual["observed_ns"], = read("observed_ns","Q")
        result.append(actual)
    return result


def trace_returns(raw):
    return [int(v,16) for v in re.findall(
        rb"(?m)^Exception return from AArch64 EL1 to AArch64 EL0 PC (0x[0-9a-fA-F]+)\r?$",raw)]


def trace_entries(raw):
    result = []
    pending = None
    level = None
    for line in raw.splitlines():
        if line.startswith(b"Taking exception"):
            m = re.fullmatch(rb"Taking exception [0-9]+ \[(IRQ|SVC)\](?: on CPU 0)?",line)
            pending = m[1].decode() if m else None
            level = None
        elif pending and line.startswith(b"...from EL"):
            m = re.fullmatch(rb"\.\.\.from EL([01]) to EL1",line)
            level = int(m[1]) if m else None
        elif pending and line.startswith(b"...with ELR "):
            if level is not None:
                result.append((pending,level,int(line.split()[-1],16)))
            pending = None
    return result


def analyze(rows, config, ops, roots, mem, dropped, initial, sleep_mode):
    version,epoch,hz,q,fuel,watchdog,period = config
    check(version==1 and (fuel,watchdog,period)==(5,2,3) and epoch and q==hz//10 and q//4>0, "clock/config")
    issues, cells, returns = [], {}, []
    hb = last_change = exited = 0
    sleeps = [0,0,0]
    ready_since = [epoch]*3
    prev = None
    frames = list(initial)
    slot_exits = {}
    for idx,r in enumerate(rows):
        k,src,dst,entry,now = [r[f] for f in ("cell","src","dst","entry","now")]
        irq = r["cause"]==1 and r["op"] & 1023 == 30
        check(src==(prev["dst"] if prev else 3) and r["old_compare"]==(prev["compare"] if prev else epoch)
              and r["old_cell"]==(prev["cell"] if prev else NONE) and entry <= now
              and (not prev or prev["now"] <= entry), f"event {idx} continuity")
        check(src<=3 and dst<=4 and r["cause"] in (1,2,3), "owner/cause")
        if r["cause"]==2:
            check(src<3, "SVC owner")
            if r["op"]==1:
                exited |= 1<<src
                slot_exits[src] = struct.unpack("<i",struct.pack("<I",r["arg"] & 0xffffffff))[0]
            if r["op"]==7 and r["result"]==0 and r[f"sleep{src}"]:
                sleeps[src] = ready_since[src] = r[f"sleep{src}"]
        if src<3:
            frames[src] = (r["src_pc"],r["src_sp"])
            check(r["src_root"] & PA_MASK == roots[src], "exception came from wrong address space")
            permissions(mem,roots[src],r["src_pc"],"user rx")
            check(0x1001000 <= r["src_sp"] <= 0x1011000, "source stack")
        if dst<3:
            check(frames[dst] == (r["dst_pc"],r["dst_sp"]), "restored continuation mismatch")
            permissions(mem,roots[dst],r["dst_pc"],"user rx")
            check(0x1001000 <= r["dst_sp"] <= 0x1011000, "target stack")
            if r["pre_return"]:
                check(r["dst_root"] & PA_MASK == roots[dst], "returned root mismatch")
                returns.append(r["dst_pc"])
        check(r["exited"]==exited, "exit mask")
        opening = k!=r["old_cell"]
        if opening:
            check(irq and k==(now-epoch)//q and r["start"]==epoch+k*q and r["cutoff"]==r["start"]+q//4
                  and r["end"]==r["start"]+q, "cell opening/grid")
            mask = sum(1<<j for j in range(3) if not exited & (1<<j) and ready_since[j] <= r["start"]
                       and (not sleeps[j] or sleeps[j]<=r["start"]))
            check(r["mask"]==mask, "opening snapshot")
            cells[k] = dict(cell=k, start=r["start"], snapshot=mask, opening_irq=entry,
                            decision=None, work_upper=None, work_lower=None, cutoff_irq=None)
            if prev and r["old_cell"] in cells:
                old = cells[r["old_cell"]]
                old["cutoff_irq"] = entry
                if not (prev["end"] <= entry < prev["end"]+q//4 and r["old_compare"]==prev["end"]
                        and r["cval"]==prev["end"] and r["ctl"] & 7==5):
                    issues.append(f"cell {old['cell']}: service cutoff PPI/compare/bound")
            if r["skipped"]:
                issues.append(f"skipped {r['skipped']} cells before {k}")
        if not opening and k != NONE:
            check((r["start"],r["cutoff"],r["end"])==(epoch+k*q,epoch+k*q+q//4,epoch+(k+1)*q),
                  "changed fixed cell endpoints")
        if r["reason"]==NONE and src!=dst and dst!=4:
            check(dst==3 and (opening or r["cause"] in (2,3)), "unchecked owner change")
        if r["cause"]==2 and r["op"] in (1,8) and dst not in (3,4):
            raise ValueError("yield/exit retained the grant")
        if dst!=4:
            intended = epoch if k==NONE else r["cutoff"] if r["evaluating"] else r["end"]
            check(r["compare"]==min([intended]+[r[f"sleep{j}"] for j in range(3) if r[f"sleep{j}"]]),
                  "programmed compare does not track deadline/wake")
        if r["reason"]==NONE:
            check(not r["steps"] and not r["grant"] and not r["published"], "decision fields without decision")
        if k in cells:
            cell = cells[k]
            check(r["mask"]==cell["snapshot"], "changed snapshot")
            if r["reason"]!=NONE:
                check(cell["decision"] is None and irq, "duplicate or non-PPI decision")
                mask = r["mask"] & (1 if k%3==2 else 6)
                proposal = None
                if k%3==2:
                    reason,steps = 7,0
                    check(not r["published"] and not r["evaluating"] and opening, "reservation bypass")
                elif r["published"]:
                    check(r["start"] <= r["published"] < r["cutoff"] <= now and not r["canceled"], "late evaluator reply")
                    hb += 1
                    last_change = k
                    reason,steps,proposal = nominate(ops,mask)
                else:
                    check(r["canceled"] and now>=r["cutoff"], "missing cancellation")
                    reason,steps = (3 if k-last_change>=2 else 8),0
                if k%3!=2 and (r["old_compare"]!=r["cutoff"] or r["cval"]!=r["cutoff"] or
                                entry<r["cutoff"] or r["ctl"] & 7!=5):
                    issues.append(f"cell {k}: evaluation comparator not witnessed")
                selected = proposal[0] if reason==0 else next((j for j in range(3) if mask & (1<<j)),3)
                check((r["reason"],r["steps"],dst,r["grant"])==(reason,steps,selected,int(selected<3)), "VM/gate selection")
                cell.update(decision=idx, owner=dst,reason=reason,steps=steps,proposal=proposal,
                            heartbeat=hb, publication=r["published"], selection_irq=entry)
                if dst==3 and r["idle_landing"]:
                    cell["work_upper"] = r["idle_landing"]
                    cell["witness"] = "actual idle landing"
            check(r["heartbeat"]==hb, "heartbeat")
        # A fixture yield or exit is issued after completing a batch. It gives a
        # conservative upper bound only within an uninterrupted grant of this owner.
        if prev and src<3 and r["cause"]==2 and r["old_cell"] in cells:
            old=cells[r["old_cell"]]
            post_clock = sleep_mode and src==0 and r["op"]==6 and r["result"]==0 and not any(
                prior["cause"]==2 and prior["src"]==0 and prior["op"]==7 for prior in rows[:idx])
            if old.get("owner")==src and old["work_upper"] is None and (r["op"] in (1,8) or post_clock):
                old["work_upper"]=entry
                old["witness"]="post-batch monotonic read" if post_clock else "post-batch user yield/exit SVC"
        if dst<3 and k in cells and cells[k]["work_lower"] is None and r["pre_return"]:
            cells[k]["work_lower"] = r["pre_return"]
        woke = 0
        for j,t in enumerate(sleeps):
            if t and t<=now and dst!=4:
                sleeps[j]=0
                woke |= 1<<j
        check(woke==r["woke"] and sleeps==[r[f"sleep{j}"] for j in range(3)], "wake ledger")
        prev=r
    if dropped:
        issues.append(f"{dropped} events unavailable after retained prefix")
    else:
        check(prev and prev["dst"]==4 and prev["exited"]==7 and prev["compare"]==0, "terminal cancel")
    for k,cell in cells.items():
        upper,lower,limit=cell["work_upper"],cell["work_lower"],cell["start"]+q//2
        cell["first_work"]="pass" if upper is not None and cell["start"]<=upper<limit else (
            "late" if lower is not None and lower>=limit else "incomplete")
        if cell["decision"] is None or cell["first_work"]!="pass":
            issues.append(f"cell {k}: first work {cell['first_work']}")
    return list(cells.values()),returns,issues,slot_exits


def replay(package,capture,digest):
    check(re.fullmatch("[0-9a-f]{64}",digest) and sha(capture/"session.json")==digest, "session digest")
    hashes=load_json(capture/"session.json")["hashes"]
    for name in ("kernel.elf","ram.bin","uart.bin","launch.json","user.elf"):
        check(sha(capture/name)==hashes[name], f"capture digest {name}")
    launch=load_json(capture/"launch.json")
    m=load_json(package/"manifest.json")
    check(m==launch["argument_package"] and m["policy"]=="extension" and m["board"]=="virt"
          and m["kernel_elf_sha256"]==hashes["kernel.elf"] and m["commit"]==launch["revision"], "package identity")
    slots=m["slots"]
    check(len(slots)==3 and [s["generation"] for s in slots]==GEN and all(s["cpu"]==0 for s in slots)
          and [s["role"] for s in slots]==["reserved","delegated","delegated"], "extension roles")
    rule=(package/"extension_program.txt").read_bytes()
    ops=parse_program(rule)
    check(rule==("\n".join(ops)+"\n").encode() and ops==m["extension_program"]["operations"]
          and hashlib.sha256(rule).hexdigest()==m["extension_program"]["sha256"], "canonical extension identity")
    uart=(capture/"uart.bin").read_bytes()
    config,rows,dropped,exits=journal(uart)
    mem=Snapshot(launch["ram_base"],(capture/"ram.bin").read_bytes())
    info=argument_stacks((capture/"kernel.elf").read_bytes(),mem,m,uart)
    images=[]
    for i,name in enumerate(("user.elf","native_peer.elf","native_peer2.elf")):
        image=package/name
        check(sha(image)==slots[i]["elf_sha256"],f"slot {i} identity")
        images.append(elf_file(image.read_bytes()))
    roots=[s["root"] for s in info]
    sleep_mode = slots[0]["argv"]==["/bin/app","sleep-400"]
    extra_batch = slots[2]["argv"]==["/bin/app","four-batches"]
    check(slots[0]["argv"] in (["/bin/app"],["/bin/app","sleep-400"]) and
          slots[1]["argv"]==["/bin/app"] and slots[2]["argv"] in (["/bin/app"],["/bin/app","four-batches"]),
          "reader expects deployed voltage/temperature/alarm fixtures")
    initial=[(image["e_entry"],slot["initial_sp"]) for image,slot in zip(images,info)]
    cells,returns,issues,slot_exits=analyze(rows,config,ops,roots,mem,dropped,initial,sleep_mode)
    check(all(exits[i]==status for i,status in slot_exits.items()), "SVC and terminal exits differ")
    actual=products(mem,images,roots,slots)
    if not all(s["matches"] for s in actual) or exits!=[30,31,32]:
        issues.append("private products or process exits disagree")
    # The normal fixture criterion is one completed batch per uninterrupted grant.
    # For sleep mode its first post-batch clock is an explicit observation; the later
    # read after resuming sleep precedes the second computation.
    if not sleep_mode:
        accepted = ops==["ready 2 1 2","propose 2 17 0 1","yield"]
        expected = [2,2,0,2,1,0,1,1] if accepted else [1,1,0,1,2,0,2,2]
        if extra_batch and accepted:
            expected = [2,2,0,2,2,0,1,1,3,1]
        named = accepted or ops in (["propose 2 16 0 1"],["propose 2 17 1 1"],["jump 0"])
        if named and [c.get("owner") for c in cells]!=expected:
            issues.append("actual serviced cells differ from frozen owner prediction")
    else:
        sleeps=[r for r in rows if r["cause"]==2 and r["src"]==0 and r["op"]==7 and r["result"]==0]
        wakes=[r for r in rows if r["woke"] & 1]
        clocks=[r["entry"] for r in rows if r["cause"]==2 and r["src"]==0 and r["op"]==6 and r["result"]==0]
        if len(sleeps)!=1 or len(wakes)!=1 or len(clocks)!=2:
            issues.append("incomplete reserved sleep/wake/read sequence")
        else:
            t=sleeps[0]["sleep0"]
            ns0=actual[0]["requested_ns"]-400_000_000
            ns1=actual[0]["observed_ns"]
            hz=config[2]
            elapsed=clocks[0]-ns0*hz//1_000_000_000
            # The frozen 62.5 MHz clock gives exactly 16 ns per tick.
            if hz!=62_500_000 or ns0<0 or ns0%16 or ns1%16 or ns1<ns0+400_000_000 or (
                    elapsed+ns1//16!=clocks[1] or t!=elapsed+(ns0+400_000_000)//16 or
                    not t<=wakes[0]["now"]<=clocks[1] or
                    not any(c.get("owner")==3 and c["cell"]%3==2 and sleeps[0]["entry"]<c["start"]<t for c in cells)):
                issues.append("reserved sleep target/idle window/post-wake sample mismatch")
    if config[2]!=62_500_000:
        issues.append("counter frequency differs from the frozen deployment")
    argv=launch["argv"]
    check(argv[1:1+len(FLAGS)]==FLAGS and launch["kernel_sha256"]==hashes["kernel.elf"]
          and launch["user_sha256"]==hashes["user.elf"]==slots[0]["elf_sha256"],"capture machine/image identity")
    plain="-d" not in launch["argv"]
    if not plain:
        check(sha(capture/"exceptions.log")==hashes["exceptions.log"],"trace digest")
        raw=(capture/"exceptions.log").read_bytes()
        observed=trace_returns(raw)
        entries=trace_entries(raw)
        expected_entries=[("IRQ" if r["cause"]==1 else "SVC", int(r["src"]==3), r["src_pc"])
                          for r in rows if r["cause"] in (1,2)]
        if not expected_entries or entries[-len(expected_entries):]!=expected_entries:
            issues.append("external exception source level/cause/PC stream differs from journal")
        if observed!=returns:
            issues.append("EL0 ERET stream differs from recorded returns")
    return dict(session_sha256=digest,config=config,program=ops,slots=slots,roots=roots,
                products=actual,exits=exits,dropped=dropped,cells=cells,events=rows,plain=plain,
                expected_eret_pcs=returns,issues=issues)


if __name__=="__main__":
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("package",type=Path)
    p.add_argument("capture",type=Path)
    p.add_argument("--session-sha256",required=True)
    args=p.parse_args()
    try:
        result=replay(args.package,args.capture,args.session_sha256)
    except (ValueError,OSError,KeyError,ImportError,IndexError,TypeError,struct.error) as exc:
        p.exit(2,f"incomplete or inconsistent extension evidence: {exc}\n")
    print(json.dumps(result,indent=2))
    sys.exit(1 if result["issues"] else 0)
