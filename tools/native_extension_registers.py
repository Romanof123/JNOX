"""Offline passive timer/register bindings for the explicitly instrumented native extension."""
import io
import re
import struct
import subprocess
import tarfile

from native_el0_replay import symbol
from native_extension_replay import NONE, check

PROFILE = {"abi": 1, "capacity": 512}
HISTORY = "e49c2bbe1f1cb954f2d58db24fb1e2f0b575478aaa2c9c2abe13cfa8b36898ac"
ARM = "generation row cell kind target before cval ctl after".split()
ENTRY = "ordinal row generation cell source iar old_compare prefix spsr elr sp".split()


def deployment_comparison(args, images, reference, apps):
    from native_extension_equivalence import ARCHIVE, Image, decoded, sha
    from native_extension_source_equivalence import LABELS, layout, package, read_json
    archive=ARCHIVE.with_name("native-extension-stopped-progress-first-raw.tar.gz").read_bytes()
    check(sha(archive)==HISTORY,"sealed historical disabled cohort")
    output={}
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for label in LABELS:
            dirname="accepted" if (args.position,label)==("a","ordinary") else args.position+"-"+label
            historic=tar.extractfile("./"+dirname+"-plain/kernel.elf").read()
            disabled,dh=package(args.disabled_cohort/label,args.position,label,reference,apps)
            check(read_json(args.disabled_cohort/label/"manifest.json").get("extension_register_observations") is None,
                  "disabled comparison profile")
            check(layout(disabled,label!="ordinary",False)==layout(Image(historic),label!="ordinary",False)
                  and disabled.loads==Image(historic).loads, "disabled build differs from original loaded bytes/layout")
            enabled=images[label]
            names=("jnox_extension_register_arm","jnox_extension_reg_arms","jnox_extension_reg_entries",
                   "jnox_extension_reg_generation","jnox_extension_reg_scratch")
            check(all(n in enabled.names and n not in disabled.names for n in names),"diagnostic-only linked code/storage")
            path=args.cohort/label/"kernel.elf"
            listing=subprocess.check_output(["aarch64-linux-gnu-objdump","-d",str(path)])
            (args.out/(label+"-diagnostic.dis")).write_bytes(listing)
            addr,n,_=next(r for r in enabled.ranges if r[2]==names[0])
            ops=[]
            by_pc={}
            for pc,text in re.findall(rb"(?m)^\s*([0-9a-f]+):\s+[0-9a-f]{8}\s+([^\r\n]+)",listing):
                pc=int(pc,16)
                op=re.sub(rb"\s+",b"",text.split(b"//")[0]).lower().decode()
                by_pc[pc]=op
                if addr<=pc<addr+n:
                    ops.append(op)
            stream="\n".join(ops)
            tail=r"\nisb\nmrsx\d+,cntp_cval_el0\nmrsx\d+,cntp_ctl_el0\nisb\nmrsx\d+,cntpct_el0"
            check(re.search(r"isb\nmrsx\d+,cntpct_el0\nmsrcntp_cval_el0,x\d+\nmsrcntp_ctl_el0,x\d+"+tail,stream),
                  "linked bracketed enabled CVAL/CTL immediate hardware readback")
            check(re.search(r"isb\nmrsx\d+,cntpct_el0\nmsrcntp_ctl_el0,xzr"+tail,stream),
                  "linked bracketed terminal disable/readback")
            vector,=enabled.names["native_el0_vectors"]
            prefixes=[]
            for offset in (5*128,9*128):
                pc=vector+offset
                branch=decoded(enabled.word(pc),pc)
                check(branch and branch[0]==0x14000000,"linked IRQ entry branch")
                start=branch[1]
                seq=[by_pc.get(start+4*i,"") for i in range(16)]
                check(seq[0]=="stpx9,x10,[sp,#-16]!" and seq[1:3]==["isb","mrsx10,cntpct_el0"]
                      and seq[5:10]==["strx10,[x9]","mrsx10,spsr_el1","strx10,[x9,#8]",
                                      "mrsx10,elr_el1","strx10,[x9,#16]"]
                      and seq[10] in ("addx10,sp,#0x10","addx10,sp,#16") and seq[11]=="strx10,[x9,#24]"
                      and seq[14:]==["strx10,[x9,#32]","ldpx9,x10,[sp],#16"],"linked pre-abort vector prefix")
                prefixes.append(dict(vector_pc=pc,prefix_pc=start,counter_pc=start+8,
                                     spsr_pc=start+24,elr_pc=start+32,end=start+64))
            output[label]=dict(historical=sha(historic),disabled=dh,disassembly_sha256=sha(listing),
                               sampler=[addr,n],vector_prefixes=prefixes,diagnostic_symbols={k:sorted(enabled.names[k]) for k in names},
                               changed_allocated_sections=[name for name,s in enabled.sections.items()
                                 if name not in disabled.sections or (s["sh_addr"],s["sh_size"],s["sh_flags"])!=
                                 tuple(disabled.sections[name][k] for k in ("sh_addr","sh_size","sh_flags"))
                                 or (s["sh_type"]!="SHT_NOBITS" and s.data()!=disabled.sections[name].data())])
    return dict(archive=HISTORY,variants=output,timing="enabled measurements required")


def observations(manifest,uart,elf,mem,config,rows,loss,witness=(),loop=None,traced=False):
    lines=[line for line in uart.splitlines(keepends=True) if line.startswith(b"NATIVE REG ")]
    profile=manifest.get("extension_register_observations")
    if profile is None:
        check(not lines,"unexpected diagnostic output in ordinary reference")
        return None
    check(profile==PROFILE,"register observation manifest ABI")
    check(uart.find(b"NATIVE EXTENSION DROPPED ") < uart.find(b"NATIVE REG CONFIG ") <
          uart.find(b"NATIVE REG END ") < uart.find(b"NATIVE EL0 EXIT PID "), "terminal register framing/order")
    check(config[2:4]==[62500000,6250000],"diagnostic frequency/quantum")
    records=[]
    for line in lines:
        check(re.fullmatch(rb"NATIVE REG [A-Z]+(?: [0-9]{1,20})+\r?\n",line),"truncated register record")
        _,_,tag,*fields=line.split()
        vals=tuple(map(int,fields))
        check(all(x<=NONE for x in vals),"register record u64 overflow")
        records.append((tag,vals))
    check(records and records[0]==(b"CONFIG",(1,512)) and records[-1][0]==b"END" and
          len(records[-1][1])==6,"register header/footer")
    at,ac,al,et,ec,el=records[-1][1]
    check(ac==min(at,512) and al==at-ac and ec==min(et,512) and el==et-ec and
          len(records)==2+ac+ec,"bounded record counts")
    result={"arms":[],"entries":[],"issues":[],"unknown":[],"loss":[al,el],"event_loss":loss}
    if al or el or loss:
        result["unknown"].append("bounded register/event record loss")
    for tag,cols,total,name in ((b"ARM",ARM,ac,"arms"),(b"ENTRY",ENTRY,ec,"entries")):
        found=[v for t,v in records[1:-1] if t==tag]
        check(len(found)==total and all(len(v)==len(cols) for v in found),"register row width/count")
        addr,size=symbol(elf,"jnox_extension_reg_"+name)
        check(size==512*len(cols)*8,"linked register buffer extent")
        check(list(struct.iter_unpack("<"+"Q"*len(cols),mem.read(addr,total*len(cols)*8)))==found,
              "UART/privileged RAM "+name)
        result[name]=[dict(zip(cols,v)) for v in found]
    check([t for t,_ in records[1:-1]] == [b"ARM"]*ac+[b"ENTRY"]*ec,"register dump order")
    count_addr,count_size=symbol(elf,"jnox_extension_reg_generation")
    check(count_size==8 and struct.unpack("<Q",mem.read(count_addr,8))[0]==at,"terminal generation vs hardware records")
    arms=result["arms"]
    entries=result["entries"]
    if not loss:
        check(at==len(rows)+1 and et==sum(r["cause"]==1 for r in rows),"exact ARM/IRQ event coverage")
    def observed(ok,msg):
        if not ok:
            result["issues"].append(msg)
    epoch=config[1]
    for idx,arm in enumerate(arms):
        check((arm["generation"],arm["row"])==(idx+1,idx-1 if idx else NONE),"arm generation/row sequence")
        if idx>len(rows):
            continue
        row=rows[idx-1] if idx else None
        cell=row["cell"] if row else NONE
        target=row["compare"] if row else epoch
        if row and row["dst"]==4:
            kind=16
        else:
            nominal=epoch if cell==NONE else row["cutoff"] if row["evaluating"] else row["end"]
            wake=[row[f"sleep{i}"] for i in range(3) if row[f"sleep{i}"]] if row else []
            check(target==min([nominal,*wake]),"actual compare selection")
            kind=(1 if cell==NONE else 2 if row["evaluating"] else 4) if target==nominal else 0
            if target in wake:
                kind|=8
        check((arm["cell"],arm["target"],arm["kind"])==(cell,target,kind),"arm target/cause/cell")
        check(arm["before"]<=arm["after"] and (not row or row["now"]<=arm["before"]) and
              (idx==len(rows) or arm["after"]<=rows[idx]["entry"]), "arm chronological bracket")
        observed(arm["ctl"]&3==(0 if kind==16 else 1),f"arm {idx+1}: ENABLE/IMASK mismatch")
        if kind!=16:
            observed(arm["cval"]==target,f"arm {idx+1}: hardware CVAL differs from chosen target")
            arm["target_minus_after"]=target-arm["after"]
            arm["status_observation"]="pre-target" if arm["after"]<target else "post-target" if arm["before"]>=target else "bracket spans target"
            if arm["after"]<target:
                observed(not arm["ctl"]&4,f"arm {idx+1}: ISTATUS asserted before target")
    irqrows=[i for i,r in enumerate(rows) if r["cause"]==1]
    stack=one_stack(elf,"9EVALUATOR")
    for seq,entry in enumerate(entries,1):
        check(entry["ordinal"]==seq,"entry ordinal")
        if seq>len(irqrows):
            check(loss,"extra IRQ entry")
            continue
        idx=irqrows[seq-1]
        r=rows[idx]
        check((entry["row"],entry["generation"],entry["cell"],entry["iar"],entry["old_compare"])==
              (idx,idx+1,r["old_cell"],r["op"],r["old_compare"]),"IRQ generation/row/old compare")
        observed(entry["iar"]==30,f"IRQ row {idx}: unexpected raw PPI30 IAR")
        check(entry["prefix"]<=r["entry"]<=r["now"],"vector sample vs original late handler stamp")
        if idx<len(arms):
            check(arms[idx]["after"]<=entry["prefix"] and arms[idx]["target"]==entry["old_compare"],"outstanding arm identity")
        sp=entry["sp"]
        cls=1 if r["src"]<3 else 2 if stack[0]<sp<=stack[1] else 3
        observed(entry["elr"]==r["src_pc"] and entry["source"]==cls and sp%16==0 and sp!=0,
                 f"IRQ row {idx}: saved ELR/SP/source mismatch")
        observed(entry["spsr"]&0x8f==(0 if cls==1 else 5),f"IRQ row {idx}: saved mode/IRQ mask")
        entry["prefix_to_handler_ticks"]=r["entry"]-entry["prefix"]
        if r["canceled"]:
            observed(cls==2 and loop is not None and entry["elr"]==loop and r["old_eval"] and
                     entry["old_compare"]==r["cutoff"]==r["cval"] and r["ctl"]&7==5 and entry["prefix"]>=r["cutoff"],
                     f"cell {r['cell']}: stopped transaction/direct cutoff")
            entry["prefix_lag_ticks"]=entry["prefix"]-r["cutoff"]
            samples=[s for s in witness if s["cell"]==r["cell"]]
            if samples:
                x=samples[0]
                # Compare defined NZCV, DAIF and mode bits, retaining both entire read operands.
                observed((x["pre_abort_sp"],x["elr"])==(sp,entry["elr"]) and
                         (x["live_pstate"]^entry["spsr"]) & 0xf00003cf==0,
                         f"cell {r['cell']}: direct saved state vs external pre-abort dump")
            elif traced:
                result["unknown"].append(f"cell {r['cell']}: external loop state unavailable")
    result["cell_timing"]=[dict(cell=r["cell"],start=r["start"],D=r["cutoff"],end=r["end"],
                                release_entry_ticks=r["entry"]-r["start"])
                           for r in rows if r["cell"]!=NONE and r["cell"]!=r["old_cell"]]
    return result


def trace_vectors(raw,elf,rows):
    base,size=symbol(elf,"native_el0_vectors")
    check(base%2048==0 and size==2048,"linked exception vectors")
    starts=list(re.finditer(rb"(?m)^Taking exception ",raw))
    expected=[r for r in rows if r["cause"] in (1,2)]
    check(len(starts)==len(expected),"diagnostic exception count")
    for i,(m,r) in enumerate(zip(starts,expected)):
        block=raw[m.start():starts[i+1].start() if i+1<len(starts) else len(raw)]
        head=re.match(rb"Taking exception (5 \[IRQ\]|2 \[SVC\])(?: on CPU 0)?\r?\n",block)
        origin=re.findall(rb"(?m)^\.\.\.from EL([01]) to EL1\r?$",block)
        dest=re.findall(rb"(?m)^\.\.\.to EL1 PC (0x[0-9a-fA-F]+) PSTATE (0x[0-9a-fA-F]+)\r?$",block)
        pc=re.findall(rb"(?m)^\.\.\.with ELR (0x[0-9a-fA-F]+)\r?$",block)
        src=int(r["src"]==3)
        check(head and head[1]==(b"5 [IRQ]" if r["cause"]==1 else b"2 [SVC]") and
              origin==[str(src).encode()] and len(pc)==len(dest)==1 and int(pc[0],16)==r["src_pc"] and
              int(dest[0][0],16)==base+128*(5 if src else 9 if r["cause"]==1 else 8) and
              int(dest[0][1],16)&0x8f==0x85,"external exception kind, ELR, vector, PSTATE or origin")


def one_stack(elf,name):
    hits=[(s["st_value"],s["st_value"]+s["st_size"]) for s in elf.get_section_by_name(".symtab").iter_symbols()
          if "native_el0" in s.name and name in s.name and s["st_size"]==16384]
    check(len(hits)==1,"linked evaluator stack")
    return hits[0]
