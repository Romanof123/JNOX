"""Independent CHG-075 single-position replay for pi4_migration_capture.py.

Model: python3 tools/pi4_migration_replay.py FROZEN_DIR ORDINAL SESSION_SHA256
An independently reviewed linked inner-loop interval must be supplied through
the SHA-bound ELF and its GNU disassembly (the tool locates the backedge). A valid instruction address inside the containing function
alone cannot establish that an interrupted input was being processed.
"""
import argparse
from collections import defaultdict
import io
import json
from pathlib import Path
import re
import sys
import zlib
from functools import lru_cache

from pi4_migration_capture import BASELINE, CASES, FILES, FLAGS, STOCK_SHA, STREAM
from pi4_smp_replay import bounded, digest, need, numbers, unique_fields

NA = (1 << 64) - 1
FRAME = 832


class Unobserved(ValueError):
    """A timely interruption or sufficient independent observation was absent."""


def read_json(raw):
    return json.loads(raw, object_pairs_hook=unique_fields)


def layout(elf, raw, disassembly, workload=BASELINE):
    from elftools.elf.elffile import ELFFile
    e = ELFFile(io.BytesIO(elf))
    need(e.little_endian and e.elfclass == 64 and e["e_machine"] == "EM_AARCH64"
         and e["e_entry"] == 0x80000, "unexpected ELF format/entry")
    image = bytearray()
    for seg in e.iter_segments():
        if seg["p_type"] == "PT_LOAD":
            need(0x80000 <= seg["p_vaddr"] == seg["p_paddr"]
                 and seg["p_vaddr"] + seg["p_memsz"] <= 0x100000, "invalid mapped range")
    for s in sorted(e.iter_sections(), key=lambda section: section["sh_addr"]):
        if s["sh_flags"] & 2 and s["sh_type"] != "SHT_NOBITS" and s["sh_size"]:
            off = s["sh_addr"] - 0x80000
            need(len(image) <= off and off+s["sh_size"] <= 0x80000, "invalid section layout")
            image.extend(bytes(off-len(image)))
            image.extend(s.data())
    need(raw and raw == image, "raw image differs from ELF")
    table = e.get_section_by_name(".symtab")
    stream = workload == STREAM
    name = "pi4_stream_crc32_loop_v1" if stream else "pi4_migration_input_loop"
    hits = table.get_symbol_by_name(name) if table else None
    need(hits and len(hits) == 1, "missing or ambiguous loop symbol")
    start, size = hits[0]["st_value"], hits[0]["st_size"]
    decoded = {}
    for line in disassembly.decode().splitlines():
        m = re.match(r"^\s*([0-9a-f]+):\s+([0-9a-f]{8})\s+(\S+)\s*(.*)$", line)
        if m and start <= int(m[1], 16) < start+size:
            pc = int(m[1], 16)
            need(pc not in decoded and int(m[2], 16) ==
                 int.from_bytes(raw[pc-0x80000:pc-0x80000+4], "little"), "disassembly mismatch")
            decoded[pc] = (m[3], m[4])
    need(set(decoded) == set(range(start,start+size,4)), "incomplete linked loop map")
    bodies = []
    for pc,(op,args) in decoded.items():
        match = re.search(r"(?:^|,\s*)([0-9a-f]+)\s+<", args)
        if op.startswith(("b.", "cb", "tb")) and match:
            target = int(match[1],16)
            if start <= target < pc:
                body = [decoded[x] for x in range(target,pc+1,4)]
                if (any(op.startswith("ld") for op,_ in body)
                        and any(op.startswith("st") for op,_ in body)
                        and any(op.startswith("add") for op,_ in body)
                        and any("x21" in args for _,args in body)
                        and (not stream or (any(op=="eor" for op,_ in body) and
                             any("[x21" in args for op,args in body if op.startswith(("ld","st")))))):
                    bodies.append((target,pc+4))
    entries = {}
    entry = "pi4_stream_crc32_m_v1" if stream else "pi4_migration_m"
    for symbol in (entry, "pi4_migration_p0", "pi4_migration_p1_open", "pi4_migration_p1"):
        items = table.get_symbol_by_name(symbol) or []
        need(len(items)==1 and items[0]["st_size"]>0, "missing/ambiguous task entry: "+symbol)
        entries[symbol]=items[0]["st_value"]
    return bodies,entries


def observed_threads(folder, identities):
    raw = read_json(bounded(folder/"threads.json",65536))
    cpu_for = {tid:int(name[4]) for tid,name in raw.items()
               if re.fullmatch(r"[1-9][0-9]*",tid) and re.fullmatch(r"CPU [0-3]/TCG",name)}
    if len(cpu_for)!=4 or set(cpu_for.values())!=set(range(4)):
        raise Unobserved("incomplete per-vCPU Linux thread identity")
    history = defaultdict(list)
    mapping = []
    for f in sorted(folder.glob("observer-*.log")):
        m = re.fullmatch(r"observer-([1-9][0-9]*).log",f.name)
        data = bounded(f,64*1024*1024)
        need(m and identities.get(f.name)==digest(data), "unfrozen observer stream")
        tid = m[1]
        cpu = cpu_for.get(tid)
        pending = None
        for line_no,line in enumerate(data.decode().splitlines(),1):
            exc = re.fullmatch(r"Taking exception \d+ \[([^]]+)\](?: on CPU (\d+))?",line)
            ret = re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC (0x[\da-fA-F]+)",line)
            elr = re.fullmatch(r"\.\.\.with ELR (0x[\da-fA-F]+)",line)
            if exc:
                need(pending is None and exc[1] in ("IRQ","SVC"), "unexpected guest exception")
                need(cpu is not None and (exc[2] is None or int(exc[2])==cpu),"missing/conflicting vCPU provenance")
                pending=(exc[1].lower(),line_no)
            if elr or ret:
                need(cpu in (0,1),"unexpected executable participant")
                if ret:
                    need(pending is None,"unresolved exception before ERET")
                    kind,first,pc="eret",line_no,int(ret[1],16)
                else:
                    need(pending is not None,"unattributed saved ELR")
                    kind,first=pending
                    pc=int(elr[1],16)
                    pending=None
                history[cpu].append((kind,pc))
                mapping.append([f.name,first,line_no,cpu,kind,pc])
        need(pending is None,"truncated exception log")
    if not history[0] or not history[1]:
        raise Unobserved("missing participating vCPU exception/return stream")
    return history,mapping


def journal(raw, case, wire=1):
    lines=[line.split() for line in raw.splitlines() if line.startswith(b"MG ")]
    if not lines or len(lines[0]) < 3 or lines[0][2] != b"BEGIN":
        raise Unobserved("no sealed migration journal")
    need(lines[0][:3] == [b"MG",str(wire).encode(),b"BEGIN"],"frozen wire version mismatch")
    need(numbers(lines[0][3:],2)==(case,3),"case or online mask mismatch")
    sections={}; freq={};pos=1
    for cpu in range(2):
        need(pos<len(lines) and lines[pos][:2]==[b"MG",b"CPU"],"missing CPU header")
        core,n,loss,hz,sealed=numbers(lines[pos][2:],5);pos+=1
        need(core==cpu and 0<n<=96 and loss==0 and hz>0 and sealed==1,"core seal/loss/frequency")
        freq[cpu]=hz; sections[cpu]=[]
        for seq in range(n):
            need(pos<len(lines) and lines[pos][:2]==[b"MG",b"ROW"],"missing journal row")
            owner,order,kind,*v=numbers(lines[pos][2:],11);pos+=1
            need((owner,order)==(cpu,seq) and 1<=kind<=(13 if wire==2 else 12),"CPU/sequence/event kind")
            sections[cpu].append((kind,tuple(v)))
    need(pos+1==len(lines) and lines[pos][:2]==[b"MG",b"END"],"missing footer or extra records")
    end=numbers(lines[pos][2:],5)
    need(end[0]==1 and end[1]==sum(map(len,sections.values())),"guest failure or count mismatch")
    return sections,freq,end[2:]

@lru_cache(maxsize=1)
def stream_reference():
    prefixes = {}
    crc = digest_value = 0
    mask = (1 << 64) - 1
    for block in range(4096):
        start = block * 1024
        end = start + 1024
        data = bytes((i ^ (i >> 8) ^ (i >> 16)) & 255 for i in range(start, end))
        crc = zlib.crc32(data, crc)
        prefixes[end] = crc
        rotated = ((digest_value << 7) | (digest_value >> 57)) & mask
        digest_value = ((rotated ^ end) + crc) & mask
    need((crc, digest_value) == (0x78d22ad1, 8765611335505774534), "stream reference product")
    return prefixes, (4096, digest_value, 4194304, crc)


def checkpoint_summary(task, epoch, workload=BASELINE):
    if task == 0 and workload == STREAM:
        return stream_reference()[1]
    start, stop = {(0,1):(0,65536), (1,1):(0,32), (2,0):(0,16), (2,1):(16,32)}[task,epoch]
    count = digest_value = partial_sum = 0
    mask = (1 << 64) - 1
    for index in range(start + 1, stop + 1):
        partial_sum += index
        if index % 16 == 0:
            rotated = ((digest_value << 7) | (digest_value >> 57)) & mask
            digest_value = ((rotated ^ index) + partial_sum) & mask
            count += 1
    return count, digest_value, stop, partial_sum


def score(sections, freq, totals, case, bodies, wire=1, workload=BASELINE):
    stream = workload == STREAM
    final_index = 4194304 if stream else 65536
    rows=[(cpu,seq,kind,data) for cpu in range(2)
          for seq,(kind,data) in enumerate(sections[cpu])]
    def events(kind):
        return [(c,n,d) for c,n,k,d in rows if k==kind]
    def only(seq,why):
        need(len(seq)==1,why)
        return seq[0]
    stacks={}
    for _,_,(role,lo,hi,*_) in events(1):
        need(role in range(7) and lo%16==hi%16==0 and hi-lo>=FRAME
             and 0x80000<=lo<hi<=0x100000,"invalid stack extent")
        need(role not in stacks or stacks[role]==(lo,hi),"conflicting stack bounds")
        stacks[role]=(lo,hi)
    need(set(stacks)==set(range(7)),"missing stack bounds")
    for i,(lo,hi) in stacks.items():
        need(all(i==j or hi<=l or h<=lo for j,(l,h) in stacks.items()),"overlapping stacks")
    def frame_ok(task,cpu,f,sp):
        role=task if task!=3 else 3+cpu
        need(role in stacks and stacks[role][0]<=f and f+FRAME==sp<=stacks[role][1]
             and f%16==0,"frame outside owning stack")
    def inside(role,addr):
        return stacks[role][0]<=addr<stacks[role][1]
    mask=(3 if case==0 else 1,1,2)
    need(len(events(6))==2, "opening and contested plans required")
    opening=only([d for _,_,d in events(6) if d[0]==0],"opening placement missing")
    need(opening==(0,5,3,0,2,*mask),"opening plan mismatch")
    ready=only([d for _,_,d in events(6) if d[0]==1],"contested snapshot missing/duplicated")
    epoch,ready_mask,online,p0,p1,*allowed=ready
    need((ready_mask,online,p0,p1)==(7,3,1,0 if case==0 else 2)
         and tuple(allowed)==mask,"contested placement/eligible set mismatch")
    external={0:[],1:[]}
    claimed={}
    for cpu,seq,d in events(2):
        ep,task,lease,f,elr,x21,sp,allowed=d
        need(task in range(3) and lease and lease not in claimed and allowed==mask[task],"lease claim")
        claimed[lease]=(ep,task,f,elr,x21,sp)
    retired={};account=defaultdict(int);local_intervals=defaultdict(list)
    for cpu,seq,d in events(3):
        ep,t,l,begin,stop,debit,total,disposition=d
        need(l in claimed and l not in retired and t==claimed[l][1]
             and mask[t] & (1<<cpu) and begin>0 and stop>=begin
             and debit==stop-begin and disposition in (0,1),"retired lease/accounting")
        retired[l]=(cpu,seq,t,debit,total,disposition)
        local_intervals[cpu].append((begin,stop,l))
        account[cpu,t]+=debit
    need(set(retired)==set(claimed),"unretired or missing claim")
    for cpu,intervals in local_intervals.items():
        ordered=sorted(intervals)
        need(all(a[1]<=b[0] for a,b in zip(ordered,ordered[1:])),"overlapping local service")
    for t in range(3):
        # Lease sequence numbers are global monotone allocation identifiers.
        running=0
        for l,(cpu,seq,task,debit,total,_) in sorted(retired.items()):
            if task==t:
                running+=debit
                need(total==running,"task cumulative debit mismatch")
        need(running==totals[t],"terminal debit mismatch")
    placements={}
    dispatches={}
    for c,n,d in events(11):
        ep,t,l,f,elr,sp,x21,_=d
        frame_ok(t,c,f,sp)
        if t<3:
            need(l in claimed and claimed[l]==(ep,t,f,elr,x21,sp)
                 and retired[l][0]==c and l not in dispatches,"dispatch descriptor mismatch")
            dispatches[l]=(c,n)
            if ep==epoch:
                placements.setdefault(c,[]).append((t,l))
    need(set(dispatches)==set(claimed),"unobserved dispatch descriptor")
    need(all(placements.get(c) and placements[c][0][0]==wanted
             for c,wanted in enumerate((p0,p1))),"snapshot not applied to both destinations")
    irqs=[]
    for c,n,d in events(4):
        ep,t,f,elr,sp,x21,iar,dispatcher_sp=d
        frame_ok(t,c,f,sp)
        need(inside(5+c,dispatcher_sp),"trap failed to move onto core-private dispatcher stack")
        if iar!=65535:
            irqs.append((c,n,d))
    need(len(irqs)==2 and len(events(7))==2 and len(events(8))==2,"wrong directed SGI count")
    targets={}
    for c,n,d in events(7):
        dst,word,send_epoch=d[:3]
        need(dst==1-c and dst not in targets and word&0xffff==0x8001
             and word>>24==0,"SGIR is not a single Group 1 directed SGI")
        target=(word>>16)&255
        need(target and target&(target-1)==0,"SGIR mask is not one-hot")
        targets[dst]=target
    need(targets[0]!=targets[1],"SGIR targeted the same interface twice")
    for c in range(2):
        ident=only([d for owner,_,d in events(12) if owner==c],"missing/duplicate core identity")
        need(ident[0]&0xff00ffffff==c and ident[1]==4 and ident[2]==freq[c]
             and ident[3]==targets[c],"actual GIC target/MPIDR/EL/frequency mismatch")
        need(sections[c][-1][0]==10,"quiescence is not final local event")
    need([(c,d[0],d[2]) for c,_,d in events(7)]==[(0,1,1),(1,0,0)],"SGIR route/epoch mismatch")
    src_order=[k for k,d in sections[0] if k in (3,4,6,7)][:5]
    need(src_order==[6,4,3,6,7],"source retire and ready/claim/send chronology")
    need([k for k,d in sections[1] if k in (3,4,7)][:4]==[4,3,7,4],
         "opening peer retire, request and idle IRQ chronology")
    for c,n,d in irqs:
        iar=d[6]
        need(iar&1023==1 and (iar>>10)&7==targets[1-c].bit_length()-1,"foreign IAR/source CPU")
        only([v for owner,seq,v in events(8) if owner==c and seq>n and v[0]==iar],
             "missing full-IAR EOI")
    src=only([d for c,_,d in irqs if c==0],"missing source IRQ")
    need(src[1]==0,"source IRQ did not interrupt M")
    in_loop = len(bodies)==1 and bodies[0][0]<=src[3]<bodies[0][1]
    if wire==1 and not in_loop:
        raise Unobserved("interrupted PC outside an unambiguous linked Rust arithmetic loop")
    dest=0 if case else 1
    m_leases=sorted(l for l,x in claimed.items() if x[1]==0)
    need(len(m_leases)==2 and retired[m_leases[0]][0]==0 and retired[m_leases[0]][5]==1
         and retired[m_leases[1]][0]==dest,"unexpected interrupted/resumed ownership")
    second=claimed[m_leases[1]]
    need(second[2:]==(src[2],src[3],src[5],src[4]),"portable frame, ELR, SP or X21 changed")
    source_x21=src[5]
    need(not in_loop or inside(0,source_x21),"saved x21 outside M's stack")
    checkpoints=defaultdict(list);ambiguous=[]
    for c,n,d in events(5):
        task,before,after,index,value,local,pc,sp=d
        need(task in range(3) and index and index%16==0 and inside(task,local)
             and inside(task,sp) and pc%4==0 and 0x80000<=pc<0x100000,"task marker range")
        expected=index*(index+1)//2
        if stream and task == 0:
            expected = stream_reference()[0].get(index)
        if task==2 and index==32:
            expected-=136
        need(value==expected and after in claimed and claimed[after][1]==task,
             "arithmetic or closing checkpoint lease")
        if before!=after:
            need(before in claimed and claimed[before][1]==task and before<after,"unknown opening checkpoint lease")
            ambiguous.append([c,n,task,before,after,index])
        need(dispatches[after][0]==c and dispatches[after][1]<n<retired[after][1],
             "checkpoint outside local lease")
        checkpoints[after].append((index,local,before==after))
    addresses = {local for l in m_leases for _,local,_ in checkpoints[l]}
    need(len(addresses)==1 and (not in_loop or addresses=={source_x21}), "M local state identity")
    for l in m_leases:
        points=checkpoints[l]
        need(points and all(a[0]<b[0] for a,b in zip(points,points[1:])), "M checkpoint progression/cookie")
    need(checkpoints[m_leases[0]][-1][0]<checkpoints[m_leases[1]][0][0]
         and checkpoints[m_leases[1]][-1][0]==final_index,"resumed M progress/final")
    need(any(point[2] for point in checkpoints[m_leases[1]]),"no unambiguous resumed checkpoint")
    finished=sorted((d[0],d[1]) for _,_,d in events(9))
    need(finished==[(0,0x78d22ad1 if stream else 2147516416),(1,528),(2,136),(2,392)],"terminal products")
    for c,n,(task,value,l,ep,*_) in events(9):
        need(l in retired and retired[l][0]==c and retired[l][2]==task
             and dispatches[l][1]<n<retired[l][1],"finish lacks owned lease")
    summaries = {}
    if wire == 2:
        for c,n,d in events(13):
            task,ep,lease,count,h,index,value,reserved = d
            key = task,ep
            need(key in ((0,1),(1,1),(2,0),(2,1)) and key not in summaries,
                 "unknown or duplicate activation summary")
            need(reserved == 0 and (count,h,index,value) == checkpoint_summary(task,ep,workload),
                 "arithmetic checkpoint count/digest/final value mismatch")
            need(n+1<len(sections[c]) and sections[c][n+1]==(9,(task,value,lease,ep,0,0,0,0))
                 and claimed.get(lease,())[:2]==(ep,task), "summary not bound to actual finish")
            summaries[key] = dict(task=task,epoch=ep,lease=lease,count=count,digest=h,index=index,value=value)
        need(len(summaries)==4, "incomplete activation summaries")
    for task,points in [(1,[16,32]),(2,[16,32])]:
        observed=[i for l in sorted(checkpoints) if claimed[l][1]==task for i,_,_ in checkpoints[l]]
        need(observed==points,"missing pinned peer checkpoint")
    need(sorted((c,d[0]) for c,_,d in events(10))==[(0,0),(1,1)],"missing core quiescence")
    for c,n,k,d in rows:
        if k==4:
            external[c].append(("svc" if d[6]==65535 else "irq",d[3]))
        elif k==11:
            external[c].append(("eret",d[4]))
    return dict(regime=CASES[case],lease_intervals=local_intervals,frequency=freq,
                local_ticks={str(c):[account[c,t] for t in range(3)] for c in range(2)},
                cross_core_latency="unobserved",ambiguous_checkpoints=ambiguous,
                checkpoint_coverage=("ordered summary of all checkpoints; sparse lease rows" if wire==2
                                     else "sparse retained rows; intermediate checkpoints not independently replayed"),
                activation_summaries=list(summaries.values()), qualifying_inner_pc=in_loop,
                loop_ranges=bodies, saved_x21=source_x21,
                migration_pc=src[3],destination_cpu=dest,expected_external=external)

def replay(root, ordinal, trusted):
    folder=root/f"first-{ordinal}"
    raw=bounded(folder/"session.json",65536)
    need(digest(raw)==trusted,"untrusted session digest")
    sess=read_json(raw)
    freeze_data=bounded(root/"freeze.json",65536)
    need(digest(freeze_data)==sess["freeze_sha256"],"freeze/session mismatch")
    frozen=read_json(freeze_data)
    need(frozen["case"] in CASES and sess["case"]==frozen["case"] and sess["ordinal"]==ordinal
         and frozen["platform"]==sess["platform"] and sess["platform"] in ("raspi4b-tcg","physical"),"platform/position attribution")
    if sess["platform"]=="raspi4b-tcg":
        need(frozen["flags"]==FLAGS and frozen["positions"]==[1,2] and
         sess["argv"][:1+len(FLAGS)]==[frozen["qemu"],*FLAGS]
         and sess["argv"][1+len(FLAGS)]=="-kernel" and Path(sess["argv"][2+len(FLAGS)]).name=="kernel8.img"
         and sess["argv"][-4:-1]==["-d","int,tid","-D"],"model/observer argv mismatch")
    def checked(path,expected,limit):
        content=bounded(path,limit)
        need(digest(content)==expected,"hash mismatch: "+path.name)
        return content
    files=frozen["hashes"];ident=sess["hashes"]
    elf,image,dis=[checked(root/n,files[n],64*1024*1024) for n in FILES]
    need(set(ident)=={f.name for f in folder.iterdir() if f.is_file() and f.name!="session.json"},
         "capture membership mismatch")
    for name in ident:
        need(Path(name).name==name,"nonlocal artifact name")
        checked(folder/name,ident[name],64*1024*1024)
    workload = frozen.get("workload", BASELINE)
    need(workload in (BASELINE, STREAM), "unknown migration workload")
    need(workload != STREAM or (sess["platform"] == "raspi4b-tcg" and
         frozen["qemu_sha256"] == STOCK_SHA and frozen.get("wire_version") == 2), "stream model contract")
    if workload == STREAM:
        bodies,entries=layout(elf,image,dis,workload)
    if sess["reason"]!="terminal_quiet" or sess["quiet_ns"]<1_000_000_000:
        raise Unobserved("first attempt ended without terminal quiet: "+sess["reason"])
    need(sess["quiet_ns"]<=sess["elapsed_ns"]<=30_000_000_000,"capture ceiling")
    uart=bounded(folder/"uart.bin",8*1024*1024)
    fatal=[line.decode("ascii","replace") for line in uart.splitlines() if b"FAIL" in line]
    if b"PI4 MIG UNOBSERVED M FINISHED BEFORE IRQ" in uart:
        need(fatal==["PI4 MIG FAIL 000000000000001d"] and b"MG END " not in uart,
             "early-finish marker inconsistent with terminal evidence")
        raise Unobserved("M finished before migration IRQ")
    need(not fatal,"guest failure: "+" | ".join(fatal))
    case=CASES.index(frozen["case"])
    wire=frozen.get("wire_version",1)
    need(type(wire) is int and wire in (1,2), "unsupported frozen wire version")
    sections,hz,totals=journal(uart,case,wire)
    if workload == BASELINE:
        bodies,entries=layout(elf,image,dis,workload)
    for cpu in (0,1):
        for kind,d in sections[cpu]:
            if kind==2:
                ep,task=d[:2]
                symbol={(0,0):"pi4_stream_crc32_m_v1" if workload == STREAM else "pi4_migration_m",(0,2):"pi4_migration_p1_open",
                        (1,1):"pi4_migration_p0",(1,2):"pi4_migration_p1"}.get((ep,task))
                if symbol:
                    need(d[4]==entries[symbol],"claimed wrong linked task entry")
            if kind==4 and d[6]==65535:
                off=d[3]-4-0x80000
                need(0<=off<=len(image)-4 and int.from_bytes(image[off:off+4],"little")==0xd4000021,
                     "saved SVC continuation does not follow SVC #1")
            if kind==5:
                off=d[6]-0x80000
                need(0<=off<=len(image)-4 and int.from_bytes(image[off:off+4],"little")&0xffffffe0==0x10000000,
                     "task evidence PC does not identify the linked ADR-self probe")
    result=score(sections,hz,totals,case,bodies,wire,workload)
    if workload == STREAM:
        result["workload"] = workload
    if sess["platform"]=="physical":
        need(sess["card_readback"]==frozen["card_readback"] and sess["cycle_witness"].strip(),
             "physical witness/readback association")
        result.update(position=ordinal,session_sha256=trusted,external="independent physical instruction history unavailable",
                      observation="UART software records only; reported power cycle and board identity require independent corroboration")
        return result
    try:
        external,mapping=observed_threads(folder,ident)
    except Unobserved as exc:
        result.update(observation="unobserved independent instruction history",external_reason=str(exc),
                      position=ordinal,session_sha256=trusted)
        return result
    need(all(external[c]==result["expected_external"][c] for c in (0,1)),
         "per-CPU external exception/ERET stream differs from guest PCs")
    result.update(position=ordinal,session_sha256=trusted,raw_observer_mapping=mapping,
                  observation=("matched finite model position; two first positions required per case"
                               if result["qualifying_inner_pc"] else
                               "actual continuation matched; qualifying inner-loop interruption unobserved"))
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root",type=Path)
    parser.add_argument("ordinal",type=int,choices=(1,2))
    parser.add_argument("session_sha256")
    args=parser.parse_args()
    try:
        result=replay(args.root,args.ordinal,args.session_sha256)
    except Unobserved as exc:
        print(json.dumps(dict(outcome="unobserved",reason=str(exc))))
        return 1
    except (ImportError,OSError,KeyError,IndexError,TypeError,ValueError) as exc:
        print(json.dumps(dict(outcome="invalid or incomplete evidence",reason=str(exc))))
        return 1
    print(json.dumps(result,indent=2))
    return int(not result["qualifying_inner_pc"] or result["observation"]=="unobserved independent instruction history")


if __name__=="__main__":
    sys.exit(main())
