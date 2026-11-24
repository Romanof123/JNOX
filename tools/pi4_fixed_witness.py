"""Decode the dedicated Pi 4 engineering serial profile and bind it to packaged bytes."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import re
import shutil
import subprocess

MASK = (1 << 64)-1
WIDTH = {0:13, 1:5, 2:25, 3:8, 4:6, 5:12}
HANDOFF = "pi4-fixed-rr-handoff"
PROFILES = ("pi4-fixed-fifo-witness", "pi4-fixed-rr-witness", HANDOFF)


def check(ok, why):
    if not ok:
        raise ValueError(why)


def fold(h, value):
    for byte in value.to_bytes(8,"little"):
        h = ((h ^ byte)*0x100000001b3) & MASK
    return h


def decode(data):
    lines = data.splitlines(keepends=True)
    records = []
    hash_value=0xcbf29ce484222325
    sealed=False
    for line in lines:
        check(not sealed,"bytes after engineering footer")
        if not line.startswith(b"PW "):
            check(not records, "unexpected bytes in engineering export")
            continue
        check(not sealed and line.endswith(b"\n"),"truncated or extra engineering row")
        fields=line.split()
        if fields[1:2] == [b"END"]:
            check(fields[2:] == [str(len(records)).encode(),str(hash_value).encode()],"invalid footer")
            sealed=True
            continue
        check(all(v.isdigit() and len(v)<=20 for v in fields[1:]),"invalid unsigned row")
        tag,*values=map(int,fields[1:])
        check(tag in WIDTH and len(values)==WIDTH[tag] and all(v<=MASK for v in values),"invalid schema")
        for value in [tag,*values]: hash_value=fold(hash_value,value)
        records.append((tag,values))
    check(sealed and records and records[0][0]==0 and records[-1][0]==4,"missing terminal export")
    return records


def evidence(rows, profile, image_id, executable, symbols):
    header=rows[0][1]
    version,tag,boot,rr,core,el,freq,cap,scap,frame_size,n,lost,worker_lost=header
    check((version,tag,rr,el,cap,scap)==(2 if profile==HANDOFF else 1,image_id,int("rr-" in profile),1,64,128)
          and freq>0 and frame_size>0 and 1<=n<=cap and lost==worker_lost==0,"invalid header or capture loss")
    check(core & 0xff00ffffff == 0, "unexpected MPIDR affinity")
    slots={}
    mapping=([0,2,1] if rr else [0,1,2])+[3]
    def code(pc):
        return pc%4==0 and any(lo<=pc<hi for lo,hi in executable)
    for i,(slot,role) in enumerate(zip((0,1,2,4),mapping),1):
        kind,(sid,r,lo,hi,pc)=rows[i]
        check(kind==1 and (sid,r)==(slot,role) and hi-lo==16384 and code(pc),"incorrect stack/role mapping")
        check(all(hi<=a or b<=lo for a,b,_ in slots.values()),"overlapping stacks")
        base,length=symbols["pi4_witness_stacks"]
        check(length==5*16384 and lo==base+slot*16384,"stack differs from exact ELF")
        check(pc==symbols["entries"][role], "entry differs from linked role function")
        slots[slot]=(lo,hi,role)
    events=[]
    saved={}
    samples={}
    terminal=rows[-1][1]
    body = rows[5:-1]
    if profile == HANDOFF:
        check(len(body) >= 2 and [tag for tag,_ in body[-2:]] == [5,5], "missing handoff cells")
        cells = [r for _,r in body[-2:]]
        body = body[:-2]
    for kind,r in body:
        if kind==2:
            gen,reason,source,t,owner,chosen,fp,ipc,isp,nfp,npc,nsp,arg,res,armed,due,ctl,cval,pre,service,old_cval,cpu,pa,pb,pub=r
            check(gen==len(events)+1 and chosen in slots and cpu==core and t<=pub and code(npc),
                  "invalid dispatch sequence, CPU or selected PC")
            lo,hi,_=slots[chosen]
            check(lo<=nfp and nfp%16==0 and nfp+frame_size==nsp<=hi,"invalid selected frame")
            if armed: check(armed==1 and due==cval and ctl&3==1,"invalid timer program")
            else: check(ctl&1==0,"timer was enabled unexpectedly")
            if events:
                check(owner==events[-1][5] and t>=events[-1][24] and code(ipc),"invalid outgoing owner, time or PC")
                lo,hi,_=slots[owner]
                check(lo<=fp and fp%16==0 and fp+frame_size==isp<=hi,"invalid saved frame")
                saved[owner]=(fp,ipc,isp)
                if reason==1:
                    check(source&1023==30 and pre&7==5 and old_cval<=t,"invalid timer acknowledge")
                else:
                    check(reason==2 and source>>26==0x15 and source&65535 in (0,1,2),"invalid SVC syndrome")
            else:
                check(reason==0 and owner==MASK and (fp,ipc,isp)==(0,0,0),"invalid boot dispatch")
            if not any(e[5]==chosen for e in events) and chosen!=4:
                check(npc==symbols["fixed_runtime_enter"][0], "first task PC differs from trampoline")
            if chosen in saved and not (gen==n and chosen==4):
                check(saved[chosen]==(nfp,npc,nsp),"selected frame does not match saved continuation")
            events.append(r)
        else:
            check(kind==3 and events,"orphan work sample")
            gen,phase,role,cpu,pc,sp,t,value=r
            selected=events[-1]
            lo,hi,want_role=slots[selected[5]]
            check(gen==selected[0] and phase in (0,1) and role==want_role<3 and cpu==core
                  and lo<sp<=hi and sp%16==0 and code(pc) and t>=selected[24] and value>0
                  and (gen,phase) not in samples,"invalid worker generation, role, SP, PC or progress")
            site,size=symbols["pi4_witness_site"]
            check(site<=pc<site+size, "worker probe differs from linked observation site")
            if phase==1:
                check(role in (0,1) and value>selected[22+role],"work did not advance after selection")
            samples[gen,phase]=r
    check(len(events)==n,"missing dispatch row")
    gen,cpu,pc,sp,t,entry=terminal
    lo,hi,_=slots[4]
    last=events[-1]
    check(gen==n and cpu==core and code(pc) and code(entry) and lo<sp<=hi and sp%16==0
          and last[1]==2 and last[2]&65535==1 and last[5]==4 and last[10]==entry
          and t>=last[24],"terminal idle mismatch")
    site,size=symbols["pi4_witness_site"]
    check(site<=pc<site+size and entry==symbols["pi4_witness_terminal"][0], "terminal PC differs from ELF")
    exited=[slots[e[4]][2] for e in events if e[1]==2 and e[2]&65535==1]
    check(sorted(exited)==[0,1,2],"missing role exit")
    for (seq,_),s in samples.items():
        check(seq<n and s[6]<=events[seq][3],"sample after intervening selection")
    complete_roles={s[2] for (_,phase),s in samples.items() if phase==0}
    check(complete_roles=={0,1,2},"missing task activation")
    def role(slot): return slots[slot][2]
    preemptions=[]
    rotations=[]
    witnessed=False
    rotated=False
    for i,e in enumerate(events[1:],1):
        if e[1]==1 and role(e[4])==0 and role(e[5])==2 and e[22]>0 and (e[0],0) in samples:
            high_exited=False
            for resumed in events[i+1:]:
                high_exited |= resumed[1]==2 and resumed[2]&65535==1 and role(resumed[4])==2
                if role(resumed[5])==0:
                    continued=(high_exited and resumed[9:12]==e[6:9] and (resumed[0],1) in samples
                               and samples[resumed[0],1][7]>e[22])
                    witnessed |= continued
                    preemptions.append(dict(irq_generation=e[0],high_entry=samples[e[0],0],
                                            outgoing_frame_pc_sp=e[6:9],outgoing_progress=e[22],
                                            resume_generation=resumed[0],continued=continued))
                    break
        if e[1]==1 and {role(e[4]),role(e[5])}=={0,1} and (e[0],1) in samples:
            rotations.append(dict(generation=e[0],outgoing=role(e[4]),incoming=role(e[5]),
                                  incoming_progress=samples[e[0],1][7]))
    if rotations:
        # Observe both workers on a later generation following actual execution.
        rotated=all(len({seq for (seq,k),s in samples.items() if k==1 and s[2]==r})>=2 for r in (0,1))
    result = dict(profile=profile,image_id=image_id,session_counter=boot,frequency=freq,
                dispatches=n,worker_samples=len(samples),higher_priority_preempt_resume=witnessed,
                peer_timer_rotation=rotated,complete=witnessed and (not rr or rotated),
                preemptions=preemptions,rotations=rotations,
                dispatch_rows=events,work_rows=list(samples.values()),terminal_idle=terminal,
                task_sample_intervals=[dict(generation=seq,role=s[2],sample_kind=k,
                    select_to_sample_ticks=s[6]-events[seq-1][24],
                    observed_stack_bytes=slots[events[seq-1][5]][1]-s[5]) for (seq,k),s in samples.items()],
                external_instruction_history_checked=False)

    if profile == HANDOFF:
        details = handoff_evidence(cells, slots, events, samples, symbols, core, preemptions)
        result.update(structural_valid=True, legacy_loop_complete=result["complete"],
                      peer_dispatch_work=[dict(generation=e[0], reason=e[1], incoming=role(e[5]),
                                               arithmetic_observed=(e[0],1) in samples)
                                          for e in events if role(e[5]) in (0,1)], **details)
        result["complete"] = witnessed and details["rr_data_handoff"]
    return result


def handoff_evidence(cells, slots, events, samples, symbols, core, preemptions):
    mailbox, size = symbols["fixed_app_observations"]
    check(size == 64, "unexpected observation layout")
    for owner, row in enumerate(cells):
        op,status,begin,end,lo,hi,address,value,pc,sp,cpu,role = row
        check(op == owner and status in (0,1,2), "duplicate or invalid owner cell")
        if status == 0:
            check(not any(row[2:]), "partial empty cell")
            continue
        check(role == owner and cpu == core and value > 0 and address == mailbox+56,
              "wrong handoff role, CPU, payload or mailbox")
        check(1 <= begin <= end < len(events) and (status == 1) == (begin == end),
              "invalid handoff generation status")
        first, last = events[begin-1], events[end-1]
        low, high, selected = slots[first[5]]
        check(selected == slots[last[5]][2] == owner and low < sp <= high and sp % 16 == 0,
              "handoff outside owner stack or dispatch")
        check(first[24] <= lo <= hi <= events[end][3], "handoff outside observed interval")
        check(pc in symbols["operation_pcs"][owner], "handoff PC differs from linked atomic operation")
    a,b = cells
    if a[1] and b[1]:
        check(a[7] == b[7] and a[4] <= b[5], "conflicting mailbox values or chronology")
    observed = False
    if a[1] == b[1] == 1:
        check(a[3] < b[2] and a[5] <= b[4], "overlapping attributed handoff")
        returned = events[a[2]-1]
        for e in events[:a[2]-1]:
            g = e[0]
            work = samples.get((g,1))
            if (e[1] == 1 and (e[4],e[5]) == (0,2) and work
                    and returned[1] == 1 and (returned[4],returned[5]) == (2,0)
                    and any(p["continued"] and p["resume_generation"] < g for p in preemptions)):
                check(work[7] <= a[7], "handoff payload precedes B work")
                observed = True
    return dict(rr_data_handoff=observed, handoff_cells=cells,
                handoff_status=["absent" if r[1]==0 else "ambiguous" if r[1]==2
                                else "attributed" for r in cells])


def ranges(elf):
    check(elf[:6]==b"\x7fELF\x02\x01" and elf[18:20]==b"\xb7\x00","wrong ELF architecture")
    pos=struct.unpack_from("<Q",elf,32)[0]
    size,count=struct.unpack_from("<HH",elf,54)
    result=[]
    for i in range(count):
        typ,flags,off,va,pa,filesz,memsz,align=struct.unpack_from("<IIQQQQQQ",elf,pos+i*size)
        if typ==1 and flags&1: result.append((va,va+filesz))
    check(result,"ELF lacks executable ranges")
    return result


def symbol_map(path,profile):
    nm=shutil.which("aarch64-linux-gnu-nm") or shutil.which("llvm-nm")
    check(nm,"AArch64 GNU nm or llvm-nm required")
    table=[]
    for line in subprocess.check_output([nm,"-S","-C","--defined-only",str(path)],text=True).splitlines():
        fields=line.split(maxsplit=3)
        if len(fields)==4:
            a,z,k,name=fields
            table.append((re.sub(r"::h[0-9a-f]{16}$","",name),int(a,16),int(z,16)))
    def sym(name):
        found=[(a,z) for n,a,z in table if n==name or n.endswith("::"+name)]
        check(len(found)==1, "ambiguous or missing symbol "+name)
        return found[0]
    out={name:sym(name) for name in ("pi4_witness_site","pi4_witness_terminal","pi4_witness_stacks",
                                    "fixed_runtime_idle","fixed_runtime_enter")}
    roles=("worker_a","worker_b","alarm") if "rr-" in profile else ("producer","sleeper","alarm")
    out["entries"]=[sym("fixed_apps::selected::"+role)[0] for role in roles]+[out["fixed_runtime_idle"][0]]
    if profile == HANDOFF:
        out["fixed_app_observations"] = sym("fixed_app_observations")
        elf = path.read_bytes()
        pos = struct.unpack_from("<Q",elf,32)[0]
        stride,count = struct.unpack_from("<HH",elf,54)
        out["operation_pcs"] = []
        for name,opcode in (("pi4_rr_publish",0xc89ffc00),("pi4_rr_consume",0xc8dffc00)):
            start,length = sym(name)
            sites = set()
            for i in range(count):
                typ,flags,off,va,_,filesz,_,_ = struct.unpack_from("<IIQQQQQQ",elf,pos+i*stride)
                if typ==1 and flags&1 and va<=start and start+length<=va+filesz:
                    for pc in range(start+8,start+length,4):
                        adr,sp,word = struct.unpack_from("<III",elf,off+pc-va-8)
                        # The producer captures this PC with ADR +8 and MOV Xd,SP.
                        if (adr & 0xffffffe0 == 0x10000040 and sp & 0xffffffe0 == 0x910003e0
                                and word & 0xfffffc00 == opcode):
                            sites.add(pc)
            check(len(sites) == 1, "ambiguous or missing linked atomic operation in "+name)
            out["operation_pcs"].append(sites)
    return out


def score(data, profile, image_id, elf_path):
    from fixed_runtime_observer import validate
    check(len(data)<=256*1024,"capture exceeds engineering byte bound")
    pos=data.find(b"PW 0 ")
    check(pos>=0 and validate(data[:pos],b"FIXED RR" if "rr-" in profile else b"FIXED FIFO",b"PI4"),
          "ordinary workload byte order differs")
    return evidence(decode(data[pos:]),profile,image_id,ranges(elf_path.read_bytes()),symbol_map(elf_path,profile))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("uart",type=Path)
    p.add_argument("--profile",required=True,choices=PROFILES)
    p.add_argument("--manifest",type=Path,required=True)
    p.add_argument("--elf",type=Path,required=True)
    p.add_argument("--image",type=Path,required=True)
    a=p.parse_args()
    try:
        m=json.loads(a.manifest.read_text())
        elf=a.elf.read_bytes()
        for data,key in ((elf,"elf_sha256"),(a.image.read_bytes(),"image_sha256")):
            check(hashlib.sha256(data).hexdigest()==m[key],"capture package hash mismatch")
        check(m["profile"]==a.profile,"package profile mismatch")
        result=score(a.uart.read_bytes(),a.profile,m["image_id"],a.elf)
        print(json.dumps(result,indent=2))
        if not result["complete"]: raise SystemExit(2)
    except (ValueError,KeyError,IndexError,OSError,struct.error,subprocess.CalledProcessError) as exc:
        p.exit(1,f"incomplete evidence: {exc}\n")


if __name__=="__main__":
    main()
