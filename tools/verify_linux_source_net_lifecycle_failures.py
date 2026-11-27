#!/usr/bin/env python3
"""Capture four upstream virtio-net error paths and a clean recovery in one native boot."""
import argparse
import json
from pathlib import Path
import re
import secrets
import shutil
import socket
import struct
import subprocess
import tempfile
import threading
import time

from elftools.elf.elffile import ELFFile
from elftools.elf.relocation import RelocationSection
import verify_linux_source_net_lifecycle as life
import verify_linux_source_net_irq as irq
import verify_linux_source_net_rx as rx
from native_amc_roots import linked_roots
from native_el0_replay import PA_MASK, permissions, vread
from qemu_capture import capture_halt, journal_location, request
from verify_linux_source_net import RAM, NEG, NIC, callback_attempts, load, require, save, sha
from verify_linux_source_net_packets import MAX_LOG, STRIDE, area, challenge, dma_layout, pcap_frames, socket_frames
from verify_linux_source_net_queues import routine

DRIVER=("jnox_failure_driver","jnox_lifecycle_fault_events","jnox_lifecycle_fault_state",
        "jnox_lifecycle_netdev_journal","jnox_lifecycle_registered_driver","jnox_lifecycle_common_live","jnox_failure_registry","jnox_lifecycle_fault_resources")
PEER=("jnox_failure_peer","jnox_failure_recovery_bytes")
FUNCS={"alloc_etherdev_mqs":"lifecycle_netdev.o","unregister_netdev":"lifecycle_netdev.o",
       "jnox_lifecycle_fault_hit_v1":"lifecycle_faults.o","virtnet_probe":"virtio_net.callback.o","virtnet_open":"virtio_net.callback.o",
       "virtnet_close":"virtio_net.callback.o","virtnet_remove":"virtio_net.callback.o",
       "skb_recv_done":"virtio_net.callback.o","virtnet_poll":"virtio_net.callback.o",
       "gro_receive_skb":"rx_skb.o","register_netdevice":"lifecycle_netdev.o","free_netdev":"lifecycle_netdev.o",
       "__cpuhp_state_add_instance":"lifecycle_platform.o","__cpuhp_state_remove_instance":"lifecycle_platform.o",
       "xdp_rxq_info_reg_mem_model":"lifecycle_platform.o","xdp_rxq_info_unreg":"lifecycle_platform.o"}
PRIVATE={"allocated":("netdev",8),"registered":("netdev",8),"rtnl_owner":("netdev",8),
         "online":("platform",32),"dead":("platform",32),"registered_rx":("platform",8),"shared_model":("platform",1)}
STAGES=tuple(f"case-{n}-{s}" for n in range(1,5) for s in ("armed","failed","retired"))+("recovery-ready","recovery-ack","final")


def layout(pkg,imp):
    m=load(imp)
    native=load(pkg/"manifest.json")
    require(m["mode"]=="lifecycle-failure-service" and m["fault_fixture"]["test_only"] is True and
            m["source"]["sha256"]==m["upstream"]["sha256"]==rx.PIN and
            m["upstream"]["commit"]=="4700e3adf3b24b39dd4929e9c6d62811ba3ccea8" and
            m["callback_input"]=="virtio_net.packet.o","fault-only pinned intake")
    kernel=pkg/"kernel.elf"
    k=kernel.read_bytes()
    require(sha(kernel)==native["kernel_elf_sha256"] and len(native["slots"])==2 and native["policy"]=="fifo" and
            native["linux_source_net_lifecycle"]["driver_slot"]==0 and native["linux_source_net_lifecycle"]["peer_slot"]==1,
            "native owner and peer authority")
    for n in native["slots"]:
        p=pkg/n["elf"]
        require(sha(p)==n["elf_sha256"] and p.read_bytes() in k,"embedded native executable")
    for file,expected in (("service.map",m["link_map_sha256"]),("virtio_net.original.o",m["objects"]["virtio_net.original.o"]["sha256"]),
                          ("virtio_net.packet.o",m["driver_object"]["sha256"]),("virtio_net.callback.o",m["callback_object"]["sha256"])):
        require(sha(imp.parent/file)==expected,"import object identity")
    require(sha(pkg/"user.elf")==m["service"]["sha256"],"import and packaged driver")
    linkmap=(imp.parent/"service.map").read_text().splitlines()
    code={}
    callers=[]
    actual={}
    with (pkg/"user.elf").open("rb") as h:
        elf=ELFFile(h)
        providers=dict(FUNCS)
        alloc_candidates=[]
        hook=life.symbol(elf,"jnox_lifecycle_fault_hit_v1")["st_value"]
        for s in elf.get_section_by_name(".symtab").iter_symbols():
            if not re.fullmatch(r"alloc_netdev_mqs(?:\.part\.\d+)?",s.name) or s["st_info"]["type"]!="STT_FUNC":
                continue
            for offset,(ins,) in enumerate(struct.iter_unpack("<I",life.data(elf,s))):
                if ins>>26!=0x25:
                    continue
                imm=ins&0x3ffffff
                delta=imm-(1<<26) if imm&(1<<25) else imm
                if s["st_value"]+4*(offset+delta)==hook:
                    alloc_candidates.append(s.name)
                    break
        require(len(alloc_candidates)==1,"unique linked allocation provider calling fault hook")
        allocation=alloc_candidates[0]
        providers[allocation]="lifecycle_netdev.o"
        for name,supplier in providers.items():
            sym,body=routine(elf,name)
            with (imp.parent/("virtio_net.packet.o" if supplier=="virtio_net.callback.o" else supplier)).open("rb") as f:
                src=ELFFile(f)
                orig,raw=routine(src,name)
                sec=src.get_section(orig["st_shndx"])
                rel={(r["r_offset"]-orig["st_value"])//4 for s in src.iter_sections() if isinstance(s,RelocationSection)
                     and s["sh_info"]==orig["st_shndx"] for r in s.iter_relocations()}
                require(len(raw)==len(body) and all(body[i:i+4]==raw[i:i+4] for i in range(0,len(raw),4) if i//4 not in rel)
                        and sum(l.strip().endswith(f"{supplier}:({sec.name})") for l in linkmap)==1,
                        "linked original/provider code "+name)
            code[name]=dict(entry=sym["st_value"],size=len(body),prefix=body[:16].hex(),peer=None)
        for name,(unit,n) in PRIVATE.items():
            hits=[l.split() for l in linkmap if l.strip().endswith(f"lifecycle_{unit}.o:(.bss.{name})")]
            require(len(hits)==1 and int(hits[0][2],16)==n,"private resource ELF storage "+name)
            actual[name]=(int(hits[0][0],16),n)
        for s in elf.get_section_by_name(".symtab").iter_symbols():
            if s["st_info"]["type"]=="STT_FUNC" and s["st_size"] and isinstance(s["st_shndx"],int):
                callers.append((s.name,s["st_value"],life.data(elf,s)))
        driver_ptr=life.symbol(elf,"virtio_net_driver")["st_value"]
    journals=[{n:journal_location(pkg/native["slots"][i]["elf"],n) for n in names} for i,names in enumerate((DRIVER,PEER))]
    faults={}
    for n in ("jnox_rx_napi_fault","jnox_rx_poll_fault_id","jnox_lifecycle_bus_fault","jnox_lifecycle_fault_id","jnox_lifecycle_dim_fault"):
        span=journal_location(pkg/"user.elf",n,optional=True)
        if span: faults[n]=span
    ring,buf=dma_layout(pkg)
    return dict(roots=linked_roots(kernel,2),allocation=allocation,code=code,callers=callers,driver_ptr=driver_ptr,actual=actual,journals=journals,faults=faults,
                ring=ring,buffers=buf,kernel_irq=journal_location(kernel,"jnox_linux_net_irq_metrics"),
                driver_sha256=sha(pkg/"user.elf"),kernel_sha256=sha(kernel),import_sha256=sha(imp))


def state(mem,L):
    users=[]
    pages=[]
    for i,root in enumerate(L["roots"]):
        syms=dict(L["journals"][i])
        if not i:
            syms.update(L["actual"])
            syms.update(L["faults"])
        private=set()
        data={}
        for name,(va,size) in syms.items():
            for vpn in range(va//4096,(va+size-1)//4096+1):
                private.add(int(permissions(mem,root,vpn*4096,"user rw"),16)&PA_MASK)
            data[name]=vread(mem,root,va,size)
        pages.append(private)
        users.append(data)
    protected=set()
    for addr,size in ((L["ring"],8192),(L["buffers"],64*STRIDE),L["kernel_irq"]):
        protected.update(range(addr//4096*4096,(addr+size+4095)//4096*4096,4096))
    for i,root in enumerate(L["roots"]):
        for pa in protected:
            permissions(mem,root,pa,"kernel rw")
        permissions(mem,root,NIC&~4095,"kernel rw",device=True)
        forbidden=protected|pages[1-i]
        tables={root&PA_MASK}
        for depth in range(3):
            children=set()
            for table in tables:
                for (desc,) in struct.iter_unpack("<Q",mem.read(table,4096)):
                    if not desc&1: continue
                    require(desc&3==3,"unexpected translation table")
                    if depth<2: children.add(desc&PA_MASK)
                    elif desc&64: require(desc&PA_MASK not in forbidden,"EL0 alias of peer or protected DMA")
            tables=children
    drv,peer=users
    actual={n:int.from_bytes(drv[n],"little") for n in PRIVATE if n not in ("online","dead")}
    for n in ("online","dead"):
        # bool registered, seven bytes alignment, two callback pointers and the actual hlist_head.first.
        actual[n]=int.from_bytes(drv[n][24:32],"little")
        actual[n+"_registered"]=drv[n][0]
    return dict(driver=life.unpack(drv[DRIVER[0]]),events=life.unpack(drv[DRIVER[1]]),states=life.unpack(drv[DRIVER[2]]),
                net=life.unpack(drv[DRIVER[3]]),registered=life.unpack(drv[DRIVER[4]])[0],common=life.unpack(drv[DRIVER[5]])[0],
                registry=life.unpack(drv[DRIVER[6]]),resources=life.unpack(drv[DRIVER[7]]),faults={n:int.from_bytes(drv[n],"little") for n in L["faults"]},actual=actual,
                peer=life.unpack(peer[PEER[0]]),wire=peer[PEER[1]],rings=area(mem,L["ring"],8192),
                buffers=area(mem,L["buffers"],64*STRIDE),irq=life.unpack(mem.read(*L["kernel_irq"])))


def capture(pkg,imp,out,qemu,mtu,mac,timeout):
    L=layout(pkg,imp)
    binary=shutil.which(qemu)
    require(binary,"QEMU missing")
    out.mkdir(parents=True,exist_ok=False)
    (out/"net.events").write_text(irq.TRACE)
    nonce=secrets.token_bytes(8)
    while not any(nonce): nonce=secrets.token_bytes(8)
    (out/"nonce.bin").write_bytes(nonce)
    addr=bytes.fromhex(mac.replace(":",""))
    deadline=time.monotonic()+timeout
    host,nic=socket.socketpair()
    with host,nic,(out/"stderr.log").open("xb") as err,(out/"host.bin").open("xb") as sent:
        fd=nic.fileno()
        argv=life.command(binary,out,pkg/"kernel.elf",fd,mac,mtu,L)
        with subprocess.Popen(argv,pass_fds=(fd,),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,bufsize=0) as proc:
            nic.close()
            class Pipe:
                readline=proc.stdout.readline
                write=proc.stdin.write
            q=Pipe()
            timer=threading.Timer(timeout+20,proc.kill)
            timer.start()
            save(out/"launch.json",dict(schema=1,argv=argv,kernel=str(pkg/"kernel.elf"),out=str(out),pid=proc.pid,fd=fd,
                 mtu=mtu,mac=mac,import_sha256=sha(imp),kernel_sha256=L["kernel_sha256"],driver_sha256=L["driver_sha256"]))
            def read(pa,n):
                request(q,"pmemsave",val=pa,size=n,filename=str(out/"probe.bin"))
                return (out/"probe.bin").read_bytes()
            def field(slot,offset):
                va=L["journals"][slot][(DRIVER if slot==0 else PEER)[0]][0]+8*offset
                mem=RAM({},read)
                return int.from_bytes(vread(mem,L["roots"][slot],va,8),"little") if mem.leaf(L["roots"][slot],va) else 0
            def uart():
                return (out/"uart.log").read_bytes() if (out/"uart.log").exists() else b""
            def wait(name,pred):
                nonlocal current
                current=name
                while True:
                    u=uart()
                    require(not re.search(rb"NATIVE EL0 EXIT PID [01] (?!0\r?$)-?\d+",u,re.M) and b"NATIVE EL0 FAULT" not in u,
                            "failed role or native fault at "+name)
                    if pred(): return
                    require(proc.poll() is None and time.monotonic()<deadline,"deadline or QEMU exit at "+name)
                    require(not (out/"machine.log").exists() or (out/"machine.log").stat().st_size<=MAX_LOG,"trace size limit")
                    time.sleep(.004)
            def freeze(name):
                where=out/name
                where.mkdir()
                capture_halt(q,where)
                mem=RAM({},read)
                try:
                    return state(mem,L)
                finally:
                    save(where/"ram.json",mem.records)
            def send(frame):
                b=len(frame).to_bytes(4,"big")+frame
                host.settimeout(max(.001,deadline-time.monotonic()))
                host.sendall(b)
                sent.write(b)
                sent.flush()
            current="boot"
            try:
                require("QMP" in json.loads(q.readline()),"QMP greeting")
                request(q,"qmp_capabilities")
                for case in range(4):
                    for i,phase in enumerate(("armed","failed","retired"),1):
                        name=f"case-{case+1}-{phase}"
                        wait(name,lambda:field(0,32*case+1)==i)
                        shot=freeze(name)
                        require(shot["driver"][32*case+1]==i,"missed bounded fixture hold "+name)
                        request(q,"cont")
                def blocked():
                    return int.from_bytes(read(L["kernel_irq"][0]+22*8,8),"little")==1
                wait("recovery-ready",lambda:field(0,129)==2 and blocked())
                freeze("recovery-ready")
                request(q,"cont")
                send(rx.frames(addr,nonce)[0])
                wait("recovery-ack",lambda:field(1,38)==1 and blocked())
                freeze("recovery-ack")
                request(q,"cont")
                send(challenge(addr,nonce,True))
                wait("final",lambda:len(re.findall(rb"(?m)^NATIVE EL0 EXIT PID [01] 0\r?$",uart()))==2)
                freeze("final")
                request(q,"quit")
                require(proc.wait(timeout=5)==0 and host.recv(1)==b"","shutdown or unexpected egress")
                (out/"probe.bin").unlink(missing_ok=True)
            except Exception as exc:
                diagnostics=dict(error=repr(exc),stage=current,uart=uart()[-2048:].decode(errors="replace"))
                if proc.poll() is None:
                    try:
                        request(q,"stop")
                        mem=RAM({},read)
                        for i,syms in enumerate(L["journals"]):
                            for n,(va,sz) in dict(syms,**(L["faults"] if not i else {})).items():
                                try:
                                    raw=vread(mem,L["roots"][i],va,sz)
                                    if n in (DRIVER[0],DRIVER[1],*L["faults"]):
                                        diagnostics[n]=life.unpack(raw) if sz%8==0 else raw.hex()
                                except Exception as error:
                                    diagnostics.setdefault("unreadable",{})[n]=repr(error)
                        save(out/"failure-ram.json",mem.records)
                    except Exception as error:
                        diagnostics["stop_error"]=repr(error)
                save(out/"failure.json",diagnostics)
                raise
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
                timer.cancel()
                timer.join()


def crosscheck_live(s,row):
    a=s["actual"]
    require([a["allocated"],a["registered"],a["rtnl_owner"]]==row[:3] and
            [a["online"],a["dead"],a["registered_rx"],a["shared_model"]]==row[4:8] and
            [s["common"],s["registered"]]==row[9:11] and s["net"][:4]==row[11:15],
            "raw ELF private resources disagree with captured failure snapshot")
    require(not any(s["faults"].values()) and not s["net"][31] and not a["rtnl_owner"],"runtime fault or leaked RTNL")
    require(s["net"][22]==a["registered"],"netdevice registry mismatch")


def check_empty(s):
    a=s["actual"]
    require(not any(a.values()) and not s["common"] and not s["registered"] and not any(s["buffers"]),
            "netdevice, CPUHP, XDP, allocator, module registration or DMA leak")
    require(not s["net"][22] and not s["net"][23] and s["net"][0]==s["net"][1] and s["net"][2]==s["net"][3],
            "unbalanced netdev lifecycle")


def replay(pkg,imp,out):
    L=layout(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["schema"]==1 and all(launch[k]==L[k] for k in ("kernel_sha256","driver_sha256","import_sha256"))
            and (out/"net.events").read_text()==irq.TRACE and launch["argv"]==life.command(launch["argv"][0],
            Path(launch["out"]),Path(launch["kernel"]),launch["fd"],launch["mac"],launch["mtu"],L),"fault capture identity")
    shots={}
    for name in STAGES:
        where=out/name
        q,h=load(where/"qmp.json"),load(where/"halted.json")
        require(q["status"]["running"] and q["stop_return"]=={} and [c["cpu-index"] for c in q["cpus"]]==[0] and
                h["status"]=="paused" and not h["running"],"stopped CPU0")
        mem=RAM(load(where/"ram.json"))
        shots[name]=state(mem,L)
        require(mem.used==set(mem.records),"unexpected RAM read")
    final=shots["final"]
    results=[]
    tokens=[]
    for i in range(4):
        case=i+1
        armed,failed,retired=[shots[f"case-{case}-{phase}"] for phase in ("armed","failed","retired")]
        slices=[s["events"][i*8:(i+1)*8] for s in (armed,failed,retired,final)]
        before,hit,done,frozen=slices
        require(before==[case,0,0,0,0,0,1,0] and hit[:3]==[case,1,case] and hit[3] and hit[4] and
                hit[5:]==[case,7,0 if case==1 else NEG-12] and done==hit[:6]+[15,hit[7]] and frozen==done,
                "fault arm, one-shot provider hit, result or freeze")
        histories=[final["states"][(i*4+j)*20:(i*4+j+1)*20] for j in range(4)]
        for s,j,phase in ((armed,0,1),(failed,2,2),(retired,3,3)):
            require(s["driver"][i*32:i*32+2]==[case,phase] and s["states"][(i*4+j)*20:(i*4+j+1)*20]==histories[j],
                    "phase identity or changed saved resources")
            crosscheck_live(s,histories[j])
        a,f,returned,clean=histories
        resource=[final["resources"][(i*4+j)*8:(i*4+j+1)*8] for j in range(4)]
        require(not any(resource[0][:3]) and not any(resource[3][:4]) and
                failed["resources"][(i*4+2)*8:(i*4+3)*8]==resource[2] and
                all(not r[7] for r in resource),"RX page, SKB or queue cleanup")
        if case==4:
            require(all(r[0]==32 and r[1]>0 and r[2] for r in resource[1:3]),
                    "failed-open mapped RX grant/page retention")
        else:
            require(not any(resource[2][:4]),"failed probe retained RX pages")
        d=final["driver"][32*i:32*(i+1)]
        p=final["peer"][8*i:8*(i+1)]
        require(d[:4]==[case,100,NEG-12 if case<4 else 0,NEG-12 if case==4 else 0] and not d[31] and
                p[:5]==[case,d[4],NEG-13,NEG-13,NEG-13] and p[7]==0,
                "upstream callback errno or independent peer authority")
        require(a[10]==L["driver_ptr"] and not a[0] and not a[1] and not a[8] and f[10]==L["driver_ptr"],
                "fault entered wrong registered driver")
        if case==1:
            require(not any(f[:2]) and not f[8] and not d[4],"allocation injection after resource acquisition")
        else:
            require(f[0]!=0 and f[8]==d[4]>0 and f[16]!=0 and f[17]!=0,"fault outside actual live queues")
            tokens.append(d[4])
        if case==2:
            require(f[1]==0 and f[2]!=0 and hit[3]==f[0],"registration hook after publication or without RTNL")
        if case==3:
            require(f[0]==f[1]!=0 and f[4]!=0 and not f[5],"second notifier hook without first online instance")
        if case==4:
            require(f[0]==f[1]!=0 and all(f[k] for k in (4,5,6)) and f[6]==hit[3] and not f[7],
                    "XDP page-shared failure outside live RX registration")
            require(returned[0]==returned[1]!=0 and returned[8]==d[4] and returned[10]==L["driver_ptr"] and
                    returned[4] and returned[5] and not any(returned[6:8]) and not returned[3]&1 and
                    failed["net"][23]==0 and failed["net"][18]==NEG-12 and d[5:8]==[d[4],15,32] and
                    d[8]>0 and d[9]==d[4] and d[10]==NEG-100,"failed open must remain registered DOWN")
            ring=failed["rings"][:4096]
            require(struct.unpack_from("<H",ring,514)[0]==32 and struct.unpack_from("<H",ring,586)[0]==0,
                    "posted RX grants lost or unexpectedly used before remove")
            for j in range(32):
                require(struct.unpack_from("<QIHH",ring,16*j)==(L["buffers"]+STRIDE*j,1530,2,0),
                        "failed open descriptor ownership")
        else:
            check_empty(failed)
            require(d[5:9]==[0,NEG-19,0,0],"failed probe retained device transport")
        check_empty(retired)
        require(d[13:17]==[NEG-19,0,0,0] and (not d[4] or d[17:19]==[NEG-116,NEG-116]) and
                not any(clean[k] for k in (*range(11),16,17,18,19)),"post-cleanup device, IRQ or stale grants")
        require([returned[n]-a[n] for n in range(11,15)]==([0,0,0,0] if case==1 else
                [1,1,0,0] if case==2 else [1,1,1,1] if case==3 else [1,0,1,0]),"case resource acquisition and unwind")
        ret=hit[4]
        sites=[(name,va,b) for name,va,b in L["callers"] if va<=ret-4 and ret<=va+len(b)]
        require(sites,"fault hook caller outside linked code")
        names=set()
        for name,va,body in sites:
            ins=struct.unpack_from("<I",body,ret-4-va)[0]
            require((ins&0xfc000000)==0x94000000 or (ins&0xfffffc1f)==0xd63f0000,"fault caller lacks AArch64 call")
            names.add(name)
        results.append(dict(case=case,caller_symbols=sorted(names),errno=-12,provider_caller=hex(hit[4]),provider_argument=hex(hit[3]),token=d[4]))
    d=final["driver"][128:]
    p=final["peer"][32:]
    require(d[:4]==[5,100,0,0] and d[4]==d[5]>0 and tokens+[d[4]]==sorted(set(tokens+[d[4]])) and
            d[20:23]==[NEG-116]*3 and d[13:17]==[NEG-19,0,0,0] and d[25]==2 and not d[31] and
            p[:5]==[5,d[4],NEG-13,NEG-13,NEG-13] and p[6:]==[2,0],"clean recovery, authority or token reuse")
    check_empty(final)
    nonce=(out/"nonce.bin").read_bytes()
    addr=bytes.fromhex(launch["mac"].replace(":",""))
    wire=[rx.frames(addr,nonce)[0],challenge(addr,nonce,True)]
    pcap=pcap_frames(out/"wire.pcap")
    require(len(nonce)==8 and any(nonce) and socket_frames((out/"host.bin").read_bytes())==wire and len(pcap)==2 and
            all(x in (f,bytes(10)+f,bytes(12)+f) for x,f in zip(pcap,wire)) and final["wire"]==b"".join(wire) and
            shots["recovery-ready"]["wire"]==bytes(120) and shots["recovery-ack"]["wire"]==wire[0]+bytes(60) and
            p[5]==d[24]==int.from_bytes(nonce,"big"),"fresh recovery bytes differ")
    return trace_evidence(out,L,shots,results,d[4])


def trace_evidence(out,L,shots,results,token):
    log=out/"machine.log"
    require(log.stat().st_size<=MAX_LOG,"QEMU trace size")
    lines=log.read_bytes().splitlines()
    mm=[]
    for i,line in enumerate(lines):
        m=re.search(rb"memory_region_ops_(read|write) cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size 4",line)
        if m and 0<=int(m[2],16)-NIC<512:
            mm.append((i,m[1],int(m[2],16)-NIC,int(m[3],16)))
    statuses=[(i,v) for i,op,r,v in mm if (op,r)==(b"write",112)]
    starts=[i for i,v in statuses if v==1]
    require(len(starts)==5,"expected five distinct hardware claims")
    rows={}
    for name,meta in L["code"].items():
        _,executed=callback_attempts(lines,meta["entry"])
        if executed:
            rx.verify_execution(lines,meta,len(executed))
        rows[name]=executed
    epochs=irq.delivery_epochs(lines)
    irq.wire_delivery(lines,epochs,False)
    counts=[]
    for i,first in enumerate(starts):
        end=starts[i+1] if i<4 else len(lines)
        writes=[(j,v) for j,v in statuses if first<=j<end]
        require([v for _,v in writes]==[1,3,11]+([15] if i>=2 else [])+([0,0] if i<4 else [0]),
                "unexpected status lifecycle in case "+str(i+1))
        release=next(j for j,v in writes if v==0)
        reset_seen=False
        selected=None
        ready=set()
        # Only the retiring owner's reset supplies these readbacks.
        stop=next((j for j,v in writes if j>release and v==0),end)
        for j,op,off,v in mm:
            if release<j<stop:
                if (op,off,v)==(b"read",112,0): reset_seen=True
                if (op,off)==(b"write",48): selected=v
                if (op,off,v)==(b"read",68,0) and reset_seen: ready.add(selected)
        require(reset_seen and {0,1}<=ready,"missing retirement reset or queue-ready readback in case "+str(i+1))
        events={n:[x for x in v if first<=x<end] for n,v in rows.items()}
        e={n:len(v) for n,v in events.items()}
        require(e["alloc_etherdev_mqs"]==e[L["allocation"]]==1 and e["virtnet_probe"]==1 and e["virtnet_open"]==int(i>=3) and e["virtnet_remove"]==int(i>=3) and
                e["virtnet_close"]==int(i==4) and e["register_netdevice"]==int(i>=1) and
                e["unregister_netdev"]==int(i>=2) and e["free_netdev"]==int(i>=1),
                "original lifecycle entry/rollback counts in case "+str(i+1))
        if i<4:
            require(not any(first<x["iar"]<end and x["status"]&1 for x in epochs) and
                    not e["gro_receive_skb"] and not e["virtnet_poll"] and not e["skb_recv_done"],
                    "RX delivery during fault trial")
            require(e["jnox_lifecycle_fault_hit_v1"]>0,"fault provider never executed")
            if i==2:
                require(e["__cpuhp_state_add_instance"]==2 and e["__cpuhp_state_remove_instance"]==1 and
                        events["__cpuhp_state_add_instance"][1]<events["__cpuhp_state_remove_instance"][0]<
                        events["unregister_netdev"][0],"CPU hotplug partial registration unwind order")
            if i==3:
                require(e["xdp_rxq_info_reg_mem_model"]==1 and e["xdp_rxq_info_unreg"]==1 and
                        events["virtnet_open"][0]<events["xdp_rxq_info_reg_mem_model"][0]<
                        events["xdp_rxq_info_unreg"][0]<events["virtnet_remove"][0],"failed-open XDP rollback order")
        else:
            genuine=[x for x in epochs if first<x["iar"]<release and x["status"]&1]
            # Upstream open can schedule a poll before any device input. Bind
            # each delivery to its own serviced IRQ and ensuing callback/poll.
            require(len(genuine)==2 and e["gro_receive_skb"]==2 and e["skb_recv_done"]==2,
                    "clean original IRQ delivery and GRO")
            for j,event in enumerate(genuine):
                cb=events["skb_recv_done"][j]
                gro=events["gro_receive_skb"][j]
                bound=genuine[j+1]["iar"] if j+1<len(genuine) else release
                require(event["eoi"]<cb<gro<bound and any(cb<poll<gro for poll in events["virtnet_poll"]),
                        "clean original IRQ delivery and GRO chronology")
        counts.append(e)
    uart=(out/"uart.log").read_bytes()
    require(set(re.findall(rb"(?m)^NATIVE EL0 EXIT PID ([01]) (-?\d+)\r?$",uart))=={(b"0",b"0"),(b"1",b"0")}
            and b"NATIVE EL0 FAULT" not in uart,"native exits")
    return dict(injected=results,recovery_token=token,recovery_frames=2,hardware_rx_irqs=len(epochs),entry_counts=counts)


def controls(pkg,imp,out):
    L=layout(pkg,imp)
    rejected=[]
    with tempfile.TemporaryDirectory(prefix="jnox-fault-controls-") as tmp:
        work=Path(tmp)/"capture"
        shutil.copytree(out,work)
        def reject(label,path,replacement,reason):
            original=path.read_bytes()
            try:
                path.write_bytes(replacement)
                try:
                    replay(pkg,imp,work)
                except ValueError as exc:
                    require(reason in str(exc),"wrong rejection for "+label+": "+str(exc))
                else:
                    raise ValueError("accepted mutation "+label)
            finally:
                path.write_bytes(original)
            rejected.append(label)
        def change(stage,slot,symbol,index,label,reason):
            va,size=L["journals"][slot].get(symbol) or L["actual"][symbol]
            where=work/stage/"ram.json"
            records=load(where)
            while size:
                n=min(size,4096-va%4096)
                if index<n:
                    pa=(RAM(records).leaf(L["roots"][slot],va)&PA_MASK)+va%4096
                    key=f"{pa:x}/{n}"
                    raw=bytearray.fromhex(records[key]); raw[index]^=1
                    records[key]=raw.hex()
                    reject(label,where,json.dumps(records).encode(),reason)
                    return
                va+=n; size-=n; index-=n
            raise ValueError("mutation outside symbol")
        change("case-2-failed",0,DRIVER[1],8*(8+1),"absent register failure","one-shot")
        change("case-3-failed",0,"online",24,"unremoved hotplug instance","raw ELF private")
        change("case-4-failed",0,"registered",0,"false DOWN registration","raw ELF private")
        change("final",1,PEER[1],22,"corrupted peer nonce","recovery bytes")
    return rejected


if __name__=="__main__":
    cli=argparse.ArgumentParser(description=__doc__)
    for name in ("package","driver-manifest","out"):
        cli.add_argument("--"+name,type=Path,required=True)
    cli.add_argument("--replay",action="store_true")
    cli.add_argument("--mutation-tests",action="store_true")
    cli.add_argument("--qemu",default="qemu-system-aarch64")
    cli.add_argument("--timeout",type=float,default=120)
    cli.add_argument("--mtu",type=int)
    cli.add_argument("--mac",type=str.lower)
    a=cli.parse_args()
    pkg,imp,out=(p.resolve() for p in (a.package,a.driver_manifest,a.out))
    if not a.replay:
        mac=a.mac or "52:54:00:19:26:41"
        mtu=a.mtu if a.mtu is not None else 1410
        require(re.fullmatch(r"(?:[0-9a-f]{2}:){5}[0-9a-f]{2}",mac) and 100<=mtu<=65535,"NIC identity")
        capture(pkg,imp,out,a.qemu,mtu,mac,a.timeout)
    launch=load(out/"launch.json")
    require((a.mac is None or a.mac==launch["mac"]) and (a.mtu is None or a.mtu==launch["mtu"]),"CLI NIC differs")
    result=replay(pkg,imp,out)
    if a.mutation_tests: result["rejected_mutations"]=controls(pkg,imp,out)
    if not a.replay: save(out/"result.json",result)
    print(json.dumps(result,indent=2))
