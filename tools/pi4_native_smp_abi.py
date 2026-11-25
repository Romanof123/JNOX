"""Seal an ordinary Pi SMP ABI pair, capture its plain/traced firsts, or read one.

seal PACKAGE NEW_SEAL; capture SEAL SEAL_SHA256 plain|traced;
report SEAL SEAL_SHA256 plain|traced SESSION_SHA256
"""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys

from native_el0_replay import elf_file, symbol
from ordinary_amc_capture import save, sha
from pi4_native_smp_capture import capture
from pi4_native_smp_replay import PAGE_MASK, Ram, one, require

N = 1 << 25
SIG = (0x1111222233334444, 0xaaaabbbbccccdddd)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_range(elf, va, size, flags):
    segments = [s for s in elf.iter_segments() if s["p_type"] == "PT_LOAD" and
                s["p_flags"] == flags and s["p_vaddr"] <= va and
                va + size <= s["p_vaddr"] + s["p_memsz"]]
    return one(segments)


def instructions(path, disassembly):
    elf = elf_file(path.read_bytes())
    result, n = symbol(elf, "smp_abi_result")
    tls, m = symbol(elf, "smp_abi_tls")
    require(n == 128 and m == 16 and tls % 32 == 0 and (tls+m<=result or result+n<=tls), "private ABI objects")
    file_range(elf,result,n,6)
    file_range(elf,tls,m,6)
    start, size = symbol(elf,"smp_abi_work")
    loop = symbol(elf,"smp_abi_loop")[0]
    end = symbol(elf,"smp_abi_loop_end")[0]
    ret = symbol(elf,"smp_abi_restored")[0]
    require(start<loop<end<=ret==start+size-4 and end-loop==28, "linked leaf ranges")
    seg=file_range(elf,start,size,5)
    data=seg.data()[start-seg["p_vaddr"]:start-seg["p_vaddr"]+size]
    ops={}
    for addr, word, text in re.findall(r"(?m)^\s*([a-f0-9]+):\s+([a-f0-9]{8})\s+([^\n]+)",disassembly):
        pc=int(addr,16)
        if start<=pc<start+size:
            require(pc not in ops and struct.unpack_from("<I",data,pc-start)[0]==int(word,16),"listing differs from linked instructions")
            ops[pc]=re.sub(r"\s+","",text.split("//")[0]).lower()
    require(set(ops)==set(range(start,start+size,4)) and ops[ret]=="ret","incomplete leaf listing")
    actual=[ops[pc] for pc in range(loop,end,4)]
    require(actual[:3]==["addx6,x6,x5","fmovd0,x5","addd1,d1,d0"] and
            actual[3] in ("addx5,x5,#0x2","addx5,x5,#2") and actual[4] in
            ("addx4,x4,#0x1","addx4,x4,#1") and actual[5]=="cmpx4,x7" and
            re.fullmatch(r"b\.ne(?:0x)?"+format(loop,"x")+r"<smp_abi_loop>",actual[6]),
            "independent scalar/SIMD live-loop shape")
    require({"movx19,x3","msrtpidr_el0,x2","msrfpcr,x9","msrfpsr,x9"} <=
            {op for pc,op in ops.items() if pc<loop}, "linked sentinel setup")
    require({"mrsx9,tpidr_el0","mrsx9,tpidrro_el0","mrsx10,fpcr","mrsx9,fpsr",
             "msrtpidr_el0,x9","msrfpcr,x10","msrfpsr,x11"} <=
            {op for pc,op in ops.items() if end<=pc<=ret}, "linked readback and restoration")
    require(not any(s.startswith("msrtpidrro_el0,") for s in ops.values()), "EL0 read-only TLS write")
    return dict(result=result,tls=tls,start=start,loop=loop,end=end,ret=ret,leaf_sha256=digest(data))


def deployment(path):
    m=json.loads((path/"manifest.json").read_bytes())
    require(m["policy"]=="smp-partitioned" and m["board"]=="pi4" and len(m["slots"])==2,"ABI pair deployment")
    for i,s in enumerate(m["slots"]):
        require((s["slot"],s["cpu_mask"],s["priority"],s["release_cell"],s["argv"])==
                (i,1<<i,2,0,["/bin/app"]),"ABI placement")
        require(Path(s["elf"]).name==s["elf"] and sha(path/s["elf"])==s["elf_sha256"],"input ELF identity")
    require(sha(path/"kernel.elf")==m["kernel_elf_sha256"] and sha(path/"kernel8.img")==m["kernel8_img_sha256"],"kernel identity")
    return m


def seal(a):
    pkg=a.package.resolve()
    m=deployment(pkg)
    qemu=Path(shutil.which(a.qemu) or a.qemu).resolve(strict=True)
    a.seal.mkdir()
    apps=[]
    for i,s in enumerate(m["slots"]):
        listing=subprocess.check_output([a.objdump,"-d",str(pkg/s["elf"])])
        (a.seal/f"slot{i}.dis").write_bytes(listing)
        apps.append(instructions(pkg/s["elf"],listing.decode()))
    require(apps[0]["tls"]!=apps[1]["tls"]+8,"distinct effective TLS VAs")
    save(a.seal/"seal.json",dict(package=str(pkg),manifest=sha(pkg/"manifest.json"),apps=apps,
         listings=[sha(a.seal/f"slot{i}.dis") for i in range(2)],qemu=str(qemu),qemu_sha256=sha(qemu),
         timeout=120,positions=["plain","traced"]))
    print(sha(a.seal/"seal.json"))


def memory(ram,ttbr,va,n,write):
    chunks=[]
    physical=[]
    while n:
        length=min(n,4096-va%4096)
        pa,d=ram.walk(ttbr,va)
        require(pa and d&64 and d&(1<<11) and bool(d&128)!=write and
                bool(d&(1<<54))==write and d&(1<<53),"user mapping permissions")
        off=pa-ram.base
        require(0<=off<=len(ram.data)-length,"user backing outside capture")
        chunks.append(ram.data[off:off+length])
        physical.append(pa & PAGE_MASK)
        n-=length
        va+=length
    return b"".join(chunks),physical


def trace_check(raw, entries, vectors):
    header=re.compile(r"Taking exception (2 \[SVC\]|5 \[IRQ\]) on CPU ([01])$")
    records=[[],[]]
    returns=[]
    lines=raw.splitlines()
    for i,line in enumerate(lines):
        if line.startswith("Taking exception"):
            m=header.fullmatch(line)
            require(m is not None and i+5<len(lines),"external exception framing")
            body="\n".join(lines[i+1:i+6])
            v=re.fullmatch(r"\.\.\.from EL([01]) to EL1\n\.\.\.with ESR 0x[\da-f]+/0x[\da-f]+\n"
                           r"\.\.\.with SPSR (0x[\da-f]+)\n\.\.\.with ELR (0x[\da-f]+)\n"
                           r"\.\.\.to EL1 PC (0x[\da-f]+) PSTATE (0x[\da-f]+)",body)
            require(v is not None,"truncated CPU-indexed exception")
            cpu=int(m[2])
            cause=1 if m[1]=="5 [IRQ]" else 2
            origin=int(v[1])
            require(int(v[4],16)==vectors+128*(5 if origin else 9 if cause==1 else 8) and
                    int(v[5],16)&0x8f==0x85,"exception vector")
            records[cpu].append((i,cause,origin,int(v[3],16),int(v[2],16)))
        else:
            m=re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL0 PC (0x[\da-f]+)(?: on CPU ([01]))?",line)
            if m:
                returns.append((i,int(m[1],16),int(m[2]) if m[2] else None))
    windows=[]
    for cpu,rows in enumerate(entries):
        require([e[1:] for e in records[cpu]]==[(r[2],int(r[6]==2),r[30],r[31]) for r,f,g in rows],
                "per-CPU exception cause/origin/PC/PSTATE stream")
        for i,(r,f,g) in enumerate(rows):
            if r[7]!=2:
                require(i+1<len(rows),"nonterminal external return")
                windows.append((cpu,i,g[32],records[cpu][i][0],records[cpu][i+1][0]))
    # Stock QEMU may omit CPU on ERET. Only attribute a return when its PC
    # and per-CPU exception brackets leave exactly one possible owner.
    certain=set()
    graph=[]
    for pos,pc,tag in returns:
        candidates=[(c,i) for c,i,p,l,h in windows if p==pc and l<pos<h and tag in (None,c)]
        require(candidates,"unmatched external user return")
        if len(candidates)==1:
            certain.add(candidates[0])
        graph.append(candidates)
    # Every return must cover a different prepared transition. A duplicate
    # return in one core's window cannot substitute for a missing peer return.
    matched={}
    def assign(index, visited):
        for candidate in graph[index]:
            if candidate in visited:
                continue
            visited.add(candidate)
            if candidate not in matched or assign(matched[candidate], visited):
                matched[candidate]=index
                return True
        return False
    require(len(returns)==len(windows) and all(assign(i,set()) for i in range(len(returns))),
            "inconsistent external user return coverage")
    return certain


def report(a, frozen):
    root=a.seal/a.position
    require(sha(root/"session.json")==a.session_sha,"session digest")
    sess=json.loads((root/"session.json").read_bytes())
    require(set(sess["files"])=={f.name for f in root.iterdir() if f.is_file()}-{"session.json"},"unsealed capture files")
    for n,h in sess["files"].items():
        require(Path(n).name==n and sha(root/n)==h,"artifact identity")
    require(sess["reason"]=="terminal",sess["reason"])
    m=deployment(root)
    require(sha(root/"manifest.json")==frozen["manifest"],"frozen manifest")
    launch=json.loads((root/"launch.json").read_bytes())
    trace=a.position=="traced"
    require(launch["trace"]==trace and launch["timeout"]==120 and launch["qemu_sha256"]==frozen["qemu_sha256"],"launch identity")
    cmd=[frozen["qemu"],"-M","raspi4b","-accel","tcg","-display","none","-serial","file:"+str(root.resolve()/"uart.bin"),
         "-qmp","stdio","-kernel",str(root.resolve()/"kernel8.img")]
    require(launch["argv"]==cmd+(["-d","int","-D",str(root.resolve()/"exceptions.log")] if trace else []),"stock launch command")
    apps=frozen["apps"]
    kernel=elf_file((root/"kernel.elf").read_bytes())
    ram=Ram(root,launch["ram_base"])
    require((ram.base,ram.base+len(ram.data))==(symbol(kernel,"__native_payload_start")[0],symbol(kernel,"__stack_top")[0]),"RAM extent")
    uart=(root/"uart.bin").read_text("ascii")
    def rows(tag):
        return [tuple(map(int,line.split())) for line in re.findall(rf"(?m)^NATIVE SMP {tag} ([0-9 ]+)$",uart)]
    require(one(rows("END"))==(1,3,1,1),"terminal rendezvous")
    ver,hz,q,epoch,n,idle=one(rows("CONFIG"))
    require((ver,n,idle)==(1,2,2) and q==hz//50 and q>0,"SMP clock")
    before,other,after=one(rows("CAL"))
    require(before<=after and other>0,"cross-core clock calibration")
    lo,hi=before-other,after-other
    exits=sorted((int(i),int(v)) for i,v in re.findall(r"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)$",uart))
    require(exits==[(0,12),(1,77)],"signed exits")
    tasks=sorted(rows("TASK"))
    require(len(tasks)==2,"tasks")
    spaces=one([s for s in kernel.get_section_by_name(".symtab").iter_symbols()
                if "native_el0_mmu" in s.name and "6SPACES" in s.name])
    roots=[((cpu+1)<<48)|(spaces["st_value"]+cpu*(spaces["st_size"]//2)) for cpu in range(2)]
    require(spaces["st_size"]>0 and spaces["st_size"]%8192==0,"linked MMU space array")
    ev,esize=symbol(kernel,"native_smp_events")
    fr,fsize=symbol(kernel,"native_smp_frames")
    require((esize,fsize)==(2*256*37*8,2*256*2*816),"linked journal buffers")
    journals=sorted(rows("JOURNAL"))
    require(len(journals)==2,"both CPU journals")
    stacks=rows("STACK")
    require(len(stacks)==2 and len({s[k] for s in stacks for k in (0,3)})==4,"CPU stack guards")
    for guard,bottom,top in [s[i:i+3] for s in stacks for i in (0,3)]:
        for ttbr in roots:
            require(bottom==guard+4096 and top==bottom+16384 and not ram.walk(ttbr,guard)[1],"stack guard")
            for addr in range(bottom,top,4096):
                pa,desc=ram.walk(ttbr,addr)
                require(pa==addr and not desc&64 and desc&(1<<53),"privileged stack")
    entries=[[],[]]
    samples=[[],[]]
    backing=[set(),set()]
    output=[]
    released=0
    for cpu,s in enumerate(m["slots"]):
        app=apps[cpu]
        elf=elf_file((root/s["elf"]).read_bytes())
        require(instructions(root/s["elf"],(root/(s["elf"]+".dis")).read_text())==app,"linked ABI code")
        ttbr=roots[cpu]
        require(tasks[cpu][:4]==(cpu,cpu,2,0) and tasks[cpu][5]==ttbr,"assigned task root")
        sig=SIG[cpu]
        lane=(cpu+1)*256+1
        tls=app["tls"]+8*cpu
        fpcr=(cpu+1)<<22
        fpsr=(1-cpu)<<27
        total=N*(N+cpu)
        product,pages=memory(ram,ttbr,app["result"],128,True)
        backing[cpu].update(pages)
        words=struct.unpack("<16Q",product)
        require(words==(3,cpu,N,total,total,sig,lane,lane+1,0,tls,0,0,fpcr,fpsr,sig,0),"final user register readbacks and scalar/SIMD products")
        cell,pages=memory(ram,ttbr,app["tls"],16,True)
        backing[cpu].update(pages)
        require(struct.unpack("<2Q",cell)==((sig,0) if cpu==0 else (0,sig)),"private TLS record")
        code,pages=memory(ram,ttbr,app["start"],app["ret"]+4-app["start"],False)
        require(digest(code)==app["leaf_sha256"],"private executable bytes")
        backing[cpu].update(pages)
        c,addr,used,width,lost,frames,fs=journals[cpu]
        require((c,addr,width,frames,fs)==(cpu,ev+cpu*esize//2,37,fr+cpu*fsize//2,816) and not lost and 0<used<=256,"linked journals/loss")
        owner,prev,service=2,0,0
        exited=False
        for j in range(used):
            r=ram.words(addr+296*j,37)
            f=ram.words(frames+1632*j,102)
            g=ram.words(frames+1632*j+816,102)
            src,dst=r[6:8]
            require(r[:2]==(cpu,j) and src==owner and src in (cpu,2) and dst in (cpu,2)
                    and prev<=r[4]<=r[5] and stacks[cpu][1]<=r[29]<=stacks[cpu][2],"journal owner/timing/stack")
            require(not released&r[14] and (not r[14] or cpu==0),"duplicate/remote release")
            released|=r[14]
            require(r[34]==r[11] and r[35]&3==1,"rearmed physical comparator")
            if src==cpu:
                require(not exited and r[21]==ttbr and prev<=r[8]<r[4] and r[9]==r[4]-r[8],"actual user interval")
                service+=r[9]
                require(r[28]==service and f[32:34]==r[30:32] and f[33]&31==0 and
                        0x01001000<=f[31]<=0x01011000 and f[31]%16==0,"user exception frame")
                _,pages=memory(ram,ttbr,f[31]-8,8,True)
                backing[cpu].update(pages)
                if r[2]==2:
                    require((r[3],f[8],r[23],f[0])==(1,1,exits[cpu][1],exits[cpu][1]) and dst==2,"ordinary exit SVC")
                    require(f[34:38]==(0,0,0,0),"restored TLS and FP environment at exit")
                    exited=True
                else:
                    require(r[2]==1 and dst==cpu and r[22]==ttbr and f==g,"complete interrupted and prepared same-owner frame")
                    if app["loop"]<=f[32]<app["end"]:
                        if r[3]!=30:
                            require(r[3]&1023!=30,"malformed timer acknowledgement")
                        else:
                            require(r[32]==r[10] and r[33]&7==5,"asserted original PPI30 comparator")
                            require(f[0:4]==(cpu,app["result"],tls,sig) and f[19]==sig and f[70:72]==(lane,lane+1) and f[34:38]==(tls,0,fpcr,fpsr),"live task sentinels")
                            step=(f[32]-app["loop"])//4
                            done=f[4]-int(step>=5)
                            require(0<=done<N and f[4]==done+int(step>=5) and f[7]==N,"loop counter/limit")
                            value=2*done+cpu+1
                            partial=done*(done+cpu)
                            require(f[5]==value+2*int(step>=4) and f[6]==partial+value*int(step>=1)
                                    and f[40]==partial+value*int(step>=3),"interrupted scalar and SIMD liveness")
                            samples[cpu].append((j,r[8],r[4],done, f[32]))
            if dst==cpu:
                require(not exited and r[22]==ttbr,"selected private root")
            entries[cpu].append((r,f,g))
            owner,prev=dst,r[5]
        require(exited and service==tasks[cpu][4],"independent terminal service")
        output.append(dict(slot=cpu,root=ttbr,tls=tls,count=N,scalar=words[3],vector=words[4],exit=exits[cpu][1],service_ticks=service))
    require(released==3 and not backing[0]&backing[1],"releases or private backing alias")
    certain=trace_check((root/"exceptions.log").read_text(),entries,symbol(kernel,"pi4_native_smp_vectors")[0]) if trace else None
    unknown=[]
    pairs=[[],[]]
    for cpu,ss in enumerate(samples):
        resumed=[]
        for j,start,entry,done,pc in ss:
            if j+1>=len(entries[cpu]):
                continue
            later=entries[cpu][j+1][0]
            if later[6]==cpu and later[8]>=entry and later[4]>later[8] and (certain is None or (cpu,j) in certain):
                if any(y[3]>done for y in ss if y[0]>j) or done<N-1:
                    resumed.append(j)
        if not resumed:
            unknown.append(f"CPU{cpu}: missing unambiguous in-loop interruption and real return")
        output[cpu].update(saved_ppis=len(ss),resumed_ppis=resumed,private_pages=sorted(backing[cpu]))
        pairs[cpu]=[(x,y) for x,y in zip(ss,ss[1:]) if y[0]==x[0]+1 and x[3]<y[3] and x[0] in resumed]
    overlap=None
    for pair0,pair1 in itertools.product(*pairs):
        for c,p,d,t in ((0,pair0,1,pair1),(1,pair1,0,pair0)):
            x,y=p
            v,w=t
            shift=lambda cpu:(lo,hi) if cpu else (0,0)
            cl,ch=shift(c)
            dl,dh=shift(d)
            if max(x[2],y[1])+ch<v[2]+dl and max(v[2],w[1])+dh<y[2]+cl and y[2]+ch<w[2]+dl:
                overlap=dict(cpu_order=[c,d,c,d],samples=[x,v,y,w],inner_ticks=y[2]+cl-(v[2]+dh))
                break
        if overlap:
            break
    if not overlap:
        unknown.append("no unambiguous interleaved advancing work observations")
    return dict(position=a.position,session=a.session_sha,products=output,counter_offset=[lo,hi],
                overlap=overlap,unknown=unknown,status="incomplete" if unknown else "observations-met")


def main():
    p=argparse.ArgumentParser(description=__doc__)
    sub=p.add_subparsers(dest="action",required=True)
    s=sub.add_parser("seal")
    s.add_argument("package",type=Path)
    s.add_argument("seal",type=Path)
    s.add_argument("--qemu",default="qemu-system-aarch64")
    s.add_argument("--objdump",default="aarch64-linux-gnu-objdump")
    for name in ("capture","report"):
        c=sub.add_parser(name)
        c.add_argument("seal",type=Path)
        c.add_argument("seal_sha")
        c.add_argument("position",choices=("plain","traced"))
        if name=="report":
            c.add_argument("session_sha")
        else:
            c.add_argument("--nm",default="aarch64-linux-gnu-nm")
            c.add_argument("--objdump",default="aarch64-linux-gnu-objdump")
    a=p.parse_args()
    if a.action=="seal":
        seal(a)
        return 0
    require(sha(a.seal/"seal.json")==a.seal_sha,"ABI control seal")
    frozen=json.loads((a.seal/"seal.json").read_bytes())
    for i,h in enumerate(frozen["listings"]):
        require(sha(a.seal/f"slot{i}.dis")==h,"frozen linked disassembly")
    if a.action=="capture":
        a.package=Path(frozen["package"])
        require(sha(a.package/"manifest.json")==frozen["manifest"],"sealed package manifest")
        deployment(a.package)
        a.qemu=frozen["qemu"]
        require(sha(Path(a.qemu))==frozen["qemu_sha256"],"sealed emulator")
        a.trace=a.position=="traced"
        if a.trace:
            require((a.seal/"plain/session.json").is_file(),"plain first must be recorded before traced first")
        a.out=a.seal/a.position
        return capture(a)
    result=report(a,frozen)
    print(json.dumps(result,indent=2))
    return 2 if result["unknown"] else 0


if __name__=="__main__":
    try:
        sys.exit(main())
    except (ValueError,OSError,KeyError,IndexError,struct.error) as exc:
        sys.exit("Invalid or incomplete Pi ABI evidence: "+str(exc))
