#!/usr/bin/env python3
"""Seal, capture and replay the six frozen native AMC clock-pair positions."""
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys

from native_amc_report import FIXTURES, MAX, capture, score
from native_el0_capture import command
from native_el0_replay import ESR, PA_MASK, SVC, elf_file, permissions, symbol, vread
from ordinary_amc_capture import save, sha

VA = 0x01400000
NANO = 1_000_000_000
POSITIONS = (("normal-hi", True, False), ("normal-hi", True, True),
             ("staggered", True, False), ("staggered", True, True),
             ("normal-hi", False, True), ("control", True, False))
CONTROL = FIXTURES.parents[1] / "native-amc-clock" / "control.json"


def need(ok, reason):
    if not ok:
        raise ValueError(reason)


def fixture(case):
    return CONTROL if case == "control" else FIXTURES / (case+".json")


class ClockEvidence:
    def __init__(self, case, enabled, trace):
        self.case, self.enabled, self.trace = case, enabled, trace
        self.fixture = fixture(case)
        self.issues, self.unknown = [], []
        self.grants = defaultdict(list)
        self.calls = defaultdict(list)
        self.matches = defaultdict(list)
        self.report = {}

    def check(self, ok, message):
        if not ok and message not in self.issues:
            self.issues.append(message)
        return ok

    def load(self, root, launch, manifest, records, mem, kernel, intervals, hz, origin):
        self.mem, self.kernel, self.records, self.root = mem, kernel, records, root
        self.intervals = intervals
        self.hz, self.origin = hz, origin
        self.roots = [s["expected_ttbr0_el1"] for s in manifest["slots"]]
        self.count = len(self.roots)
        need((launch["case"],launch["trace"],launch["timeout"]) == (self.case,self.trace,120),
             "wrong frozen capture position or timeout")
        self.manifest = manifest
        table = kernel.get_section_by_name(".symtab")
        pages = table.get_symbol_by_name("jnox_native_amc_clock_pages") or []
        need(bool(pages) == self.enabled, "clock-page ELF/selector mismatch")
        cols = []
        for slot, ttbr in enumerate(self.roots):
            if not self.enabled:
                need(mem.leaf(ttbr,VA) is None, "disabled page is mapped")
                continue
            base, size = symbol(kernel,"jnox_native_amc_clock_pages")
            pa = base + slot*4096
            need(size == self.count*4096 and pa%4096 == 0, "private page extent")
            permissions(mem, ttbr, VA, "user r")
            need(mem.leaf(ttbr,VA) & PA_MASK == pa, "slot page identity")
            for peer_root in self.roots:
                permissions(mem, peer_root, pa, "kernel rw")
            page = mem.read(pa,4096)
            payload = struct.unpack_from("<8Q",page)
            self.check(payload[0] == 0 and not any(page[64:]), "retirement revocation or reserved bytes")
            cols.append(dict(slot=slot,root=ttbr,pa=pa,payload=payload,pte=mem.leaf(ttbr,VA)))
        self.report["pages"] = cols
        rows = []
        if self.enabled:
            need(len(records["CLOCKS"]) == 1, "missing clock publication descriptor")
            ver, va, address, count, width, lost, cntkctl = records["CLOCKS"][0]
            need(ver == 1 and va == VA and width == 4 and count == len(intervals), "clock history ABI")
            self.check(lost == 0, "clock publication loss")
            need(symbol(kernel,"native_amc_clock_history") == (address,8192*32), "linked clock history extent")
            self.check(cntkctl == 1, "physical-counter access / timer permissions")
            self.report["cntkctl_el1"] = cntkctl
            rows = list(struct.iter_unpack("<4Q", mem.read(address, count*32)))
        else:
            need(not records["CLOCKS"], "unexpected enabled clock history")
        service = Counter()
        sequence = {}
        for i,r in enumerate(intervals):
            key = tuple(r[4:6])
            if r[4] < self.count and r[0] != 3:
                service[key] += r[6]
                if r[0] == 2:
                    self.calls[key].append(r)
                    need(int.from_bytes(vread(mem,r[12],r[10]-4,4),"little") == SVC,
                         "recorded lower-EL syscall instruction")
            if not rows:
                continue
            seq,R,settled,generation = rows[i]
            next_row = intervals[i+1] if i+1 < len(intervals) else None
            if r[13] >= self.count or not next_row or next_row[0] == 3:
                self.check(seq == R == 0, "idle/terminal/rejected gate published clock")
                continue
            dest = tuple(r[13:15])
            self.check((seq>0 and not seq%2 and seq > sequence.get(dest,0) and R == next_row[3]
                        and r[2] <= R <= next_row[2] and dest == tuple(next_row[4:6]) and
                        generation == dest[1] and service[dest] == settled and
                        next_row[7] == settled+next_row[6] and
                        next_row[9] == min(R+r[18],r[17])), "published R / charge / CVAL mismatch")
            sequence[dest] = seq
            self.grants[dest].append((R,next_row[2],settled,seq))
        self.report["accepted_publications"] = sum(map(len,self.grants.values()))
        if self.trace:
            raw = (root/"exceptions.log").read_text("ascii")
            observed = [(int(code),int(pc,16)) for code,pc in re.findall(
                r"Taking exception (2|5) \[(?:SVC|IRQ)\][^\n]*\n\.\.\.from EL0 to EL1\n"
                r"\.\.\.with ESR [^\n]+\n\.\.\.with ELR (0x[\da-fA-F]+)",raw)]
            expected = [(2 if r[0] == 2 else 5,r[10]) for r in intervals if r[0] in (1,2) and r[4]<self.count]
            self.check(observed == expected, "external exception kind/ELR stream")
            self.report["external_el0_entries"] = len(observed)
        else:
            self.report["external_el0_entries"] = None

    def pair(self,key,wall,cpu):
        # F=62.5 MHz gives exactly 16 ns per tick. Derive g from the user
        # monotonic word, then check CPU independently on the same grant.
        ticks, rem = divmod(wall*self.hz,NANO)
        g = self.origin+ticks
        if rem or g > MAX:
            return None
        for R,stop,base,seq in self.grants[key]:
            if R <= g <= stop and (base+g-R)*NANO//self.hz == cpu:
                return g
        return None

    def samples(self,slot,gen,recorded,syscall_pairs):
        key = slot,gen
        for wall,cpu,n in recorded:
            if self.enabled:
                g = self.pair(key,wall,cpu)
                if self.check(g is not None,"private stamp does not match a published owner interval"):
                    self.matches[key].append((g,n))
            elif self.check((wall,cpu) in syscall_pairs,"fallback stamp not bound to syscall pair"):
                self.matches[key].append((self.origin+wall*self.hz//NANO,n))

    def crossing(self,b,commit,q,intervals,stamps):
        change, = [e for e in self.records["LIFE"] if e[0] == 9]
        candidates = [r for r in intervals if tuple(r[4:6]) == (1,0) and r[2] <= commit
                      and r[7] == change[7] and r[0] in (1,2)]
        if not self.check(bool(candidates),"missing HI1 budget crossing"):
            return None
        row = candidates[-1]
        irq = row[0] == 1 and row[1]&1023 == 30
        if self.enabled:
            self.check(irq,"HI1 transition did not enter through PPI30")
        owned = [g for g,n in self.matches[1,0] if g >= commit and n>0]
        x = min(owned,default=None)
        self.check(x is not None and b <= row[2] <= commit <= x < b+q, "crossing/commit/task deadline")
        return dict(boundary=b,compare=row[9],cause=row[0],iar_or_syscall=row[1],ppi30=irq,
                    i=row[2],c=commit,x=x,root=row[12],owner_charge=row[6],used=row[7],spill=row[8])

    def control(self,life,products,intervals,exits):
        self.check(set(exits)=={(0,0),(1,0),(0,1)} and not any(e[0] in (2,4,5,7,9) for e in life),
                   "control completion, deadlines or generation identity")
        low, high = (0, 0), (1, 0)
        admissions = [e for e in life if e[0] == 1 and tuple(e[2:4]) == high]
        starts = [i for i,r in enumerate(intervals)
                  if tuple(r[4:6]) == low and tuple(r[13:15]) == high]
        returns = [i for i,r in enumerate(intervals)
                   if tuple(r[4:6]) == high and tuple(r[13:15]) == low]
        if self.check(len(admissions) == len(starts) == len(returns) == 1 and
                      0 < starts[0] < returns[0] < len(intervals)-1,
                      "missing unique control admission, preemption or restoration"):
            i, j = starts[0], returns[0]
            prev, handoff, restore = intervals[i-1], intervals[i], intervals[j]
            admission = admissions[0]
            release = products[high]["release"]
            high_grants = self.grants[high]
            self.check(high_grants and admission[5] == release and
                       max(release,handoff[2]) <= admission[1] <= high_grants[0][0],
                       "high admission outside release/dispatch interval")
            cause, op = handoff[:2]
            if cause == 3:
                valid = (op == 0 and handoff[3] == handoff[6] == 0 and
                         prev[17] == handoff[9] == release <= handoff[2] and
                         tuple(prev[13:15]) == low and prev[15:17] == handoff[10:12])
            elif cause == 1:
                valid = op & 1023 == 30 and handoff[9] == release <= handoff[2]
            else:
                valid = (cause == 2 and op == 6 and handoff[19] in (1,2) and
                         handoff[20] == 0 and handoff[9] == release)
            self.check(valid, "unsubstantiated control handoff cause")
            before = sum(r[6] for r in intervals[:i+1] if tuple(r[4:6]) == low)
            after = sum(r[6] for r in intervals[j+1:] if tuple(r[4:6]) == low)
            middle = intervals[i+1:j+1]
            self.check(before == handoff[7] > 0 and after > 0 and
                       before + after == products[low]["service"] and
                       all(tuple(r[4:6]) == high for r in middle) and
                       sum(r[6] for r in middle) == products[high]["service"] > 0,
                       "control owner service before, during or after preemption")
            self.check(restore[:2] == (2,1) and restore[19] == high[0] and
                       restore[2] == exits[high] and restore[15:17] == handoff[10:12] and
                       restore[21] == handoff[12] == self.roots[low[0]] and
                       any(R >= restore[2] and base == before and stop > R
                           for R,stop,base,_ in self.grants[low]),
                       "control exit did not restore charged original low frame")
            self.report["control_handoff"] = dict(row=i, cause=cause, op=op, release=release,
                                                  admitted=admission[1], restored_row=j,
                                                  low_before=before, low_after=after)
        self.check(products.get((0,1),{}).get("finite_count")==8, "clean eight-reading generation")

    def finish(self,products,life):
        manifest_clock = self.manifest.get("amc_clock_page")
        need(manifest_clock == dict(enabled=self.enabled,abi=1,va=VA), "manifest clock ABI")
        need(len(self.records["CLOCK_EVIDENCE"]) == 1, "missing generation evidence index")
        ver,index,archive,length,dropped = self.records["CLOCK_EVIDENCE"][0]
        need(ver == 1 and length <= 1 << 20, "retirement archive format")
        self.check(dropped == 0, "generation evidence archive overflow")
        table = self.kernel.get_section_by_name(".symtab")
        for name,addr,size in (("CLOCK_INDEX",index,self.count*16*32),("CLOCK_ARCHIVE",archive,1 << 20)):
            hits = [s for s in table.iter_symbols() if name in s.name and s["st_info"]["type"]=="STT_OBJECT"]
            need(len(hits)==1 and (hits[0]["st_value"],hits[0]["st_size"])==(addr,size), "linked "+name)
        spans = []
        reports = {}
        for (slot,gen),product in products.items():
            app = elf_file((self.root/f"slot{slot}.elf").read_bytes())
            addr,size = symbol(app,"amc_clock_evidence")
            va,n,off,stored = struct.unpack("<4Q",self.mem.read(index+(slot*16+gen)*32,32))
            need((va,n)==(addr,size) and n==(73760 if self.case=="control" else 32),
                 "linked client clock evidence format")
            if stored != n or off+stored>length:
                self.unknown.append(f"missing archived clock evidence {slot}.{gen}")
                continue
            spans.append((off,off+n))
            data = self.mem.read(archive+off,n)
            errors,fast,slow,count,overflow = struct.unpack_from("<3Q2I",data)
            calls = self.calls[slot,gen]
            discovery = [r for r in calls if r[1]==20]
            clocks = [r for r in calls if r[1]==6]
            # A discarded generation can be admitted without ever obtaining a grant.
            ran = bool(product["service"])
            if ran and (fast+slow or product["kind"]==3):
                self.check(len(discovery)==1 and discovery[0][20] in
                           ((VA,) if self.enabled else ((-38)&MAX,(-95)&MAX)),
                           "generation-private discovery mismatch")
            self.check(errors==0 and not overflow, "helper error or bracket loss")
            if self.enabled:
                self.check(slow==0 and (fast>0 or product["kind"]!=3), "page-backed generation samples")
                if self.case!="control":
                    self.check(not clocks, "steady-state clock SVC in page cohort")
            else:
                self.check(fast==0 and (slow>0 or product["kind"]!=3), "syscall-backed generation samples")
                # A discarded transaction may have started a last pair before its
                # userspace count commits. All observed calls must be its prefix.
                ids = [r[19] for r in clocks]
                self.check(ids==([2,1]*((len(ids)+1)//2))[:len(ids)] and
                           2*slow<=len(ids)<=2*slow+(0 if product["kind"]==3 else 2) and
                           all(r[20]==0 for r in clocks), "fallback CPU/monotonic calls")
            details = dict(error=errors,page=fast,syscall=slow,discovery=len(discovery),clock_svcs=len(clocks),
                           deadline_margin_ticks=product["due"]-product["terminal"], brackets=[])
            if self.case=="control":
                self.check(2<=count<=1024 and count<=fast and len(clocks)==4*count,
                           "missing start/completion or unmatched control bracket calls")
                previous = 0
                for j in range(min(count,1024)):
                    g,n,c0,m0,c,m,m1,c1,backend = struct.unpack_from("<9Q",data,32+72*j)
                    self.check(g==gen and n>=previous and backend==1 and c0<=c<=c1 and m0<=m<=m1
                               and self.pair((slot,gen),m,c) is not None,
                               "bracket owner, bounds or published sample mismatch")
                    pair = clocks[4*j:4*j+4]
                    self.check([r[19] for r in pair]==[2,1,1,2] and all(r[20]==0 for r in pair)
                               and [((r[7] if r[19]==2 else r[2]-self.origin)*NANO//self.hz)
                                    for r in pair]==[c0,m0,m1,c1],"bracket syscall operands")
                    details["brackets"].append(dict(count=n,cpu_width=c1-c0,monotonic_width=m1-m0,
                                                    pair_ticks=self.pair((slot,gen),m,c)))
                    previous = n
                if (slot,gen)==(0,0):
                    peer = [(r[3],r[2]) for r in self.intervals if r[4:6]==(1,0) and r[6]>0]
                    sample_ticks = [b["pair_ticks"] for b in details["brackets"] if b["pair_ticks"] is not None]
                    self.check(peer and sample_ticks and min(sample_ticks)<min(a for a,b in peer)
                               and max(sample_ticks)>max(b for a,b in peer),
                               "control low task lacks bracket samples on both sides of peer execution")
                if (slot,gen)==(0,1):
                    grants=self.grants[slot,gen]
                    self.check(grants and grants[0][2]==0 and grants[0][3]==2,
                               "fresh generation accumulated service or reused publication")
                self.check(count and details["brackets"][0]["count"]==0 and
                           previous==product["count"], "control start/final ring evidence")
            else:
                self.check(count==0,"unexpected work bracket records")
            reports[f"{slot}.{gen}"] = details
        ordered = sorted(spans)
        need(ordered and ordered[0][0]==0 and ordered[-1][1]==length and
             all(a[1]==b[0] for a,b in zip(ordered,ordered[1:])), "archive gaps or aliases")
        if self.enabled:
            for page in self.report["pages"]:
                slot = page["slot"]
                keys = [key for key in self.grants if key[0]==slot and self.grants[key]]
                if keys:
                    key = max(keys)
                    R,_,base,_ = self.grants[key][-1]
                    self.check(tuple(page["payload"][1:])==(1,slot,key[1],self.hz,self.origin,base,R),
                               "last revoked page payload vs actual final grant")
        self.report["generation_evidence"] = reports
        return self.report


def read_seal(path, expected):
    need(sha(path)==expected, "cohort seal hash mismatch")
    return json.loads(path.read_bytes())


def seal(args):
    qemu = Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True)
    args.out.mkdir()
    rows = []
    quartet = None
    for ordinal,((case,enabled,trace),pkg) in enumerate(zip(POSITIONS,args.packages),1):
        pkg = pkg.resolve(strict=True)
        m = json.loads((pkg/"manifest.json").read_bytes())
        need(m["policy"]=="amc" and m["amc_clock_page"]==dict(enabled=enabled,abi=1,va=VA),
             "cohort package selector mismatch")
        need(sha(fixture(case))==m["amc_input"]["sha256"],"cohort descriptor mismatch")
        digests = []
        disassemblies = {}
        for i,s in enumerate(m["slots"]):
            elf = pkg/s["elf"]
            need(sha(elf)==s["elf_sha256"],"slot ELF changed")
            digests.append(s["elf_sha256"])
            name=f"{ordinal}-slot{i}.dis"
            with (args.out/name).open("xb") as stream:
                subprocess.run([args.objdump,"-d",elf],stdout=stream,check=True)
            disassemblies[name]=sha(args.out/name)
        if ordinal<6:
            need(len(digests)==4 and (quartet is None or quartet==digests),"quartet changed")
            quartet=digests
        need(sha(pkg/"kernel.elf")==m["kernel_elf_sha256"],"kernel changed")
        name=f"{ordinal}-kernel.dis"
        with (args.out/name).open("xb") as stream:
            subprocess.run([args.objdump,"-d",pkg/"kernel.elf"],stdout=stream,check=True)
        disassemblies[name]=sha(args.out/name)
        rows.append(dict(ordinal=ordinal,case=case,enabled=enabled,trace=trace,package=str(pkg),
                         manifest=sha(pkg/"manifest.json"),kernel=m["kernel_elf_sha256"],slots=digests,
                         config=sha(fixture(case)),disassemblies=disassemblies,
                         argv=command(str(qemu),(args.out/f"first-{ordinal}").resolve(),not trace)))
    path = args.out/"cohort.json"
    save(path,dict(qemu=str(qemu),qemu_sha256=sha(qemu),timeout=120,positions=rows))
    print(sha(path))
    return 0


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    commands=parser.add_subparsers(dest="action",required=True)
    freeze=commands.add_parser("seal",help="seal six ordered package paths and disassembly before the first boot")
    freeze.add_argument("out",type=Path)
    freeze.add_argument("packages",type=Path,nargs=6)
    freeze.add_argument("--qemu",default="qemu-system-aarch64")
    freeze.add_argument("--objdump",default="aarch64-linux-gnu-objdump")
    for action in ("capture","report"):
        sub=commands.add_parser(action)
        sub.add_argument("cohort",type=Path)
        sub.add_argument("seal_sha256")
        sub.add_argument("position",type=int,choices=range(1,7))
        if action=="report":
            sub.add_argument("session_sha256")
    args=parser.parse_args()
    try:
        if args.action=="seal":
            return seal(args)
        data=read_seal(args.cohort,args.seal_sha256)
        n=args.position
        row=data["positions"][n-1]
        case,enabled,traced=POSITIONS[n-1]
        need((row["ordinal"],row["case"],row["enabled"],row["trace"])==(n,case,enabled,traced),
             "frozen acquisition order")
        directory=args.cohort.parent
        root=directory/f"first-{n}"
        for name,digest in row["disassemblies"].items():
            need(sha(directory/name)==digest,"changed disassembly")
        if args.action=="capture":
            pkg=Path(row["package"])
            need(sha(pkg/"manifest.json")==row["manifest"] and sha(Path(data["qemu"]))==data["qemu_sha256"],
                 "sealed build or emulator changed")
            need(n==1 or (directory/f"first-{n-1}"/"session.json").is_file(),"preceding first not retained")
            return capture(argparse.Namespace(out=root,package=pkg,input=fixture(case),case=case,
                qemu=data["qemu"],trace=traced,timeout=120),fixture=fixture(case))
        launch=json.loads((root/"launch.json").read_bytes())
        need(launch["qemu_sha256"]==data["qemu_sha256"] and launch["argv"]==row["argv"] and
             sha(root/"kernel.elf")==row["kernel"] and sha(root/"manifest.json")==row["manifest"],
             "capture/launch differs from sealed package")
        return score(argparse.Namespace(root=root,digest=args.session_sha256),
                     clock=ClockEvidence(case,enabled,traced))
    except (ValueError,KeyError,IndexError,OSError,struct.error,subprocess.CalledProcessError) as exc:
        print(json.dumps(dict(status="incomplete_or_mismatch",detail=str(exc))))
        return 2


if __name__=="__main__":
    sys.exit(main())
