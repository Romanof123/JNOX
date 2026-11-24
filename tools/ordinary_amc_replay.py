"""Replay one first CHG-076 cold-guest position against the frozen demand table.

Usage: python tools/ordinary_amc_replay.py FROZEN_DIRECTORY TRUSTED_SESSION_SHA256
Requires pyelftools. For each case only the first retained position is scored.
"""
import argparse
from collections import Counter, defaultdict
from dataclasses import dataclass
from fractions import Fraction
import io
import json
from pathlib import Path
import re
import struct
import sys

from ordinary_amc_capture import CASES, FLAGS, sha


U64=(1<<64)-1
RING=(2,5,1,6,3,7,4,8)
DEMAND=((1,1,0,0),(2,3,2,1),(2,4,2,1))
DEADLINE=(3,4,3,5)
PRIORITY=(1,4,2,3)
LO=(2,1,2,1)
HI=(0,3,0,1)
FRAME=832
WIDTH=dict(RELEASE=8, GRANT=12, IRQ=11, WORK=9, RETIRE=7,
           CARRY=5, DENY=3, FAULT=3, RESET=3, FINAL=5, COMPLETE=13, FROZEN=7)


class Incomplete(ValueError):
    """Required observation is missing or cannot be uniquely attributed."""


def need(ok,why):
    if not ok:
        raise ValueError(why)


def obj(pairs):
    data={}
    for k,v in pairs:
        need(k not in data,"duplicate manifest key")
        data[k]=v
    return data


def bounded(path,limit):
    with path.open("rb") as f:
        data=f.read(limit+1)
    need(len(data)<=limit,"oversize "+path.name)
    return data


def words(tokens,n):
    need(len(tokens)==n and all(re.fullmatch(rb"0|[1-9][0-9]{0,19}",v) for v in tokens),
         "invalid row shape or decimal encoding")
    values=tuple(map(int,tokens))
    need(all(v<=U64 for v in values),"unrepresentable counter")
    return values


def parse(raw):
    lines=[line.split() for line in raw.splitlines() if line.startswith(b"AMR ")]
    if any(line[1:2]==[b"FAIL"] for line in lines):
        raise ValueError("guest failure: "+" ".join(x.decode() for line in lines if line[1:2]==[b"FAIL"] for x in line))
    if len(lines)<2 or lines[0][:2]!=[b"AMR",b"BEGIN"] or lines[-1]!=[b"AMR",b"END"]:
        raise Incomplete("missing complete AMR envelope")
    header=words(lines[0][2:],5)
    result=[]
    for rawrow in lines[1:-1]:
        tag=rawrow[1].decode("ascii")
        need(tag in WIDTH,"unexpected AMR row "+tag)
        result.append((tag,words(rawrow[2:],WIDTH[tag])))
    return header,result


def sample_sum(task,g,n):
    start=(task+g)%8
    cycles,rest=divmod(n,8)
    return (36*cycles+sum(RING[(start+i)%8] for i in range(rest))) & U64


@dataclass
class Job:
    task:int
    generation:int
    demand:int
    deadline:int
    used:int=0


def model(case):
    """Construct the released instances and dispatch/retirement history from first principles."""
    live={};epochs=[0]*4;releases=[];slots=[];states=[];faults=[]
    hi=False
    def release(boundary,task,units):
        need(task not in live,"overlapping model generation")
        job=Job(task,epochs[task],units,boundary+DEADLINE[task])
        epochs[task]+=1;live[task]=job
        releases.append((boundary,task,job.generation,units,job.deadline))
    release(0,0,1)
    for k in range(3 if case==0 else 6):
        if k==1:
            for t,units in enumerate(DEMAND[case]):
                if units:release(k,t,units)
        if k==2:
            states.append((k,*(live[t].demand-live[t].used if t in live else 0 for t in range(4))))
        if k==5:
            need(not live,"reset while runnable")
            hi=False;release(k,0,1)
        drop=0
        while True:
            j=max(live.values(),key=lambda j:(PRIORITY[j.task],-j.task))
            if HI[j.task] and j.used==HI[j.task]:
                faults.append((k,j.task,j.generation));del live[j.task];continue
            if not hi and HI[j.task] and j.used==LO[j.task]:
                hi=True
                for t in tuple(live):
                    if not HI[t]:drop|=1<<t;del live[t]
            break
        j.used+=1
        slots.append((k,j.task,j.generation,int(hi),drop,j.used,j.demand-j.used,j.deadline))
        if j.used==j.demand:del live[j.task]
    return releases,slots,states,faults


def symbols(raw):
    from elftools.elf.elffile import ELFFile
    elf=ELFFile(io.BytesIO(raw))
    need(elf.elfclass==64 and elf.little_endian and elf["e_machine"]=="EM_AARCH64","invalid target ELF")
    table=elf.get_section_by_name(".symtab")
    matches=table.get_symbol_by_name("ordinary_amc_worker") if table else None
    if not matches or len(matches)!=1:raise Incomplete("missing unique linked worker")
    sym=matches[0];base=sym["st_value"];size=sym["st_size"]
    section=elf.get_section(sym["st_shndx"])
    need(section["sh_flags"]&4 and base%4==size%4==0 and size>0,"invalid worker extent")
    offset=base-section["sh_addr"];code=section.data()[offset:offset+size]
    need(len(code)==size,"truncated worker")
    instructions=dict((base+4*i,u[0]) for i,u in enumerate(struct.iter_unpack("<I",code)))
    # Compute the cyclic region from actual AArch64 branches. An optimized
    # worker may put the publication block physically beyond the backedge.
    graph={pc:set() for pc in instructions}
    for pc,w in instructions.items():
        fall=True;target=None
        if w&0xfc000000==0x14000000:
            count=26;imm=w&0x3ffffff;fall=False
        elif w&0xff000010==0x54000000 or w&0x7e000000==0x34000000:
            count=19;imm=(w>>5)&0x7ffff
        elif w&0x7e000000==0x36000000:
            count=14;imm=(w>>5)&0x3fff
        else:
            count=0;imm=0
            if w&0xfffffc1f in (0xd65f0000,0xd61f0000):fall=False
        if count:
            target=pc+4*(imm-(1<<count) if imm&(1<<(count-1)) else imm)
        if target in instructions:graph[pc].add(target)
        if fall and pc+4 in instructions:graph[pc].add(pc+4)
    def reach(start,edges):
        seen=set();pending=[start]
        while pending:
            n=pending.pop()
            if n not in seen:
                seen.add(n);pending.extend(edges[n]-seen)
        return seen
    reverse={pc:set() for pc in graph}
    for pc,successors in graph.items():
        for dest in successors:reverse[dest].add(pc)
    reachable=reach(base,graph);components=set()
    for pc in reachable:
        component=frozenset(reach(pc,graph)&reach(pc,reverse))
        if len(component)>1:
            # Volatile ring is [u64;8], loaded with a register index or
            # from an address formed by a separately emitted ADD.
            load64=any(instructions[x]&0xffe00c00==0xf8600800 or
                       instructions[x]&0xffc00000==0xf9400000 for x in component)
            counter=any(instructions[x]&0xffffffe0==0xd53be020 for x in component)
            if load64 and counter:components.add(component)
    loops=[]
    if len(components)==1:
        for pc in sorted(components.pop()):
            if loops and pc==loops[-1][1]:loops[-1]=(loops[-1][0],pc+4)
            else:loops.append((pc,pc+4))
    stack_symbols=[s for s in table.iter_symbols() if "ordinary_amc" in s.name and "STACKS" in s.name]
    if len(stack_symbols)!=1:raise Incomplete("missing unique linked ordinary AMC stack pool")
    pool=stack_symbols[0]
    return base,base+size,loops,(pool["st_value"],pool["st_size"])



def finite_layout(raw):
    from elftools.elf.elffile import ELFFile
    elf=ELFFile(io.BytesIO(raw))
    table=elf.get_section_by_name(".symtab")
    def code(name):
        hits=table.get_symbol_by_name(name) or []
        if len(hits)!=1:raise Incomplete("missing or ambiguous "+name)
        sym=hits[0];section=elf.get_section(sym["st_shndx"])
        start=sym["st_value"];size=sym["st_size"]
        offset=start-section["sh_addr"]
        need(section["sh_flags"]&4 and size>0 and (start|size)%4==0 and offset>=0,
             "invalid linked function "+name)
        data=section.data()[offset:offset+size]
        need(len(data)==size,"incomplete linked function")
        return start,[n[0] for n in struct.iter_unpack("<I",data)]
    worker,instructions=code("ordinary_amc_finite_worker")
    idle,idle_code=code("ordinary_amc_completed_idle")
    # SVC immediate #76 encodes as 0xd4000981; ELR points to its successor.
    calls=[worker+4*(i+1) for i,w in enumerate(instructions) if w==0xd4000001+(76<<5)]
    need(len(calls)==1,"finite worker lacks unique linked SVC #76")
    need(any(w&0xffe00c00==0xf8600800 or w&0xffc00000==0xf9400000 for w in instructions),
         "missing linked 64-bit worker load")
    need(0xd503207f in idle_code,"completion idle lacks linked WFI")
    stacks=[s for s in table.iter_symbols() if "ordinary_amc" in s.name and "IDLE" in s.name
            and s["st_size"]==16384]
    if len(stacks)!=1:raise Incomplete("missing or ambiguous private idle stack")
    lo=stacks[0]["st_value"]
    return worker,calls[0],idle,idle+4*len(idle_code),lo,lo+16384


def external(raw):
    observed=[];pending=None
    for num,line in enumerate(raw.decode().splitlines(),1):
        exc=re.fullmatch(r"Taking exception \d+ \[([^]]+)\](?: on CPU (\d+))?",line)
        elr=re.fullmatch(r"\.\.\.with ELR (0x[0-9a-fA-F]+)",line)
        ret=re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC (0x[0-9a-fA-F]+)",line)
        if exc:
            need(pending is None and exc[1] in ("IRQ","SVC") and exc[2] in (None,"0"),"unexpected exception")
            pending=(exc[1].lower(),num)
        if elr:
            need(pending is not None,"unattributed exception PC")
            observed.append((pending[0],int(elr[1],16),pending[1],num));pending=None
        if ret:
            need(pending is None,"return before IRQ ELR")
            observed.append(("eret",int(ret[1],16),num,num))
    if pending is not None:raise Incomplete("truncated final IRQ")
    return observed

def assess(case,raw,elf,trace):
    header,events=parse(raw)
    version,case_id,hz,origin,q=header
    need((version==1 or version==2 and case in (1,2)) and case==case_id and hz>=20 and q==hz//20 and origin>0,"case/frequency/origin")
    releases,slots,carry,faults=model(case)
    need(origin+(len(slots)+1)*q<=U64,"compare calculation overflows")
    grouped=defaultdict(list)
    for seq,(kind,d) in enumerate(events):
        grouped[kind].append((seq,d))
    def by(kind):return [d for _,d in grouped[kind]]
    need([r[:5] for r in by("RELEASE")]==releases,"admitted generations/demands/deadlines")
    need(by("FAULT")==faults,"exhaustion boundary/generation")
    need(by("CARRY")==(carry if case else []),"residual-demand snapshot")
    need(by("DENY")==([(2,0,0)] if case else []),"LO admission/pending HI reset controls")
    need(by("RESET")==([(5,0,1)] if case else []),"quiescent reset outcomes")
    need(all(len(grouped[k])==len(slots) for k in ("GRANT","IRQ","WORK","RETIRE")),
         "missing/duplicate grant, IRQ, task work or retirement")
    base,limit,loops,(pool_base,pool_size)=symbols(elf)
    if not loops:raise Incomplete("no linked telemetry-input loop backedge identified")
    finite=finite_layout(elf) if version==2 else None
    need(len(by("COMPLETE"))==len(by("FROZEN"))==int(version==2),"missing or extraneous finite records")
    stacks={};release_seq={}
    for seq,(at,t,g,units,deadline,lo,hi,entry) in grouped["RELEASE"]:
        need(lo%16==hi%16==0 and 0x40200000<=lo<=hi-FRAME<hi<=0x48000000
             and entry==(finite[0] if finite and (t,g)==(0,2) else base),"invalid stack extent or Rust entry")
        key=(t,g)
        need(hi-lo==16384 and lo==pool_base+16384*len(stacks) and hi<=pool_base+pool_size,
             "release does not use its linked private stack")
        need(all(hi<=a or b<=lo for a,b in stacks.values()),"overlapping generation stacks")
        stacks[key]=(lo,hi);release_seq[key]=seq
        if at:need(grouped["RETIRE"][at-1][0]<seq,"release preceding prior retirement")
    last_irq={};last_commit={};stopped={};expected_external=[];transition=None
    service=Counter();products=[];previous_irq=0
    def in_stack(key,frame,sp):
        lo,hi=stacks[key]
        return frame%16==0 and lo<=frame and frame+FRAME==sp<=hi
    for s in slots:
        k,t,g,hi,discard,used,left,deadline=s
        key=t,g;b=origin+(k+1)*q
        grant_i,grant=grouped["GRANT"][k]
        irq_i,irq=grouped["IRQ"][k]
        work_i,work=grouped["WORK"][k]
        end_i,end=grouped["RETIRE"][k]
        need(grant[:5]==(k,t,g,hi,discard) and irq[:3]==work[:3]==(k,t,g)
             and end[:6]==(k+1,t,g,used,left,deadline),"owner/mode/generation/retirement mismatch")
        need(release_seq[key]<grant_i<irq_i<work_i<end_i
             and (not k or grouped["RETIRE"][k-1][0]<grant_i),"grant/IRQ/retirement order")
        _,_,_,_,_,commit,f,pc,sp,compare,readback,x19=grant
        _,_,_,iar,icval,ctl,stamp,fi,pci,spi,xi=irq
        need(compare==readback==icval==b and iar&1023==30 and ctl&7==5
             and previous_irq<=commit<stamp and b<=stamp<b+q,"timer IAR/CTL/compare/entry")
        finite_slot=bool(finite and k==5)
        need(in_stack(key,f,sp) and (finite_slot or in_stack(key,fi,spi)),"saved frame/stack mismatch")
        if key in last_irq:
            need((f,pc,sp,x19)==last_irq[key],"resumed generation lost frame/ELR/SP/x19")
        else:
            need(pc==(finite[0] if finite_slot else base),"fresh generation did not start at linked Rust entry")
        outside_loop=not any(begin<=pci<finish for begin,finish in loops)
        if outside_loop and not finite_slot:
            raise Incomplete(f"slot {k}: IRQ ELR {pci:#x} outside identifiable linked input loop")
        last_irq[key]=fi,pci,spi,xi
        expected_external.append(("eret",pc))
        if finite_slot:
            complete_i,complete=grouped["COMPLETE"][0]
            frozen_i,frozen=grouped["FROZEN"][0]
            task_entry,svc_elr,idle_entry,idle_end,idle_lo,idle_hi=finite
            _,_,_,n,value,tick,svc_tick,wf,welr,wsp,idle_frame,selected_pc,idle_sp=complete
            need(grant_i<complete_i<irq_i<frozen_i<work_i<end_i and
                 complete[:5]==(5,0,2,8,sample_sum(0,2,8)) and
                 frozen==(5,0,2,n,value,tick,stamp),"finite completion/frozen evidence mismatch")
            need(commit<=tick<=svc_tick<b<=stamp and in_stack(key,wf,wsp) and welr==svc_elr,
                 "late or unbound finite completion SVC")
            need(all(hi<=idle_lo or idle_hi<=lo for lo,hi in stacks.values())
                 and selected_pc==idle_entry and idle_sp==idle_hi and idle_frame==idle_hi-FRAME
                 and idle_lo<=fi and fi%16==0 and fi+FRAME==spi<=idle_hi and
                 idle_entry<=pci<idle_end and (pci-idle_entry)%4==0,
                 "closing IRQ is not on linked private idle context")
            need(work[3:6]==(work[3],1,sample_sum(0,2,1)) and work[6:]==(tick,n,value),
                 "finite first/last task publication differs from completion")
            expected_external.extend([("svc",svc_elr),("eret",idle_entry)])
        expected_external.append(("irq",pci))
        _,_,_,first_tick,first_n,first_sum,last_tick,last_n,last_sum=work
        before=last_commit.get(key,(0,0,0))[0]
        if key not in last_commit:need(first_n==1,"fresh generation did not start at first sample")
        need(first_n>before and first_n<=last_n and first_sum==sample_sum(t,g,first_n)
             and last_sum==sample_sum(t,g,last_n),"task progress or checksum mismatch")
        need(commit<=first_tick<=last_tick<=stamp<=end[6] and last_tick>0,"task/retirement counter order")
        need(k+1<=deadline and (k+1<deadline or left==0)
             and last_tick<origin+deadline*q,"job missed declared absolute deadline")
        service[key]+=1;last_commit[key]=(last_n,last_sum,last_tick)
        stopped[key]=dict(completion_or_last_work=last_tick,retirement_sample=end[6],
                          deadline_tick=origin+deadline*q)
        if discard:
            # This decision follows the IRQ that retired slot 1 at b2.
            boundary=origin+2*q
            need(k==2 and boundary<=previous_irq<=commit<=first_tick<boundary+q,
                 "switch boundary/IRQ/commit/first task sample missed the next compare")
            transition=dict(boundary=boundary,irq=previous_irq,mode_commit=commit,task_sample=first_tick,
                            ticks=first_tick-boundary,seconds=str(Fraction(first_tick-boundary,hz)))
        previous_irq=stamp
    need(bool(transition)==bool(case),"unexpected transition count")
    if case:
        # Refusal must precede same-boundary successor. Pending-reset denial must be in HI.
        deny_seq=grouped["DENY"][0][0];reset_seq=grouped["RESET"][0][0]
        need(grouped["RETIRE"][1][0]<grouped["CARRY"][0][0]<deny_seq<grouped["GRANT"][2][0]
             and grouped["RETIRE"][4][0]<reset_seq<release_seq[0,2],"reset/release chronology")
        if faults:need(grouped["RETIRE"][3][0]<grouped["FAULT"][0][0]<grouped["GRANT"][4][0],
                       "fault advanced the modeled boundary")
        need(stopped[3,0]["retirement_sample"]<origin+6*q,"survivor completion past b6")
        if case==1:need(stopped[1,0]["retirement_sample"]<origin+5*q,"three-grant completion past b5")
        need(last_commit[0,2][2]<origin+6*q,"fresh LO result was late")
    final={}
    for t,g,n,value,tick in by("FINAL"):
        key=t,g
        need(key in stacks and key not in final and (n,value,tick)==last_commit.get(key,(0,0,0)),
             "missing, duplicated, stale or modified final generation")
        final[key]=(n,value,tick)
        products.append(dict(task=t,generation=g,samples=n,checksum=value,last_tick=tick,grants=service[key]))
    need(list(final)==list(stacks) and grouped["FINAL"][0][0]>grouped["RETIRE"][-1][0],
         "missing, reordered or premature final snapshot")
    actual=external(trace)
    if not actual:raise Incomplete("same-process IRQ/return trace absent")
    need([(kind,pc) for kind,pc,_,_ in actual]==expected_external,"linked exception/ERET history mismatch")
    result=dict(case=CASES[case],frequency=hz,epoch=origin,quantum=q,owners=[s[1] for s in slots],
                generation_products=products,switch=transition,
                completions={f"{t}/{g}":v for (t,g),v in stopped.items()},
                worker_extent=[base,limit],linked_loop_ranges=loops,
                external_pc_line_ranges=actual,scope="one finite single-core QEMU workload")
    if finite:
        c=by("COMPLETE")[0];frozen=by("FROZEN")[0]
        result["finite_completion"]=dict(wire_version=2, task=0,generation=2,
            expected_ring=[RING[(2+i)%8] for i in range(8)], samples=c[3],checksum=c[4],
            task_tick=c[5],svc_entry_tick=c[6],svc_elr=c[8],idle_entry=c[11],
            closing_irq_tick=frozen[6],frozen_tick=frozen[5],
            coverage="first/last task publications, completion SVC, idle return, closing reread and retirement; individual intermediate reads are not exported")
    return result


def replay(root,session_hash):
    session_path=root/"first/session.json"
    need(sha(session_path)==session_hash,"trusted session digest mismatch")
    session=json.loads(bounded(session_path,65536),object_pairs_hook=obj)
    freeze_path=root/"freeze.json"
    need(sha(freeze_path)==session["freeze_sha256"],"freeze hash mismatch")
    freeze=json.loads(bounded(freeze_path,65536),object_pairs_hook=obj)
    need(freeze["case"]==session["case"] and freeze["flags"]==FLAGS and freeze["case"] in CASES
         and freeze["allowance_seconds"]==15,"observer/case mismatch")
    args=session["argv"]
    need(args[:1+len(FLAGS)]==[freeze["qemu"],*FLAGS] and len(args)==len(FLAGS)+9
         and args[-8]=="-serial" and args[-7].startswith("file:")
         and Path(args[-7][5:]).name=="uart.bin" and args[-6:-3]==["-d","int","-D"]
         and Path(args[-3]).name=="exceptions.log" and args[-2]=="-kernel"
         and Path(args[-1]).name=="kernel.elf","observer command mismatch")
    folder=root/"first"
    files=session["hashes"]
    need(set(files)=={p.name for p in folder.iterdir() if p.is_file() and p.name!="session.json"},
         "first-launch artifact membership mismatch")
    for name,digest in files.items():
        need(Path(name).name==name and sha(folder/name)==digest,"artifact mismatch: "+name)
    need(sha(root/"kernel.elf")==freeze["kernel_sha256"],"ELF hash mismatch")
    if session["reason"]=="guest_failure":
        raw=bounded(folder/"uart.bin",2*1024*1024)
        raise ValueError("guest failure: "+repr([l for l in raw.splitlines() if l.startswith(b"AMR FAIL ")]))
    if session["reason"]!="guest_footer":
        raise Incomplete("retained first launch: "+session["reason"])
    need(0<session["elapsed_ns"]<=15_000_000_000,"capture allowance")
    result=assess(CASES.index(freeze["case"]),bounded(folder/"uart.bin",2*1024*1024),
                  bounded(root/"kernel.elf",64*1024*1024),bounded(folder/"exceptions.log",64*1024*1024))
    result.update(elf_sha256=freeze["kernel_sha256"],session_sha256=session_hash)
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root",type=Path)
    parser.add_argument("session_sha256")
    args=parser.parse_args()
    try:
        print(json.dumps(replay(args.root,args.session_sha256),indent=2))
        return 0
    except (Incomplete,ImportError) as error:
        outcome="inconclusive";reason=str(error)
    except (ValueError,KeyError,IndexError,TypeError,OSError) as error:
        outcome="rejected";reason=str(error)
    print(json.dumps(dict(outcome=outcome,reason=reason)))
    return 1


if __name__=="__main__":
    sys.exit(main())
