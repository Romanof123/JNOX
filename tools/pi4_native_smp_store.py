"""One additional stock-QEMU Pi prime-pair observation.

seal P2 NEW_DIR QEMU PLUGIN HEADER BUILD_RECEIPT AVAILABILITY AVAILABILITY_SHA256
capture DIR SEAL_SHA256
report DIR SEAL_SHA256 SESSION_SHA256
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import struct
import sys
from types import SimpleNamespace

from elftools.elf.elffile import ELFFile
from native_el0_replay import elf_file, symbol
from ordinary_amc_capture import save, sha
from pi4_native_smp_abi import trace_check
from pi4_native_smp_capture import capture
from pi4_native_smp_replay import Ram, inspect, require
from pi4_native_smp_stage1 import APPS, MODEL, SESSIONS, SOURCE, get_rows

SITES=(0x40048c,0x400084)
WORDS=((0xb900012b,0x1100090b),(0xb900014b,0x17fffff0))
PADDR=(34824192,39104512)
RESULT=0x402000
ROWS=1024
SIZE=8*(8+2*(16+16*ROWS))
FIRST="first-store-traced"
FILE="observer.bin"
HEAD="attempted stored dropped store_exec memory completed aborted invalid generation pending_store pending_return discontinuities returns first_user first_loss pending_mem".split()
FIELDS="seq kind cpu gen pc aux va pa value ordinal pre_lo pre_hi post_lo post_hi counter extra".split()


def obj(path):
    return json.loads(path.read_bytes())


def original(root):
    require(sha(root/"session.json")==SESSIONS["p2"],"original pair-traced session")
    s=obj(root/"session.json")
    for name,h in s["files"].items():
        require(Path(name).name==name and sha(root/name)==h,"original artifact "+name)
    m=obj(root/"manifest.json")
    require(m["commit"]==SOURCE and m["board"]=="pi4" and m["policy"]=="smp-partitioned" and
            [(s["elf_sha256"],s["cpu_mask"],s["priority"],s["release_cell"],s["argv"]) for s in m["slots"]]==
            [(h,1<<i,2,0,["/bin/app"]) for i,h in enumerate(APPS)],"original placement and inputs")
    k=obj(root/"launch.json")
    require(k["qemu_sha256"]==MODEL and k["timeout"]==120 and k["trace"],"archived launch")
    return m,k


def file_bytes(elf,pc,size):
    seg,=[s for s in elf.iter_segments() if s["p_type"]=="PT_LOAD" and
          s["p_vaddr"]<=pc and pc+size<=s["p_vaddr"]+s["p_filesz"]]
    idx=pc-seg["p_vaddr"]
    return seg.data()[idx:idx+size]


def mappings(root,m,launch):
    ram=Ram(root,launch["ram_base"])
    k=elf_file((root/"kernel.elf").read_bytes())
    tables,=[s for s in k.get_section_by_name(".symtab").iter_symbols() if "native_el0_mmu" in s.name and "6SPACES" in s.name]
    require(tables["st_size"]>0 and tables["st_size"]%8192==0,"linked MMU extent")
    maps=[]
    for cpu,s in enumerate(m["slots"]):
        ttbr=((cpu+1)<<48)|(tables["st_value"]+cpu*(tables["st_size"]//2))
        elf=elf_file((root/s["elf"]).read_bytes())
        require(symbol(elf,"private_result")== (RESULT,4),"private result symbol")
        require(struct.unpack("<2I",file_bytes(elf,SITES[cpu],8))==WORDS[cpu],"linked store/fall-through opcode")
        code,cd=ram.walk(ttbr,SITES[cpu])
        pa,d=ram.walk(ttbr,RESULT)
        require(pa==PADDR[cpu] and (d&0x8c0)==0x840 and d>>54&1 and d>>53&1 and
                code and cd&0x8c0==0x8c0 and not cd>>54&1 and cd>>53&1,"private writable count/executable site")
        require(ram.words(code,1)[0]==int.from_bytes(file_bytes(elf,SITES[cpu],8),"little"),"loaded store/fall-through")
        maps.append(dict(ttbr=ttbr,pc=SITES[cpu],fall=SITES[cpu]+4,code_pa=code,va=RESULT,pa=pa,
                         terminal=ram.words(pa,1)[0]&0xffffffff))
    require([s["terminal"] for s in maps]==[78498,70435],"terminal independent prime counts")
    for pc in (0x2002154,0x2002170):
        require(file_bytes(k,pc,4)==bytes.fromhex("e0039fd6"),"frozen EL0 / idle ERET")
    return maps


def seal(a):
    m,k=original(a.p2)
    require(sha(a.qemu)==MODEL and sha(a.availability)==a.availability_sha,"emulator/availability identity")
    proof=obj(a.availability)
    required=("vcpu_exec","vcpu_memory","physical_address","store_u32_le","discontinuity",
              "eret_first_user","stopped_shared_output","cpu_indexed_exceptions")
    require(proof["qemu_sha256"]==MODEL and proof["api"]==7 and proof["header_sha256"]==sha(a.header)
            and proof["plugin_sha256"]==sha(a.plugin) and proof["plugin_build_sha256"]==sha(a.build_receipt)
            and proof["build_id"] and proof["build_configuration"] and
            all(proof["capabilities"].get(key) is True for key in required),"prelaunch exact-model availability receipt")
    header=a.header.read_text()
    require(re.search(r"(?m)^\s*#\s*define\s+QEMU_PLUGIN_VERSION\s+7\b",header),"plugin API 7 header")
    plugin_source=Path(__file__).with_name("pi4_native_smp_store_plugin.c")
    require(proof["plugin_source_sha256"]==sha(plugin_source),"source-built observer identity")
    build=obj(a.build_receipt)
    with a.qemu.open("rb") as handle:
        notes=ELFFile(handle).get_section_by_name(".note.gnu.build-id")
        ids=[n["n_desc"] for n in notes.iter_notes() if n["n_type"]=="NT_GNU_BUILD_ID"]
    require(ids==["f2717a7db511bde6a6b5d68032618c682ca8f56b"] and
            ids[0]==proof["build_id"]==build["qemu_build_id"] and build["exit"]==0 and
            build["qemu_sha256"]==MODEL and build["plugin_sha256"]==sha(a.plugin) and
            build["header_sha256"]==sha(a.header)=="335d4e472067e914add598b60d1f7cf0dc029cd48f62f66656d6a88c54d8d9f9"
            and build["source_sha256"]==sha(plugin_source) and build["compiler"] and build["argv"]
            and re.fullmatch("[0-9a-f]{64}",build["compiler_sha256"]),"observer build inputs / GNU build ID")
    a.out.mkdir()
    for src,name in [(a.plugin,"observer.so"),(a.header,"qemu-plugin.h"),(a.build_receipt,"plugin-build.json"),
                     (a.availability,"availability.json"),(plugin_source,"observer.c")]:
        shutil.copyfile(src,a.out/name)
    save(a.out/"seal.json",dict(version=1,source=SOURCE,p2=str(a.p2.resolve()),reference=SESSIONS["p2"],
         manifest=sha(a.p2/"manifest.json"),qemu=str(a.qemu.resolve()),qemu_sha256=MODEL,
         qemu_build_id=proof["build_id"],archived_command=k["argv"],maps=mappings(a.p2,m,k),timeout=120,
         position=FIRST,files={f.name:sha(f) for f in a.out.iterdir() if f.is_file()}))
    print(sha(a.out/"seal.json"))


class Observer:
    def __init__(self, plugin):
        self.plugin=plugin

    def arguments(self, root):
        dest=root/"observer.shared"
        require("," not in str(self.plugin) and "," not in str(dest),"QEMU plugin path contains comma")
        return ["-plugin",str(self.plugin)+",out="+str(dest)]

    def stopped(self, root):
        # QMP stop has completed; the memory snapshot is from the same stopped VM.
        # MAP_SHARED is already visible. Capture the bounded file before SIGKILL.
        live=root/"observer.shared"
        require(live.stat().st_size==SIZE,"plugin returned truncated/unavailable bounded mapping")
        with live.open("rb") as src,(root/FILE).open("xb") as dst:
            shutil.copyfileobj(src,dst)


def decode(raw):
    require(len(raw)==SIZE,"truncated or surplus observer bytes")
    hdr=struct.unpack_from("<8Q",raw)
    require(hdr[:5]==(0x3535334f425331,1,2,ROWS,16) and hdr[5]>0 and hdr[6:]==(7,1),"observer ABI/ready")
    offset=64
    all_packets=[]
    headers=[]
    for cpu in (0,1):
        h=dict(zip(HEAD,struct.unpack_from("<16Q",raw,offset)))
        headers.append(h)
        offset+=128
        require(h["stored"]<=ROWS and h["stored"]+h["dropped"]==h["attempted"],"bounded commit counts")
        packets=[]
        for index in range(ROWS):
            vals=struct.unpack_from("<16Q",raw,offset)
            offset+=128
            if index>=h["stored"]:
                require(not any(vals),"uncommitted observer tail")
                continue
            p=dict(zip(FIELDS,vals))
            require(p["seq"]==index+1 and p["cpu"]==cpu and p["kind"] in range(1,8),"packet commit/CPU/type")
            packets.append(p)
        all_packets.append(packets)
    return hdr[5],headers,all_packets


def strict_chain(runs,resolution,offset):
    # In addition to the first-user / exception markers, guard the four
    # observations by earlier and later completed private stores on BOTH cores.
    # This excludes handler time without estimating trap-to-callback latency.
    best=None
    for a in runs[0]:
        for b in runs[1]:
            if min(len(a["stores"]),len(b["stores"]))<4:
                continue
            low=max(r["stores"][0]["post_hi"]+resolution for r in (a,b))
            high=min(r["stores"][-1]["pre_lo"]-resolution for r in (a,b))
            if low>=high:
                continue
            # These are outer physical counter envelopes only. Never convert
            # the host observation order or callback latencies to guest ticks.
            start=max(a["R"],b["R"]+offset[1])
            end=min(a["E"],b["E"]+offset[0])
            if start>=end:
                continue
            for first in (0,1):
                selection=[]
                threshold=low
                for cpu in (first,1-first,first,1-first):
                    store=next((s for s in (a,b)[cpu]["stores"] if threshold<s["pre_lo"]-resolution
                                and s["post_hi"]+resolution<high),None)
                    if not store:
                        break
                    threshold=store["post_hi"]+resolution
                    selection.append(store)
                if len(selection)==4:
                    best=dict(stores=selection,host_guards_ns=[low,high],clock_resolution_ns=resolution,
                              guest_counter_common_outer_ticks=[start,end],
                              runs=[{k:v for k,v in r.items() if k!="stores"} for r in (a,b)])
                    return best
    return None
def report(a,record):
    root=a.out/FIRST
    sess=obj(root/"session.json")
    require(sha(root/"session.json")==a.session_sha and sess["reason"]=="terminal","incomplete terminal capture/session")
    require(set(sess["files"])=={p.name for p in root.iterdir() if p.is_file()}-{"session.json"},"unsealed capture members")
    for name,h in sess["files"].items():
        require(Path(name).name==name and sha(root/name)==h,"capture identity "+name)
    require(sha(root/FILE)==sha(root/"observer.shared"),"observer changed after QMP stop")
    ref=Path(record["p2"])
    m,ref_launch=original(ref)
    require(sha(root/"manifest.json")==record["manifest"] and all(sha(root/f)==sha(ref/f)
            for f in ["kernel.elf","kernel8.img",*[s["elf"] for s in m["slots"]]]),"frozen first image/ELFs")
    launch=obj(root/"launch.json")
    require(launch["trace"] and launch["timeout"]==120 and launch["qemu_sha256"]==MODEL and
            launch["version"]==ref_launch["version"],"model/clock/capture budget")
    argv=launch["argv"]
    # Relocatable captured paths; keep the original command order and options.
    dst=Path(argv[argv.index("-kernel")+1]).parent
    src=Path(ref_launch["argv"][ref_launch["argv"].index("-kernel")+1]).parent
    expected=[str(v).replace(str(src)+"/",str(dst)+"/") for v in ref_launch["argv"]]
    expected[0]=record["qemu"]
    expected+=["-plugin",str(dst.parent/"observer.so")+",out="+str(dst/"observer.shared")]
    require(argv==expected,"only the sealed observer may differ from the original QEMU command")
    maps=mappings(root,m,launch)
    require(maps==record["maps"],"terminal root/site/physical identity")
    legacy=inspect(SimpleNamespace(capture=root,session_sha256=a.session_sha,candidate_register=[8,11]))
    rows,cfg,cal=get_rows(root,Ram(root,launch["ram_base"]))
    require(cfg[:3]==(1,62500000,1250000) and cfg[4:]==(2,2) and cal[0]<=cal[2],"unchanged timer configuration")
    offset=(cal[0]-cal[1],cal[2]-cal[1])
    kernel=elf_file((root/"kernel.elf").read_bytes())
    vec=symbol(kernel,"pi4_native_smp_vectors")[0]
    require(vec==0x2001800,"frozen linked vectors")
    trace_check((root/"exceptions.log").read_text("ascii"),
                [[(p["raw"],p["saved"],p["selected"]) for p in group] for group in rows],vec)
    res,heads,records=decode((root/FILE).read_bytes())
    runs=[[],[]]
    counters=[]
    for cpu,(head,packets,group) in enumerate(zip(heads,records,rows)):
        require(head["dropped"]==head["first_loss"]==head["invalid"]==head["pending_store"]==
                head["pending_return"]==head["pending_mem"]==0,f"CPU{cpu} observer loss, invalid or pending packet: {head}")
        require(head["store_exec"]==head["memory"]==maps[cpu]["terminal"]-(1-cpu) and
                head["completed"]+head["aborted"]==head["store_exec"],"verified store/memory/terminal counts")
        expected_runs=[(i,p,group[i+1]) for i,p in enumerate(group[:-1]) if p["dst"]==cpu]
        current=None
        latest=0
        sampled=0
        events=[]
        last_value=0
        r_count=f_count=0
        idle_pending=False
        counter=0
        for p in packets:
            kind=p["kind"]
            require(latest<=p["pre_lo"]<=p["pre_hi"] and p["pre_lo"]>0,"local callback clock order")
            latest=p["pre_hi"]
            if events and events[-1][1] is None:
                require(kind==4,"architectural change missing actual vector landing")
            if idle_pending:
                require(kind==7,"missing first idle landing")
            if kind!=1:
                require(p["counter"]==counter,"packet return operand mismatch")
            if kind!=5:
                require(not any(p[k] for k in ("post_lo","post_hi","va","pa","value","ordinal")),"boundary payload")
                if kind!=3:
                    require(p["extra"]==p["aux"]==0,"unbound boundary operand")
            if kind==1:
                require(current is None and r_count<len(expected_runs),"unexpected user ERET")
                i,prep,closing=expected_runs[r_count]
                require(len(events)==i+1,"ERET is out of journal order")
                counter=p["counter"]
                r_count+=1
                require(p["gen"]==r_count and p["pc"]==0x2002154 and p["counter"]==closing["resume"]
                        and prep["raw"][22]==closing["raw"][21]==maps[cpu]["ttbr"] and closing["src"]==cpu
                        and prep["now"]<=closing["resume"]<closing["entry"],"ERET/counter/CPU/root/journal binding")
                current=dict(cpu=cpu,gen=r_count,prepared_row=i,R=closing["resume"],E=closing["entry"],
                             expected_pc=prep["selected"][32],stores=[],first=None)
                runs[cpu].append(current)
            elif kind==2:
                require(current is not None and current["first"] is None and p["gen"]==current["gen"] and
                        p["pc"]==current["expected_pc"] and p["counter"]==current["R"],"first actual EL0 after ERET")
                current["first"]=p["pre_hi"]
                f_count+=1
            elif kind==3:
                require(p["extra"] in (1,2) and len(events)<len(group),"unexpected architectural change")
                row=group[len(events)]
                require(p["gen"]==r_count and p["extra"]==(1 if row["cause"]==1 else 2) and
                        p["aux"]==vec+128*(5 if row["src"]==2 else 9 if row["cause"]==1 else 8),
                        "discontinuity class and destination vector")
                if current is not None:
                    require(current["first"] is not None and p["counter"]==current["R"] and
                            row["src"]==cpu and row["row"]==current["prepared_row"]+1,"interrupted user run linkage")
                    current["boundary_seq"]=p["seq"]
                    current=None
                else:
                    require(row["src"]==2,"unrecorded EL0 execution")
                events.append([p,None])
            elif kind==4:
                require(events and events[-1][1] is None and current is None and p["pc"]==events[-1][0]["aux"] and
                        p["gen"]==r_count,"missing/duplicated vector landing")
                events[-1][1]=p
            elif kind==5:
                sampled+=1
                require(current is not None and current["first"] is not None and
                        (p["gen"],p["counter"])==(current["gen"],current["R"]) and
                        p["pre_hi"]<=p["post_lo"]<=p["post_hi"] and
                        (p["pc"],p["aux"],p["va"],p["pa"])==(SITES[cpu],SITES[cpu]+4,RESULT,PADDR[cpu]),
                        "store/fall-through chronology, ownership or mapping")
                latest=p["post_hi"]
                require(p["ordinal"]==256*sampled and last_value<p["value"]<=maps[cpu]["terminal"] and
                        0<=p["value"]-p["ordinal"]-(1-cpu)<=head["aborted"],"completed private store value/ordinal")
                last_value=p["value"]
                current["stores"].append(p)
            else:
                require(current is None and p["gen"]==r_count and
                        (kind,p["pc"],idle_pending) in ((6,0x2002170,False),(7,0x2002174,True)),
                        "idle return/landing site or generation")
                idle_pending=kind==6
        require(not idle_pending and current is None and len(events)==len(group)==head["discontinuities"] and all(v for _,v in events)
                and r_count==f_count==len(expected_runs)==head["returns"]==head["first_user"]==head["generation"]
                and sampled==head["completed"]//256 and sampled<=(307,276)[cpu],"incomplete correlated trace")
        counters.append(head)
    for cpu in (0,1):
        require(any(rows[cpu][r["prepared_row"]]["op"]==30 and
                    rows[cpu][r["prepared_row"]]["cause"]==1 and
                    rows[cpu][r["prepared_row"]]["src"]==cpu and
                    rows[cpu][r["prepared_row"]]["selected"]==rows[cpu][r["prepared_row"]]["saved"]
                    and r["stores"] for r in runs[cpu]),"missing actual PPI30 resumption with subsequent private stores")
    chain=strict_chain(runs,res,offset)
    return dict(position=FIRST,session=a.session_sha,reference=SESSIONS["p2"],strict_overlap="established" if chain else "unobserved",
                overlap=chain,guest_offset=offset,counts=counters,verified_store_samples=[sum(len(r["stores"]) for r in rs) for rs in runs],
                products=legacy["products"],exits=legacy["exits"],original_scores="separate retained firsts")


def main():
    p=argparse.ArgumentParser(description=__doc__)
    sub=p.add_subparsers(dest="action",required=True)
    s=sub.add_parser("seal")
    for name in ("p2","out","qemu","plugin","header","build_receipt","availability"):
        s.add_argument(name,type=Path)
    s.add_argument("availability_sha")
    for name in ("capture","report"):
        c=sub.add_parser(name)
        c.add_argument("out",type=Path)
        c.add_argument("seal_sha")
        if name=="report":
            c.add_argument("session_sha")
        else:
            c.add_argument("--nm",default="aarch64-linux-gnu-nm")
            c.add_argument("--objdump",default="aarch64-linux-gnu-objdump")
    a=p.parse_args()
    a.out=a.out.resolve()
    if a.action=="seal":
        seal(a)
        return 0
    require(sha(a.out/"seal.json")==a.seal_sha,"seal identity")
    r=obj(a.out/"seal.json")
    for n,h in r["files"].items():
        require(Path(n).name==n and sha(a.out/n)==h,"changed sealed input "+n)
    if a.action=="report":
        result=report(a,r)
        print(json.dumps(result,indent=2))
        return 0 if result["strict_overlap"]=="established" else 2
    a.package=Path(r["p2"])
    original(a.package)
    a.qemu=r["qemu"]
    require(sha(Path(a.qemu))==MODEL,"emulator changed after seal")
    a.trace=True
    parent=a.out
    a.out=parent/FIRST
    return capture(a,Observer(parent/"observer.so"))


if __name__=="__main__":
    try:
        sys.exit(main())
    except (OSError,ValueError,KeyError,IndexError,struct.error) as exc:
        print(json.dumps(dict(strict_overlap="unobserved",reason=str(exc))))
        sys.exit(2)
