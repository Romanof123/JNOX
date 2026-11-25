"""Capture a single AMC cold boot or score a sealed capture. Requires pyelftools."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import threading
import time

from native_amc_roots import expected_roots
from build_native_el0 import amc_manifest
from native_el0_capture import command
from native_el0_replay import Snapshot, elf_file, need, symbol, vread
from ordinary_amc_capture import save, sha
from qemu_capture import request

CASES = ("within-lo", "normal-hi", "over-hi", "staggered", "refusal", "overlap")
FIXTURES = Path(__file__).resolve().parents[1] / "examples/native-amc/config"
MAX = (1 << 64) - 1
RING = (2, 5, 1, 6, 3, 7, 4, 8)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def package(root, case):
    m = json.loads((root / "manifest.json").read_bytes())
    slots = m["slots"]
    need(m["policy"] == "amc" and 1 <= len(slots) <= 4, "AMC package")
    raw = (root / "deployment.json").read_bytes()
    desc, wire = amc_manifest(raw, len(slots))
    need(raw == (FIXTURES / (case + ".json")).read_bytes(), "named fixture input identity")
    need(desc == m["amc"] and digest(raw) == m["amc_input"]["sha256"], "raw deployment identity")
    need(wire == (root / "amc-v1.txt").read_bytes() and digest(wire) == m["amc_canonical"]["sha256"], "canonical deployment identity")
    need(sha(root / "kernel.elf") == m["kernel_elf_sha256"], "kernel identity")
    symbols = elf_file((root / "kernel.elf").read_bytes()).get_section_by_name(".symtab")
    need(symbols is not None, "kernel symbol table for private roots")
    linked = expected_roots(((s.name, s["st_value"], s["st_size"]) for s in symbols.iter_symbols()
                             if s["st_info"]["type"] == "STT_OBJECT"), len(slots))
    for i, s in enumerate(slots):
        need(type(s.get("expected_ttbr0_el1")) is int and s["expected_ttbr0_el1"] == linked[i],
             "missing or incorrect packaged private TTBR0_EL1 for slot " + str(i))
        need(s["slot"] == i and s["amc_task"] == desc["tasks"][i] and
             sha(root / f"slot{i}.elf") == s["elf_sha256"], "ordered task/image identity")
    return m


def capture(a):
    root = a.out.resolve()
    root.mkdir()
    m = json.loads((a.package / "manifest.json").read_bytes())
    files = {"manifest.json": a.package / "manifest.json", "kernel.elf": a.package / "kernel.elf",
             "amc-v1.txt": a.package / "amc-v1.txt", "deployment.json": a.input}
    for i, s in enumerate(m["slots"]):
        need(Path(s["elf"]).name == s["elf"], "local ELF filename")
        files[f"slot{i}.elf"] = a.package / s["elf"]
    for name, src in files.items():
        shutil.copyfile(src, root / name)
    package(root, a.case)
    kernel = elf_file((root / "kernel.elf").read_bytes())
    base = symbol(kernel, "_start")[0]
    size = symbol(kernel, "__stack_top")[0] - base
    need(base == 0x40200000 and 0 < size <= 64 << 20, "kernel snapshot bounds")
    exe = Path(shutil.which(a.qemu) or a.qemu).resolve(strict=True)
    cmd = command(str(exe), root, not a.trace)
    need(0 < a.timeout <= 300, "capture budget")
    save(root / "launch.json", dict(argv=cmd, qemu_sha256=sha(exe), case=a.case,
                                   trace=a.trace, ram_base=base, ram_size=size, timeout=a.timeout))
    marker = "incomplete"
    with (root / "stderr").open("xb") as err:
        proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, bufsize=0)
        timer = threading.Timer(a.timeout, proc.kill)
        timer.start()
        try:
            need("QMP" in json.loads(proc.stdout.readline()), "QMP greeting")
            class Pipe:
                readline = proc.stdout.readline
                write = proc.stdin.write
            request(Pipe, "qmp_capabilities")
            uart = root / "uart.bin"
            while proc.poll() is None:
                if uart.exists():
                    need(uart.stat().st_size <= 1 << 20, "UART limit")
                    raw = uart.read_bytes()
                    if re.search(rb"(?m)^NATIVE AMC END (?:[0-9]+ ){2}[0-9]+\r?\n", raw):
                        request(Pipe, "stop")
                        request(Pipe, "pmemsave", val=base, size=size, filename=str(root / "ram.bin"))
                        marker = "end"
                        break
                    if b"NATIVE EL0 FAULT " in raw:
                        marker = "fault"
                        break
                time.sleep(.005)
        finally:
            timer.cancel()
            if proc.poll() is None:
                proc.kill()
            proc.wait()
            timer.join()
            save(root / "session.json", dict(marker=marker, hashes={p.name: sha(p) for p in root.iterdir() if p.is_file()}))
            print(sha(root / "session.json"))
    return int(marker != "end")


def product(blob, slot, gen, mode, amount, kind):
    version, sid, g, active = struct.unpack_from("<4I", blob)
    if not any(blob):
        need(kind == 5, "empty product on running/completed generation")
        return dict(count=0, sum=0, stamps=[], fresh=False)
    need((version, sid, g) == (1, slot, gen) and active < 2, "private product identity")
    n, total = struct.unpack_from("<QQ", blob, 16 + 16 * active)
    ring = [RING[(slot + gen + j) % 8] for j in range(16)]
    need(total == (36 * (n // 8) + sum(ring[:n % 8])) & MAX, "private ring sum")
    values = struct.unpack_from("<16I", blob, 48)
    ck, fn, cs, fs, ns, overflow = struct.unpack_from("<IIQQII", blob, 112)
    need(ns <= 32 and overflow in (0, 1), "stamp header")
    stamps = list(struct.iter_unpack("<3Q", blob[144:144 + 24 * ns]))
    final = struct.unpack_from("<3Q", blob, 912)
    need(all(all(x <= y for x, y in zip(a, b)) for a, b in zip(stamps, stamps[1:])), "task clock or progress reversed")
    if kind == 3:
        need(final[0] and final[2] == n and (not stamps or all(x <= y for x,y in zip(stamps[-1],final))), "final task publication")
        if mode != "finite":
            need(final[1] >= amount * 1000000 and n and n % 256 == 0, "stream CPU target")
        if mode != "stream":
            required = amount if mode == "finite" else 16
            need(fn == required and list(values[:fn]) == ring[:fn] and fs == sum(ring[:fn]), "finite output")
            if mode == "finite":
                need(n == fn, "finite publication count")
            else:
                need((ck, cs) == (8, 36), "survivor checkpoint")
    return dict(count=n, sum=total, finite_count=fn, checkpoint_count=ck, stamps=stamps, final=final,
                stamp_loss=overflow, fresh=True)


def parse_uart(raw):
    records = defaultdict(list)
    for line in raw.splitlines(keepends=True):
        if line.startswith(b"NATIVE AMC "):
            need(re.fullmatch(rb"NATIVE AMC [A-Z]+(?: [0-9]+)+\r?\n", line), "incomplete UART record")
            _, _, tag, *fields = line.split()
            records[tag.decode()].append(tuple(map(int, fields)))
    def one(tag):
        need(len(records[tag]) == 1, "missing/duplicate " + tag)
        return records[tag][0]
    return records, one


def overlap_control(life, epoch, q):
    """Match admitted LO0, its latched miss, refused next attempt and real exit."""
    low = [event for event in life if event[2] == 0]
    peer = [event for event in life if event[2] == 1]
    if ([e[0] for e in low] != [1, 7, 2, 3] or
            [e[0] for e in peer] != [1, 3] or len(life) != 6 or
            any(e[4] != 0 for e in life)):
        return False
    admitted, miss, refused, exited = low
    due = epoch + q
    return (admitted[3] == 0 and admitted[5:] == (epoch, due, 0, 0, 0)
            and epoch <= admitted[1] < due <= miss[1] == refused[1] < exited[1]
            and miss[3] == 0 and miss[5] == due and miss[7:] == (0, 0, 0)
            and refused[3] == 1 and refused[5:] == (due, due + q, 1, 1, 0)
            and exited[3] == 0 and 0 <= miss[6] < exited[5] <= q
            and exited[6:] == (due, 0, MAX, 936)
            and peer[0][3] == peer[1][3] == 0
            and peer[0][5:] == (epoch, epoch + 3*q, 0, 0, 0)
            and epoch <= peer[0][1] <= peer[1][1] <= epoch + 3*q
            and 0 < peer[1][5] <= 2*q)


def score(a):
    root = a.root
    need(sha(root / "session.json") == a.digest, "session identity")
    session = json.loads((root / "session.json").read_bytes())
    hashes = session["hashes"]
    need(set(hashes) == {p.name for p in root.iterdir() if p.is_file()} - {"session.json"}, "unsealed files")
    for name, h in hashes.items():
        need(Path(name).name == name and sha(root / name) == h, "capture digest " + name)
    launch = json.loads((root / "launch.json").read_bytes())
    case = launch["case"]
    need(case in CASES, "named case")
    m = package(root, case)
    if session["marker"] != "end":
        print(json.dumps(dict(status="incomplete", marker=session["marker"], session=a.digest)))
        return 2
    raw_uart = (root / "uart.bin").read_bytes()
    image_lines = [line for line in raw_uart.splitlines() if re.match(rb"^NATIVE EL0 (?:PEER[23]? )?ELF ", line)]
    labels = ("", "PEER ", "PEER2 ", "PEER3 ")
    need(image_lines == [f"NATIVE EL0 {labels[i]}ELF {s['elf_sha256']}".encode("ascii")
                         for i,s in enumerate(m["slots"])], "ordered UART image identities")
    expected = [s["expected_ttbr0_el1"] for s in m["slots"]]
    records, one = parse_uart(raw_uart)
    ver, epoch, hz, q, count, idle, term, origin = one("CONFIG")
    tasks = m["amc"]["tasks"]
    need((ver,hz,q,count,idle,term) == (1,62500000,3125000,len(tasks),len(tasks),len(tasks)+1), "deployment clock/shape")
    mem = Snapshot(launch["ram_base"], (root / "ram.bin").read_bytes())
    kernel = elf_file((root / "kernel.elf").read_bytes())
    need(len(mem.raw) == launch["ram_size"] and (mem.base,mem.base+len(mem.raw)) ==
         (symbol(kernel,"_start")[0],symbol(kernel,"__stack_top")[0]), "snapshot extent")
    addr, rows, width, lost = one("INTERVALS")
    pa, generations, stride = one("RESULTS")
    need((addr, 8192*23*8) == symbol(kernel,"native_amc_intervals") and width == 23 and rows <= 8192, "interval ELF extent")
    need((pa,count*16*1024) == symbol(kernel,"native_amc_results") and (generations,stride)==(16,1024), "result ELF extent")
    intervals = list(struct.iter_unpack("<23Q",mem.read(addr,rows*width*8)))
    life = records["LIFE"]
    logs, missing, landing = one("END")
    need(len(life)==logs and all(len(e)==10 for e in life) and len(records["TASK"])==count,"lifecycle framing")
    issues, unknown = [], []
    if lost or missing:
        unknown.append("bounded trace loss")
    def check(ok, message):
        if not ok:
            issues.append(message)
    vas = []
    for i, (t, fields) in enumerate(zip(tasks,records["TASK"])):
        va, n = symbol(elf_file((root / f"slot{i}.elf").read_bytes()),"amc_product")
        need(fields == (i,int(t["tier"]=="HI"),*(t[k] for k in ("priority","c_lo","c_hi","deadline","period","offset","generations")),va,n) and n==936,"runtime task/ELF binding")
        vas.append(va)
    frame = {}
    charge = Counter()
    roots = {}
    for i,r in enumerate(intervals):
        cause, op, entry, start, src, gen, delta, used = r[:8]
        need(cause in (1,2,3) and src <= idle and r[13]<=term, "owner/cause")
        if i:
            need(intervals[i-1][13]==src, "decision owner discontinuity")
        if src < count:
            need(r[12] == expected[src], "actual source TTBR0_EL1 differs from linked slot identity")
            frame[src,gen]=r[10:13]
            if i and intervals[i-1][21] and cause != 3:
                need(intervals[i-1][21]==r[12],"prepared versus actual root")
        dst,dg=r[13:15]
        if dst<count:
            if r[21]:
                need(r[21] == expected[dst], "prepared destination TTBR0_EL1 differs from linked slot identity")
            if (dst,dg) in frame:
                need(r[15:17]==frame[dst,dg][:2],"restored PC/SP")
            else:
                app=elf_file((root/f"slot{dst}.elf").read_bytes())
                need(r[15]==app["e_entry"],"pristine ELF entry")
            need(r[18]>0,"positive destination allowance")
        if src < count and cause != 3:
            need(start and entry>=start and delta==entry-start and (not i or start>=intervals[i-1][2]),"outgoing interval")
            charge[src,gen] += delta
            need(charge[src,gen]==used,"cumulative service")
            if src in roots:
                need(roots[src]==r[12],"changed private root")
            roots[src]=r[12]
        else:
            need(delta==0,"charge without user execution")
    need(len({r & 0x0000fffffffff000 for r in roots.values()})==len(roots),"aliased roots")
    products, exit_at, accepted = {}, {}, {}
    for e in life:
        kind, stamp, slot, gen, hi, *v = e
        if kind in (1,2):
            t=tasks[slot]
            need(v[0]==epoch+(t["offset"]+v[2]*t["period"])*q and v[1]==v[0]+t["deadline"]*q,"original release/deadline")
            if kind==1:
                accepted[slot,gen]=(v[2],v[0],v[1])
        if kind not in (3,4,5):
            continue
        need(slot<count and gen<16 and v[4]==936,"terminal product extent")
        if (slot,gen) not in accepted and missing:
            continue
        attempt,release,due=accepted[slot,gen]
        need(v[1]==due,"terminal deadline")
        args=tasks[slot]["argv"][attempt]
        data=mem.read(pa+(slot*16+gen)*stride,936)
        p=product(data,slot,gen,args[1],int(args[2]),kind)
        p.update(kind=kind, raw_sha256=digest(data), release=release, due=due, terminal=stamp, service=v[0])
        products[slot,gen]=p
        if not lost:
            check(charge[slot,gen]==v[0],"terminal charge")
        if not lost and p.get("fresh"):
            recorded=p["stamps"] + ([p["final"]] if kind==3 else [])
            calls=[r for r in intervals if r[0:2]==(2,6) and r[4:6]==(slot,gen) and r[20]==0]
            # A task samples CPU then MONOTONIC. Pair adjacent owner clock calls,
            # permitting arbitrary preemption or IRQs in between.
            samples={((b[2]-origin)*1000000000//hz,a[7]*1000000000//hz)
                     for a,b in zip(calls,calls[1:]) if a[19]==2 and b[19]==1}
            check(all((wall,cpu) in samples for wall,cpu,n in recorded),"task clock product/syscall mismatch")
        if kind==3:
            exit_at[slot,gen]=stamp
            check(v[2] == slot, "signed exit slot " + str(slot))
            if case == "overlap" and (slot, gen) == (0, 0):
                check(stamp > due, "overlap control requires actual overdue LO0 exit")
                p["completion_lateness_ticks"] = stamp - due
                p["completion_lateness_ns"] = (stamp - due) * 1000000000 // hz
            else:
                check(stamp <= due, "completion after deadline slot " + str(slot))
            if not lost:
                check(any(r[0]==2 and r[1]==1 and r[2]==stamp and r[4:6]==(slot,gen) and r[19]==slot for r in intervals),"actual exit syscall")
        if slot in roots and gen==max((e[3] for e in life if e[0]==1 and e[2]==slot),default=-1):
            check(vread(mem,m["slots"][slot]["expected_ttbr0_el1"],vas[slot],936)==data,"last generation private RAM")
    boundaries=lambda k:epoch+k*q
    switches=[e for e in life if e[0]==9]
    crossing=None
    carry=[]
    if case in ("normal-hi","over-hi","staggered"):
        b=boundaries(5 if case=="staggered" else 2)
        check(len(switches)==1,"one HI transition")
        if switches:
            c=switches[0][1]
            transition=life.index(switches[0])
            before=life[:transition]
            active={(e[2],e[3]) for e in before if e[0]==1}-{(e[2],e[3]) for e in before if e[0] in (3,4,5)}
            expected_live={(0,0),(1,0),(3,0)} if case=="staggered" else {(i,0) for i in range(4)}
            check(active==expected_live,"live carry-in at transition")
            for slot,g in sorted(active):
                seq=[r for r in intervals if r[4:6]==(slot,g) and r[2]<=c]
                used=seq[-1][7] if seq else 0
                carry.append(dict(slot=slot,generation=g,used=used,remaining_lo=max(0,tasks[slot]["c_lo"]*q-used),remaining_hi=max(0,tasks[slot]["c_hi"]*q-used)))
            if case=="staggered":
                check(any(v["slot"]==3 and 0<v["used"]<2*q for v in carry),"partially served HI3")
            for e in life:
                if e[0]==5:
                    check(not any(r[13:15]==e[2:4] and r[2]>=c for r in intervals),"return to discarded generation")
            irq=[r for r in intervals if r[0]==1 and r[1]&1023==30 and r[4:6]==(1,0) and b<=r[2]<=c]
            stamps=products.get((1,0),{}).get("stamps",[])
            witnesses=[r for r in intervals if r[0:2]==(2,6) and r[4:6]==(1,0) and r[19:21]==(1,0) and
                       r[2]>=c and any((r[2]-origin)*1000000000//hz==s[0] for s in stamps)]
            if not irq or not witnesses:
                unknown.append("crossing PPI/task-owned clock witness")
            else:
                x=witnesses[0][2]
                crossing=dict(b=b,irq=irq[-1][2],compare=irq[-1][9],commit=c,observed=x,latency_ticks=x-b)
                check(b<=irq[-1][2]<=c<=x<b+q,"crossing timing window")
        discarded={e[2] for e in life if e[0]==5}
        check(discarded==({0} if case=="staggered" else {0,2}),"discard set")
        check(products.get((0,0),{}).get("count",0)>0,"partially served LO0")
        if case=="staggered":
            check(exit_at.get((2,0),MAX)<boundaries(2) and (1,0) in exit_at and exit_at.get((3,0),MAX)<boundaries(8),"survivor before b8")
            wfi=symbol(kernel,"native_sleep_idle")[0]
            check(any(r[10] in (wfi,wfi+4) and r[0]==1 and r[4]==idle and r[1]&1023==30 and r[9]==boundaries(2) and r[13]==0 for r in intervals),"idle PPI30 b2 release")
        else:
            check(any(e[0]==6 and e[5:8]==(boundaries(5),1,1) and any(v[0]==1 and v[2:4]==(0,1) for v in life[index+1:]) for index,e in enumerate(life)),"approved reset b5")
            check(exit_at.get((0,1),MAX)<boundaries(6) and exit_at.get((3,0),MAX)<boundaries(6),"fresh LO and survivor before b6")
            if case=="normal-hi":
                check(exit_at.get((1,0),MAX)<boundaries(5) and 2*q<charge[1,0]<=3*q,"HI third entitlement completion")
            else:
                p=products.get((1,0),{})
                check(p.get("kind")==4 and p.get("count",0)>0 and p.get("service",0)>=3*q,"HI exhaustion")
    elif case=="within-lo":
        check(not switches and set(exit_at)=={(0,0),(1,0)} and exit_at[(1,0)]<boundaries(2) and charge[1,0]<=q,"within LO")
    elif case=="refusal":
        check([(e[5],e[6],e[7]) for e in life if e[0]==6]==[(boundaries(2),1,0),(boundaries(3),0,0),(boundaries(4),1,1)],"reset refusals")
        check(any(e[0]==2 and e[2]==0 and e[5]==boundaries(2) and e[8]==2 for e in life),"LO refused in HI")
    else:
        check(overlap_control(life, epoch, q), "overlap admission, single b1 miss, refusal and continuing service to exit")
        if not lost:
            misses = [e for e in life if e[0] == 7 and e[2:4] == (0, 0)]
            if len(misses) == 1:
                at = misses[0][1]
                check(sum(r[6] for r in intervals if r[4:6] == (0, 0) and r[2] <= at)
                      == misses[0][6], "LO0 service at recorded deadline fault")
    if launch["trace"]:
        raw=(root/"exceptions.log").read_bytes()
        entries=re.findall(rb"Taking exception (?:2 \[SVC\]|5 \[IRQ\])[^\n]*\r?\n\.\.\.from EL0 to EL1\r?\n(?:\.\.\.with ESR [^\n]+\n)?\.\.\.with ELR (0x[0-9a-fA-F]+)",raw)
        observed=[int(s,16) for s in entries]
        expected=[r[10] for r in intervals if r[0] in (1,2) and r[4]<count]
        if not lost:
            check(observed==expected and bool(expected),"external EL0 exception PC sequence")
    if not missing and not lost:
        for i,t in enumerate(tasks):
            admissions=[e for e in life if e[0] in (1,2) and e[2]==i]
            check([e[7] for e in admissions]==list(range(t["generations"])),"release attempt coverage")
        check(set(products)==set(accepted),"all admitted generations retired")
    for p in products.values():
        p["release_to_terminal_ticks"]=p["terminal"]-p["release"]
        p["release_to_terminal_ns"]=p["release_to_terminal_ticks"]*1000000000//hz
        if p.get("stamp_loss"):
            unknown.append("task stamp loss")
    report=dict(case=case,session=a.digest,raw_deployment_sha256=m["amc_input"]["sha256"],
                canonical_sha256=m["amc_canonical"]["sha256"],rows=rows,lost=lost,life_lost=missing,
                idle_landing=landing,expected_roots=[s["expected_ttbr0_el1"] for s in m["slots"]],roots=roots,crossing=crossing,carry=carry,products={f"{i}.{g}":p for (i,g),p in products.items()},
                lifecycle=life,issues=issues,unknown=unknown)
    report["status"]="incomplete" if (lost or missing) else "failed" if issues else "incomplete" if unknown else "observations-met"
    print(json.dumps(report,indent=2))
    return 0 if report["status"]=="observations-met" else 2


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest="action",required=True)
    c=sub.add_parser("capture")
    for name in ("package","input","out"):
        c.add_argument(name,type=Path)
    c.add_argument("case",choices=CASES)
    c.add_argument("--trace",action="store_true")
    c.add_argument("--qemu",default="qemu-system-aarch64")
    c.add_argument("--timeout",type=int,default=30)
    r=sub.add_parser("report")
    r.add_argument("root",type=Path)
    r.add_argument("digest")
    args=parser.parse_args()
    try:
        sys.exit(capture(args) if args.action=="capture" else score(args))
    except (ValueError,KeyError,IndexError,OSError,struct.error) as exc:
        sys.exit("AMC evidence error: "+str(exc))
