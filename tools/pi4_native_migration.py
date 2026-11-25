"""Capture and read a frozen Pi 4 migration position.

capture CASE PACKAGE NEW_DIR plain|traced --manifest-sha SHA --odd-sha SHA
    [--even-sha SHA] [--calendar-sha SHA]
report CASE DIR plain|traced --manifest-sha SHA --odd-sha SHA
    [--even-sha SHA] [--calendar-sha SHA] --session-sha SHA

Supply independently recorded #355 application hashes. An existing output directory
is never reused. The calendar substitution has only a plain position.
"""
import argparse
import itertools
import json
import re
import shutil
import struct
import sys
from pathlib import Path

from native_el0_replay import elf_file, symbol
from ordinary_amc_capture import sha
from pi4_native_smp_abi import SIG, N, digest, instructions, memory
from pi4_native_smp_capture import capture
from pi4_native_smp_replay import Ram, one, require
from pi4_native_smp_stage1 import APPS, MODEL

CASES = ("global2","global3","confined","within","calendar")


def config(a, root):
    require(sha(root/"manifest.json")==a.manifest_sha,"manifest identity")
    m=json.loads((root/"manifest.json").read_bytes())
    n=2 if a.case in ("global2","calendar") else 3
    cl=a.case in ("confined","within")
    masks=[(1<<n)-1,1]+([4 if a.case=="within" else 2] if n==3 else [])
    clusters=[0,0,int(a.case=="within")][:n]
    hashes=[a.odd_sha,a.calendar_sha if a.case=="calendar" else APPS[0],a.even_sha][:n]
    effective=[mask & ([3,4][clusters[i]] if cl else (1<<n)-1) for i,mask in enumerate(masks)]
    require(m["board"]=="pi4" and m["policy"]==("smp-clustered" if cl else "smp-global")
            and m["online_cpus"]==list(range(n)) and m["release_grid_hz"]==50 and len(m["slots"])==n
            and (not cl or m["cluster_masks"]==[3,4]),"deployment")
    for i,s in enumerate(m["slots"]):
        require((s["slot"],s["cpu_mask"],s["effective_cpu_mask"],s["priority"],s["release_cell"],
                 s.get("cluster"),s["argv"])==(i,masks[i],effective[i],[2,5,4][i],int(i==1),
                 clusters[i] if cl else None,["/bin/app"]),"ordered job descriptors")
        require(Path(s["elf"]).name==s["elf"] and sha(root/s["elf"])==s["elf_sha256"]==hashes[i],"frozen client")
    require(sha(root/"kernel.elf")==m["kernel_elf_sha256"] and sha(root/"kernel8.img")==m["kernel8_img_sha256"],"boot files")
    require(a.case!="calendar" or a.position=="plain","calendar first is plain")
    return m


def plan(old, active, slots):
    n=len(old)
    ranked=sorted(active,key=lambda p:-slots[p]["priority"])
    candidates=[]
    for m in itertools.product([*active,n],repeat=n):
        jobs=[p for p in m if p!=n]
        if len(jobs)!=len(set(jobs)) or any(p!=n and not slots[p]["effective_cpu_mask"]&(1<<c) for c,p in enumerate(m)):
            continue
        key=(tuple(p not in jobs for p in ranked),sum(x!=n and x!=y for x,y in zip(old,m)),m)
        candidates.append(key)
    return min(candidates)[2]


def attribution(root,groups,vec,n):
    raw=(root/"exceptions.log").read_text("ascii")
    lines=raw.splitlines()
    entries=[[] for _ in groups]
    returns=[]
    for i,line in enumerate(lines):
        if line.startswith("Taking exception"):
            h=re.fullmatch(r"Taking exception (5 \[IRQ\]|2 \[SVC\]) on CPU (\d+)",line)
            require(h and int(h[2])<n,"trace exception header")
            c=int(h[2])
            b=re.fullmatch(r"\.\.\.from EL([01]) to EL1\n\.\.\.with ESR 0x[\da-f]+/0x[\da-f]+\n"
                           r"\.\.\.with SPSR (0x[\da-f]+)\n\.\.\.with ELR (0x[\da-f]+)\n"
                           r"\.\.\.to EL1 PC (0x[\da-f]+) PSTATE (0x[\da-f]+)","\n".join(lines[i+1:i+6]))
            r,f,_=groups[c][len(entries[c])]
            require(b and int(b[1])==int(r[7]==n) and (h[1]=="5 [IRQ]")==(r[3]==1)
                    and int(b[4],16)==vec+128*(5 if r[7]==n else 9 if r[3]==1 else 8)
                    and int(b[5],16)&0x8f==0x85 and (r[7]==n or (int(b[3],16),int(b[2],16))==f[32:34]),"external exception origin")
            entries[c].append(i)
        elif line.startswith("Exception return from AArch64 EL1 to AArch64 EL0"):
            h=re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL0 PC (0x[\da-f]+)(?: on CPU (\d+))?",line)
            require(h,"external ERET")
            returns.append((i,int(h[1],16),None if h[2] is None else int(h[2])))
    windows=[]
    for c,rows in enumerate(groups):
        require(len(rows)==len(entries[c]),"missing external exception")
        for j,(r,_,g) in enumerate(rows):
            if r[8]!=n:
                windows.append((c,j,g[32],entries[c][j],entries[c][j+1]))
    graph=[[(c,j) for c,j,pc,l,h in windows if pc==p and l<i<h and tag in (None,c)] for i,p,tag in returns]
    matched={}
    def assign(i,seen):
        for k in graph[i]:
            if k not in seen:
                seen.add(k)
                if k not in matched or assign(matched[k],seen):
                    matched[k]=i
                    return True
        return False
    require(len(graph)==len(windows) and all(assign(i,set()) for i in range(len(graph))),"incomplete user return coverage")
    return {g[0] for g in graph if len(g)==1}


def report(a,m):
    root=a.out
    require(sha(root/"session.json")==a.session_sha,"session identity")
    sess=json.loads((root/"session.json").read_bytes())
    require(sess["reason"]=="terminal" and set(sess["files"])=={p.name for p in root.iterdir() if p.is_file()}-{"session.json"},"terminal capture")
    for name,h in sess["files"].items():
        require(Path(name).name==name and sha(root/name)==h,"artifact "+name)
    launch=json.loads((root/"launch.json").read_bytes())
    traced=a.position=="traced"
    cmd=[launch["argv"][0],"-M","raspi4b","-accel","tcg","-display","none","-serial","file:"+str(root/"uart.bin"),
         "-qmp","stdio","-kernel",str(root/"kernel8.img")]
    require(launch["qemu_sha256"]==MODEL and launch["timeout"]==120 and launch["trace"]==traced and
            launch["argv"]==cmd+(["-d","int","-D",str(root/"exceptions.log")] if traced else []),"frozen launch")
    uart=(root/"uart.bin").read_text("ascii")
    def records(tag):
        return [tuple(map(int,x.split())) for x in re.findall(rf"(?m)^NATIVE MIG {tag} ([0-9 ]+)$",uart)]
    slots=m["slots"]
    n=len(slots)
    allbits=(1<<n)-1
    ver,hz,q,epoch,cpus,tasks=one(records("CONFIG"))
    require((ver,cpus,tasks)==(1,n,n) and hz>0 and q==hz//50 and one(records("END"))==(allbits,allbits&~1),"clock and park")
    cal=sorted(records("CAL"))
    require([r[0] for r in cal]==list(range(n)) and all(r[1]<=r[3]<epoch for r in cal),"calibration")
    offsets=[(0,0)]+[(lo-v,hi-v) for _,lo,v,hi in cal[1:]]
    kernel=elf_file((root/"kernel.elf").read_bytes())
    ram=Ram(root,launch["ram_base"])
    require((ram.base,ram.base+len(ram.data))==(symbol(kernel,"__native_payload_start")[0],symbol(kernel,"__stack_top")[0]),"RAM extent")
    spaces=one(s for s in kernel.get_section_by_name(".symtab").iter_symbols() if "native_el0_mmu" in s.name and "6SPACES" in s.name)
    roots=[((p+1)<<48)|(spaces["st_value"]+p*(spaces["st_size"]//n)) for p in range(n)]
    t=sorted(records("TASK"))
    require(len(t)==n,"task count")
    status=[12,32 if a.case=="calendar" else 12,77][:n]
    require(sorted((int(p),int(v)) for p,v in re.findall(r"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)$",uart))==list(enumerate(status)),"signed exits")
    apps={}
    pages=set()
    for p,s in enumerate(slots):
        require(t[p]==(p,s["cpu_mask"],s["effective_cpu_mask"],s["priority"],s["release_cell"],t[p][5],roots[p]),"task identity")
        path=root/s["elf"]
        if p==1:
            va,size=symbol(elf_file(path.read_bytes()),"private_result")
            data,phys=memory(ram,roots[p],va,size,True)
            require(size==4 and int.from_bytes(data,"little")== (32 if a.case=="calendar" else 78498),"private peer product")
        else:
            role=int(p==2)
            app=instructions(path,(root/(path.name+".dis")).read_text())
            apps[p]=app
            data,phys=memory(ram,roots[p],app["result"],128,True)
            tls=app["tls"]+role*8
            lane=(role+1)*256+1
            total=N*(N+role)
            sig=SIG[role]
            require(struct.unpack("<16Q",data)==(3,role,N,total,total,sig,lane,lane+1,0,tls,0,0,(role+1)<<22,(1-role)<<27,sig,0),"private ABI product")
            cell,extra=memory(ram,roots[p],app["tls"],16,True)
            require(struct.unpack("<2Q",cell)==((0,sig) if role else (sig,0)),"private TLS")
            code,more=memory(ram,roots[p],app["start"],app["ret"]+4-app["start"],False)
            require(digest(code)==app["leaf_sha256"],"loaded ABI code")
            phys=phys+extra+more
        require(not pages.intersection(phys),"aliased private backing")
        pages.update(phys)
    ev,es=symbol(kernel,"native_mig_events")
    fr,fs=symbol(kernel,"native_mig_frames")
    require((es,fs)==(n*256*44*8,n*256*2*816),"journal extent")
    groups=[]
    for c,addr,count,width,lost,frames,size in sorted(records("JOURNAL")):
        require((c,addr,width,lost,frames,size)==(len(groups),ev+c*es//n,44,0,fr+c*fs//n,816) and 0<count<=256,"journal identity/loss")
        groups.append([(ram.words(addr+j*352,44),ram.words(frames+j*1632,102),ram.words(frames+j*1632+816,102)) for j in range(count)])
    require(len(groups)==n,"all CPU journals")
    certain=attribution(root,groups,symbol(kernel,"pi4_native_smp_vectors")[0],n) if traced else set()
    rows=sorted(item for group in groups for item in group)
    owners=[n]*n
    leases=[0]*n
    claims=set()
    charges=[0]*n
    saved={}
    active=set()
    released=exited=0
    last=[None]*n
    transfers=[]
    sends={}
    signals=[]
    denied=[]
    waits=[]
    wake_rows=[]
    clock_entry=sleep_target=None
    for revision,(r,f,g) in enumerate(rows,1):
        rev,c,j,cause,op,entry,now,src,dst,seq,new,R,delta,charge,prior,after=r[:16]
        require(rev==revision and groups[c][j][0]==r and oldmap(r[22],n)==tuple(owners) and src==owners[c] and seq==leases[c] and entry<=now,"ownership/revision")
        if src!=n:
            require(prior==roots[src] and R and delta==entry-R and last[c][43]<=R and f[33]&31==0 and f[31]%16==0,"real return/retirement")
            memory(ram,prior,f[31]-8,8,True)
            charges[src]+=delta
            require(charge==charges[src],"one charge per lease")
            if cause==2:
                require(f[8]==op and f[0]==r[29] and r[31]>>26==0x15 and r[31]&65535==0x4a01,"SVC")
                if op==1:
                    require(f[0]==status[src] and src in active,"exit status")
                    active.remove(src)
                    exited|=1<<src
                if op==6 and src==1 and a.case=="calendar":
                    require(clock_entry is None and f[0]==1 and r[30]==0 and hz==62500000,"calendar clock origin/sample")
                    # Each 62.5 MHz tick is exactly 16 ns. The client's +5 ms
                    # therefore round-trips exactly through the two ABI conversions.
                    clock_entry=entry
                    sleep_target=entry+hz//200
                if op==7:
                    require(a.case=="calendar" and src==1 and f[:2]==(1,1) and r[30]==0,"absolute sleep")
                    require(sleep_target is not None and entry<sleep_target,"future absolute target")
                    active.remove(src)
                    waits.append(rev)
            saved[src]=(c,j,rev,f if cause==1 else (r[30],*f[1:]),r)
        else:
            require(seq==delta==charge==0 and cause==1,"idle charge")
        if cause==1:
            require((op==30 or op&1023 in (1,2) and op>>10<n) and r[34]>=now,"IRQ and EOI")
            if op==30:
                require(r[16]==r[18]<=entry and r[19]&7==5,"physical comparator")
            if op&1023==1:
                sender=op>>10
                require((sender,c) in sends,"unsourced SGI")
                send=sends.pop((sender,c))
                signals.append((rev,c,sender,send))
        else:
            require(cause==2 and not r[34],"exception cause")
        due=sum(1<<p for p,s in enumerate(slots) if not released>>p&1 and now>=epoch+q*s["release_cell"]) if c==0 else 0
        require(r[20]==due and r[27]==exited,"absolute release/exits")
        released|=due
        for p in range(n):
            if due>>p&1 or r[21]>>p&1:
                active.add(p)
        if r[21]:
            require(a.case=="calendar" and r[21]==2 and waits and not wake_rows and now>=sleep_target,"single absolute wake")
            wake_rows.append(rev)
        proposed=plan(owners,active,slots)
        require(oldmap(r[23],n)==proposed,"ranked effective-mask placement")
        if a.case=="confined" and active=={0,1,2} and proposed==(1,2,n) and owners==[0,2,n] and c==0 and dst==1:
            denied.append(rev)
        owners[c]=n
        expected=proposed[c] if proposed[c] not in owners else n
        require(dst==expected,"vacant exclusive claim")
        owners[c]=dst
        leases[c]=new
        if dst!=n:
            require(new==len(claims)+1 and new not in claims and after==roots[dst],"new lease/TTBR")
            claims.add(new)
            if dst in saved:
                prev,idx,rv,frame,ret=saved[dst]
                require(g==frame,"complete saved-to-restored frame")
                if dst==0 and c!=prev and apps[0]["loop"]<=g[32]<apps[0]["end"]:
                    transfers.append((rv,rev,prev,c,idx,j,g))
        else:
            require(new==0,"idle claim")
        for peer in range(n):
            lo,hi=r[35+2*peer:37+2*peer]
            if r[24]>>peer&1:
                require(peer!=c and now<=lo<=hi and (c,peer) not in sends,"targeted SGI send")
                sends[c,peer]=(rev,lo,hi)
            else:
                require(lo==hi==0,"unrequested SGI send")
        if r[43]:
            require(r[41]==r[17] and r[42]&3==1 and r[43]>=now,"timer arm")
        last[c]=r
    require(released==exited==allbits and owners==[n]*n and charges==[x[5] for x in t],"terminal ownership and charges")
    witnesses=[]
    for rv,rev,src,c,idx,j,g in transfers:
        if src!=0 or c!=(2 if a.case=="global3" else 1):
            continue
        dest=groups[c]
        if j+1>=len(dest):
            continue
        rr,ff,_=dest[j+1]
        if rr[7]!=0 or not apps[0]["loop"]<=ff[32]<apps[0]["end"] or ff[4]<=g[4]:
            continue
        if not live(g,apps[0]) or not live(ff,apps[0]):
            continue
        higher=groups[src][idx][0]
        target_mapping=oldmap(higher[23],n)
        if n==3 and (higher[27]&4 or 2 not in target_mapping or
                     oldmap(higher[22],n)[1 if a.case=="global3" else 2]!=2):
            continue
        if higher[8]!=1 or higher[3]!=1:
            continue
        causal=[]
        for recv,receiver,sender,(sent,lo,hi) in signals:
            if recv==rev and receiver==c and sender==src and rv<=sent and hi+offsets[src][1]<dest[j][0][5]+offsets[c][0]:
                causal.append(dict(send=sent,receive=recv,send_ticks=[lo,hi]))
        witnesses.append(dict(retirement=rv,claim=rev,source=src,destination=c,sequence=rr[9],
                              actual_R=rr[11],first_later_work=rr[5],external_return=(c,j) in certain,sgi=causal))
    if a.case=="calendar":
        ok=len(waits)==len(wake_rows)==1 and waits[0]<wake_rows[0] and any(r[0]>=wake_rows[0] and r[8]==1 for r,_,_ in rows)
    elif a.case=="confined":
        ok=any(any(r[0]>rv and r[8]==0 and r[1]<2 for r,_,_ in rows) for rv in denied)
    else:
        ok=any((not traced or w["external_return"]) and (a.case=="within" or w["sgi"]) for w in witnesses)
    return dict(case=a.case,position=a.position,session=a.session_sha,observed=ok,charges=charges,
                migrations=witnesses,confinement_revisions=denied,sleep=waits,absolute_sleep_target=sleep_target,wake=wake_rows,offset_ticks=offsets)


def oldmap(word,n):
    require(word<1<<(4*n),"map extent")
    return tuple(word>>(4*c)&15 for c in range(n))


def live(f,app):
    step=(f[32]-app["loop"])//4
    done=f[4]-int(step>=5)
    value=2*done+1
    return (0<=done<N and f[7]==N and f[19]==SIG[0] and f[70:72]==(257,258)
            and f[34:38]==(app["tls"],0,1<<22,1<<27) and f[5]==value+2*int(step>=4)
            and f[6]==done*done+value*int(step>=1) and f[40]==done*done+value*int(step>=3))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("action",choices=["capture","report"])
    p.add_argument("case",choices=CASES)
    p.add_argument("paths",nargs="+",type=Path)
    p.add_argument("position",choices=["plain","traced"])
    for key in ("manifest","odd","even","calendar","session"):
        p.add_argument("--"+key+"-sha",required=key in ("manifest","odd"))
    p.add_argument("--qemu",default="qemu-system-aarch64")
    p.add_argument("--nm",default="aarch64-linux-gnu-nm")
    p.add_argument("--objdump",default="aarch64-linux-gnu-objdump")
    a=p.parse_args()
    if a.action=="capture":
        a.package,a.out=a.paths
        config(a,a.package)
        a.qemu=str(Path(shutil.which(a.qemu) or a.qemu).resolve(strict=True))
        require(sha(Path(a.qemu))==MODEL,"exact stock emulator")
        a.trace=a.position=="traced"
        return capture(a)
    a.out=one(a.paths).resolve()
    out=report(a,config(a,a.out))
    print(json.dumps(out,indent=2))
    return 0 if out["observed"] else 2


if __name__=="__main__":
    try:
        sys.exit(main())
    except (OSError,ValueError,KeyError,IndexError) as exc:
        print(json.dumps(dict(observed=False,reason=str(exc))))
        sys.exit(2)
