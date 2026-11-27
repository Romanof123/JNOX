#!/usr/bin/env python3
"""Capture original virtio-net registration, probe/open, RX, stop/remove and rebind on native EL0."""
import argparse
import hashlib
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
from native_amc_roots import linked_roots
from native_el0_replay import PA_MASK, permissions, vread
from qemu_capture import capture_halt, journal_location, request
import verify_linux_source_net_irq as irq
import verify_linux_source_net_rx as rx
from verify_linux_source_net import RAM, NEG, NIC, callback_attempts, load, require, save, sha
from verify_linux_source_net_packets import MAX_LOG, STRIDE, area, challenge, dma_layout, model_command, pcap_frames, socket_frames
from verify_linux_source_net_queues import routine

ORIGINAL=("init_module","cleanup_module","virtnet_validate","virtnet_probe","virtnet_open","virtnet_close","virtnet_remove",
          "virtnet_find_vqs","skb_recv_done","virtnet_poll")
ADAPTER={"__register_virtio_driver":"lifecycle_bus.o","unregister_virtio_driver":"lifecycle_bus.o",
         "register_netdevice":"lifecycle_netdev.o","jnox_lifecycle_net_up_v1":"lifecycle_netdev.o","jnox_lifecycle_net_down_v1":"lifecycle_netdev.o",
         "gro_receive_skb":"rx_skb.o","virtqueue_add_inbuf_premapped":"rx_virtqueue.o"}
TABLES={"virtio_net_driver":("id_table","virtnet_validate","virtnet_probe","virtnet_remove"),
        "virtnet_netdev":("virtnet_open","virtnet_close")}
TRACE=irq.TRACE
DRIVER=("jnox_lifecycle_driver","jnox_lifecycle_ops","jnox_lifecycle_state","jnox_lifecycle_netdev_journal",
        "jnox_lifecycle_registered_driver","jnox_lifecycle_tx_param","napi_tx")
PEER=("jnox_lifecycle_peer","jnox_lifecycle_peer_frames","jnox_lifecycle_peer_ack","jnox_lifecycle_peer_second")
STAGES=("first-probed","first-open","first-ack","first-removed","second-probed","second-open","second-ack","final")


def symbol(elf,name):
    hits=elf.get_section_by_name(".symtab").get_symbol_by_name(name) or []
    hits=[s for s in hits if isinstance(s["st_shndx"],int)]
    require(len(hits)==1,"missing unique native symbol "+name)
    return hits[0]


def data(elf,s):
    section=elf.get_section(s["st_shndx"])
    offset=s["st_value"]-section["sh_addr"]
    return section.data()[offset:offset+s["st_size"]]


def layout(pkg,imp,*,journal_names=(DRIVER,PEER),adapters=ADAPTER):
    manifest=load(pkg/"manifest.json")
    m=load(imp)
    require(m["mode"]=="lifecycle-service" and m["callback_input"]=="virtio_net.packet.o" and
            m["source"]["sha256"]==m["upstream"]["sha256"]==rx.PIN and m["upstream"]["commit"]==
            "4700e3adf3b24b39dd4929e9c6d62811ba3ccea8","pinned whole Linux TU")
    require(manifest["linux_source_net_lifecycle"]==dict(driver_slot=0,peer_slot=1,driver="virtio_net_driver",
            netdev_ops="virtnet_netdev",intid=79,syscalls={"driver_ok_empty_queues":177,"status":160,"release":156})
            and len(manifest["slots"])==2 and manifest["policy"]=="fifo","native lifecycle profile")
    kernel=pkg/"kernel.elf"
    k=kernel.read_bytes()
    require(sha(kernel)==manifest["kernel_elf_sha256"],"kernel ELF identity")
    for slot in manifest["slots"]:
        path=pkg/slot["elf"]
        require(sha(path)==slot["elf_sha256"] and path.read_bytes() in k,"embedded native role")
    for f,expected in (("virtio_net.original.o",m["objects"]["virtio_net.original.o"]["sha256"]),
                       ("virtio_net.packet.o",m["driver_object"]["sha256"]),("virtio_net.callback.o",m["callback_object"]["sha256"]),
                       ("service.map",m["link_map_sha256"])):
        require(sha(imp.parent/f)==expected,"whole import artifact "+f)
    require(sha(pkg/"user.elf")==m["service"]["sha256"],"driver import differs from package")
    linkmap=(imp.parent/"service.map").read_text().splitlines()
    code={}
    pointers={}
    with (pkg/"user.elf").open("rb") as f,(imp.parent/"virtio_net.packet.o").open("rb") as g:
        elf,original=ELFFile(f),ELFFile(g)
        param=symbol(elf,"napi_tx")
        require(data(elf,param)==b"\x01" and data(original,symbol(original,"napi_tx"))==b"\x01",
                "original TX NAPI module parameter default")
        for name in (*ORIGINAL,*adapters):
            sym,body=routine(elf,name)
            objname=adapters.get(name,"virtio_net.packet.o")
            supplier=adapters.get(name,"virtio_net.callback.o")
            with (imp.parent/objname).open("rb") as h:
                obj=ELFFile(h)
                ref,raw=routine(obj,name)
                section=obj.get_section(ref["st_shndx"]).name
                rel={(r["r_offset"]-ref["st_value"])//4 for sec in obj.iter_sections() if isinstance(sec,RelocationSection) and
                     sec["sh_info"]==ref["st_shndx"] for r in sec.iter_relocations()}
                require(len(body)==len(raw) and all(raw[i:i+4]==body[i:i+4] for i in range(0,len(body),4) if i//4 not in rel)
                        and sum(l.strip().endswith(f"{supplier}:({section})") for l in linkmap)==1,
                        "linked instruction provenance "+name)
            code[name]=dict(entry=sym["st_value"],size=len(body),prefix=body[:16].hex(),peer=None)
        for name,fields in TABLES.items():
            ts=symbol(original,name)
            linked=symbol(elf,name)
            section=original.get_section(ts["st_shndx"])
            require(sum(l.strip().endswith(f"virtio_net.callback.o:({section.name})") for l in linkmap)==1,
                    "original linked table "+name)
            with (imp.parent/"virtio_net.callback.o").open("rb") as h:
                promoted=ELFFile(h)
                require(data(original,ts)==data(promoted,symbol(promoted,name)),"export changed original table "+name)
            members={}
            for target in fields:
                dst=symbol(original,target)
                rows=[]
                for sec in original.iter_sections():
                    if not isinstance(sec,RelocationSection) or sec["sh_info"]!=ts["st_shndx"]:
                        continue
                    syms=original.get_section(sec["sh_link"])
                    for r in sec.iter_relocations():
                        rs=syms.get_symbol(r["r_info_sym"])
                        if (r["r_info_type"]==257 and rs["st_shndx"]==dst["st_shndx"] and
                            rs["st_value"]+r["r_addend"]==dst["st_value"]):
                            rows.append(r["r_offset"]-ts["st_value"])
                require(len(rows)==1 and 0<=rows[0]<=linked["st_size"]-8,"original table ABS64 field "+target)
                addr=symbol(elf,target)["st_value"]
                require(struct.unpack_from("<Q",data(elf,linked),rows[0])[0]==addr,"linked table target "+target)
                members[target]=(rows[0],addr)
            pointers[name]=dict(address=linked["st_value"],size=linked["st_size"],members=members)
    roots=linked_roots(kernel,2)
    ring,buf=dma_layout(pkg)
    journals=[{n:journal_location(pkg/manifest["slots"][slot]["elf"],n) for n in names}
              for slot,names in enumerate(journal_names)]
    faults={}
    for n in ("jnox_rx_napi_fault","jnox_rx_poll_fault_id","jnox_lifecycle_fault","jnox_lifecycle_fault_id",
              "jnox_lifecycle_bus_fault","jnox_lifecycle_platform_fault","jnox_lifecycle_netdev_fault"):
        span=journal_location(pkg/"user.elf",n,optional=True)
        if span:
            faults[n]=span
    return dict(roots=roots,journals=journals,faults=faults,code=code,tables=pointers,ring=ring,buffers=buf,
                kernel_irq=journal_location(kernel,"jnox_linux_net_irq_metrics"),
                driver_sha256=sha(pkg/"user.elf"),kernel_sha256=sha(kernel),import_sha256=sha(imp))


def command(binary,out,kernel,fd,mac,mtu,L):
    ranges=",".join(f"0x{v['entry']:x}+0x{v['size']:x}" for v in L["code"].values())
    return model_command(binary,out,kernel,fd,mac,mtu,{"filter":ranges})


def unpack(raw):
    return list(struct.unpack(f"<{len(raw)//8}Q",raw))


def state(mem,L):
    users=[]
    private=[]
    for slot,(root,syms) in enumerate(zip(L["roots"],L["journals"])):
        user={}
        owned=set()
        for name,(va,size) in syms.items():
            require(0x400000<=va<va+size<=0x800000,"private fixture extent")
            for page in range(va//4096,(va+size-1)//4096+1):
                owned.add(int(permissions(mem,root,page*4096,"user rw"),16)&PA_MASK)
            user[name]=vread(mem,root,va,size)
        users.append(user)
        private.append(owned)
    tables={}
    for name,t in L["tables"].items():
        tables[name]=vread(mem,L["roots"][0],t["address"],t["size"])
        for offset,target in t["members"].values():
            require(struct.unpack_from("<Q",tables[name],offset)[0]==target,"live callback table differs")
    for n,(addr,size) in L["faults"].items():
        require(not any(vread(mem,L["roots"][0],addr,size)),"executed an unsupported branch "+n)
    id_addr=L["tables"]["virtio_net_driver"]["members"]["id_table"][1]
    require(vread(mem,L["roots"][0],id_addr,8)==struct.pack("<II",1,0xffffffff),"live VirtIO ID match")
    ring=L["ring"]
    buffers=L["buffers"]
    kernel_irq,n=L["kernel_irq"]
    protected=set()
    for a,sz in ((ring,8192),(buffers,64*STRIDE),(kernel_irq,n)):
        protected.update(range(a//4096*4096,(a+sz+4095)//4096*4096,4096))
    for slot,root in enumerate(L["roots"]):
        for a in protected:
            permissions(mem,root,a,"kernel rw")
        permissions(mem,root,NIC&~4095,"kernel rw",device=True)
        forbidden=protected|private[1-slot]
        current={root&PA_MASK}
        for depth in range(3):
            children=set()
            for t in current:
                for (desc,) in struct.iter_unpack("<Q",mem.read(t,4096)):
                    if not desc&1:
                        continue
                    require(desc&3==3,"unexpected block mapping")
                    if depth<2:
                        children.add(desc&PA_MASK)
                    elif desc&64:
                        require(desc&PA_MASK not in forbidden,"EL0 alias of peer or protected DMA")
            current=children
    return dict(driver=unpack(users[0][DRIVER[0]]),ops=unpack(users[0][DRIVER[1]]),
                transport=unpack(users[0][DRIVER[2]]),registry=unpack(users[0][DRIVER[3]]),
                registered=unpack(users[0][DRIVER[4]])[0],tx_setting=unpack(users[0][DRIVER[5]]),tx=users[0][DRIVER[6]],peer=unpack(users[1][PEER[0]]),
                wire=[users[1][name] for name in PEER[1:]],irq=unpack(mem.read(kernel_irq,n)),
                rings=(area(mem,ring,4096),area(mem,ring+4096,4096)),buffers=area(mem,buffers,64*STRIDE))


def capture(pkg,imp,out,qemu,mtu,mac,timeout,through_open=False):
    L=layout(pkg,imp)
    binary=shutil.which(qemu)
    require(binary,"missing QEMU")
    out.mkdir(parents=True,exist_ok=False)
    (out/"net.events").write_text(TRACE)
    nonces=[]
    while len(nonces)<2:
        n=secrets.token_bytes(8)
        if any(n) and n not in nonces:
            nonces.append(n)
    (out/"nonces.bin").write_bytes(b"".join(nonces))
    addr=bytes.fromhex(mac.replace(":",""))
    deadline=time.monotonic()+timeout
    host,nic=socket.socketpair()
    with host,nic,(out/"stderr.log").open("xb") as err,(out/"host.bin").open("xb") as sent:
        fd=nic.fileno()
        argv=command(binary,out,pkg/"kernel.elf",fd,mac,mtu,L)
        with subprocess.Popen(argv,pass_fds=(fd,),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,bufsize=0) as proc:
            nic.close()
            class Pipe:
                readline=proc.stdout.readline
                write=proc.stdin.write
            q=Pipe()
            watchdog=threading.Timer(timeout+25,proc.kill)
            watchdog.start()
            save(out/"launch.json",dict(schema=1,argv=argv,kernel=str(pkg/"kernel.elf"),out=str(out),pid=proc.pid,fd=fd,
                                        mtu=mtu,mac=mac,import_sha256=sha(imp),driver_sha256=L["driver_sha256"],
                                        kernel_sha256=L["kernel_sha256"],qemu_sha256=sha(Path(binary))))
            def read(pa,n):
                request(q,"pmemsave",val=pa,size=n,filename=str(out/"probe.bin"))
                return (out/"probe.bin").read_bytes()
            def field(slot,offset):
                root=L["roots"][slot]
                addr=L["journals"][slot][(DRIVER if slot==0 else PEER)[0]][0]+8*offset
                mem=RAM({},read)
                return int.from_bytes(vread(mem,root,addr,8),"little") if mem.leaf(root,addr) else 0
            def blocked(token):
                k=unpack(read(*L["kernel_irq"]))
                return k[19]==token and k[22]==1
            def uart():
                path=out/"uart.log"
                return path.read_bytes() if path.exists() else b""
            def wait(stage,predicate):
                nonlocal current
                current=stage
                while True:
                    buf=uart()
                    exits=re.findall(rb"(?m)^NATIVE EL0 EXIT PID ([01]) (-?\d+)\r?$",buf)
                    require(not any(int(code) for _,code in exits) and b"NATIVE EL0 FAULT" not in buf,
                            "native fault or failed exit while waiting for "+stage)
                    if predicate():
                        return
                    require(proc.poll() is None and time.monotonic()<deadline,"deadline while waiting for "+stage)
                    require(not (out/"machine.log").exists() or (out/"machine.log").stat().st_size<=MAX_LOG,"trace limit")
                    time.sleep(.003)
            def freeze(stage):
                where=out/stage
                where.mkdir()
                capture_halt(q,where)
                mem=RAM({},read)
                try:
                    return state(mem,L)
                finally:
                    save(where/"ram.json",mem.records)
            def send(frame):
                blob=len(frame).to_bytes(4,"big")+frame
                host.settimeout(max(.001,deadline-time.monotonic()))
                host.sendall(blob)
                sent.write(blob)
                sent.flush()
            current="boot"
            try:
                require("QMP" in json.loads(q.readline()),"QMP greeting")
                request(q,"qmp_capabilities")
                for g,prefix in enumerate(("first","second")):
                    d,p=16*g,12*g
                    wait(prefix+"-probed",lambda:field(0,d+1)==1)
                    shot=freeze(prefix+"-probed")
                    require(shot["driver"][d+1]==1,"missed post-probe checkpoint")
                    token=shot["driver"][d+2]
                    request(q,"cont")
                    wait(prefix+"-open",lambda:field(0,d+1)==2 and blocked(token))
                    freeze(prefix+"-open")
                    if through_open:
                        request(q,"quit")
                        require(proc.wait(timeout=5)==0,"QEMU shutdown after first open")
                        (out/"probe.bin").unlink(missing_ok=True)
                        return
                    frames=rx.frames(addr,nonces[g])[:(80 if not g else 1)]
                    for f in frames[:16]:
                        send(f)
                    request(q,"cont")
                    for seq,f in enumerate(frames[16:],16):
                        if seq%8==0:
                            wait("peer RX",lambda:field(1,p+5)>=seq)
                        send(f)
                    wait(prefix+"-ack",lambda:field(1,p+5)==len(frames) and blocked(token))
                    freeze(prefix+"-ack")
                    request(q,"cont")
                    send(challenge(addr,nonces[g],True))
                    if not g:
                        wait("first-removed",lambda:field(0,1)==4)
                        freeze("first-removed")
                        request(q,"cont")
                wait("final",lambda:len(re.findall(rb"(?m)^NATIVE EL0 EXIT PID [01] 0\r?$",uart()))==2)
                freeze("final")
                request(q,"quit")
                require(proc.wait(timeout=5)==0 and host.recv(1)==b"","QEMU exit or unexpected egress")
                (out/"probe.bin").unlink(missing_ok=True)
            except Exception as exc:
                diagnostic=dict(stage=current,error=repr(exc),returncode=proc.poll())
                if proc.poll() is None:
                    try:
                        request(q,"stop")
                        diagnostic["status"]=request(q,"query-status")
                    except Exception as error:
                        diagnostic["stop_error"]=repr(error)
                    else:
                        mem=RAM({},read)
                        for slot,syms in enumerate(L["journals"]):
                            group=dict(syms,**L["faults"]) if not slot else syms
                            for name,(va,size) in group.items():
                                try:
                                    raw=vread(mem,L["roots"][slot],va,size)
                                    if name in (*DRIVER,PEER[0],*L["faults"]):
                                        diagnostic[name]=unpack(raw) if size%8==0 else raw.hex()
                                except Exception as error:
                                    diagnostic.setdefault("read_errors",{})[name]=repr(error)
                        for name,(a,n) in {"irq":L["kernel_irq"],"rings":(L["ring"],8192)}.items():
                            try:
                                raw=area(mem,a,n)
                                if name=="irq":
                                    diagnostic[name]=unpack(raw)
                            except Exception as error:
                                diagnostic.setdefault("read_errors",{})[name]=repr(error)
                        save(out/"failure-ram.json",mem.records)
                diagnostic["uart_tail"]=uart()[-2048:].decode(errors="replace")
                save(out/"failure.json",diagnostic)
                raise
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
                watchdog.cancel()
                watchdog.join()


def first_open_evidence(out,L,shots):
    before,after=[shots[s] for s in STAGES[:2]]
    token=before["driver"][2]
    table=L["tables"]["virtio_net_driver"]
    c=L["code"]
    expected=[table["address"],table["members"]["id_table"][1],
              *(c[n]["entry"] for n in ("virtnet_validate","virtnet_probe","virtnet_open","virtnet_close","virtnet_remove"))]
    require(token>0 and before["driver"][1]==1 and after["driver"][1:3]==[2,token] and
            before["ops"][:12]==expected+[1,0,0,0,token] and after["ops"][12:24]==expected+[1,1,0,0,token],
            "first probe and open operation identity")
    require(before["transport"][:5:3]==[15,0] and after["transport"][5]==15 and after["transport"][8:10]==[32,token]
            and not any(before["rings"][0][:584]) and struct.unpack_from("<H",before["rings"][0],586)[0]==0
            and after["irq"][19]==token and after["irq"][22]==1,"first post-probe / post-open device state")
    for s,up in ((before,0),(after,1)):
        registry=s["registry"]
        setting=s["tx_setting"]
        require(s["registered"]==expected[0] and setting[:4]==[L["journals"][0]["napi_tx"][0],1,0,1] and
                setting[4]!=0 and setting[5:7]==[0,token] and s["tx"]==b"\0","live driver registry or TX NAPI parameter")
        require(len(registry)==32 and registry[2]==1 and registry[8]==registry[22]!=0 and
                registry[13:16]==[L["tables"]["virtnet_netdev"]["address"],expected[4],expected[5]] and
                registry[23]==up and not registry[31],"registered netdevice, original ops or administrative state")
    irq.poll_gate.ring_state("ready",after["rings"],after["buffers"],L["buffers"],[],0)
    log=out/"machine.log"
    require(log.stat().st_size<=MAX_LOG,"trace size")
    lines=log.read_bytes().splitlines()
    first={}
    for name in ("init_module","__register_virtio_driver","virtnet_validate","virtnet_probe","virtnet_find_vqs","register_netdevice",
                 "jnox_lifecycle_net_up_v1","virtnet_open","virtqueue_add_inbuf_premapped"):
        _,indices=callback_attempts(lines,c[name]["entry"])
        require(indices,"missing executed first lifecycle entry "+name)
        rx.verify_execution(lines,c[name],len(indices))
        first[name]=indices[0]
    marks=list(first.values())
    status=[i for i,l in enumerate(lines) if re.search(rb"memory_region_ops_write cpu 0 mr \S+ addr 0x0*a003e70 value 0x0*f size 4\b",l,re.I)]
    require(marks==sorted(marks) and status and marks[5]<status[0]<marks[6],"original registration and DRIVER_OK before open/post")
    reads=[]
    for i,line in enumerate(lines[:marks[6]]):
        m=re.search(rb"memory_region_ops_read cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size 4",line)
        if m:
            reads.append((i,int(m[1],16)-NIC,int(m[2],16)))
    require(any(r==8 and v==1 and i<marks[2] for i,r,v in reads) and
            any(r==12 and i<marks[2] for i,r,v in reads) and
            any(r==112 and v==15 and i>status[0] for i,r,v in reads),"device ID/vendor or DRIVER_OK readback")
    return dict(scope="first probe through open",token=token,first_posts=32,executed_entries=first,
                driver_ok_line=status[0],checkpoints=list(STAGES[:2]))


def replay(pkg,imp,out,through_open=False):
    L=layout(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["schema"]==1 and all(launch[k]==L[k] for k in ("import_sha256","kernel_sha256","driver_sha256"))
            and (out/"net.events").read_text()==TRACE and launch["argv"]==
            command(launch["argv"][0],Path(launch["out"]),Path(launch["kernel"]),launch["fd"],
                    launch["mac"],launch["mtu"],L),"lifecycle launch identity")
    shots={}
    for name in (STAGES[:2] if through_open else STAGES):
        path=out/name
        q,h=load(path/"qmp.json"),load(path/"halted.json")
        require(q["status"]["running"] and q["stop_return"]=={} and [c["cpu-index"] for c in q["cpus"]]==[0]
                and h["status"]=="paused" and not h["running"],"QMP stopped CPU0")
        mem=RAM(load(path/"ram.json"))
        shots[name]=state(mem,L)
        require(mem.used==set(mem.records),"unexpected physical memory read")
    if through_open:
        return first_open_evidence(out,L,shots)
    first_open_evidence(out,L,shots)
    ns=(out/"nonces.bin").read_bytes()
    require(len(ns)==16 and ns[:8]!=ns[8:] and any(ns[:8]) and any(ns[8:]),"external nonces")
    addr=bytes.fromhex(launch["mac"].replace(":",""))
    frames=[rx.frames(addr,ns[:8]),rx.frames(addr,ns[8:])[:1]]
    acks=[challenge(addr,nonce,True) for nonce in (ns[:8],ns[8:])]
    expected=[*frames[0],acks[0],*frames[1],acks[1]]
    actual=pcap_frames(out/"wire.pcap")
    require(socket_frames((out/"host.bin").read_bytes())==expected and len(actual)==len(expected) and
            all(p in (f,bytes(10)+f,bytes(12)+f) for f,p in zip(expected,actual)),"socket and NIC PCAP differ")
    last=shots["final"]
    wire=last["wire"]
    require(wire==[b"".join(f.ljust(91,b"\0") for f in frames[0]),acks[0],frames[1][0]+acks[1]],
            "isolated peer Ethernet history")
    tables=L["tables"]
    c=L["code"]
    target=[tables["virtio_net_driver"]["address"],tables["virtio_net_driver"]["members"]["id_table"][1],
            *(c[n]["entry"] for n in ("virtnet_validate","virtnet_probe","virtnet_open","virtnet_close","virtnet_remove"))]
    generations=[]
    for g,prefix in enumerate(("first","second")):
        probed,opened,ack=[shots[f"{prefix}-{name}"] for name in ("probed","open","ack")]
        d=last["driver"][16*g:16*(g+1)]
        p=last["peer"][12*g:12*(g+1)]
        token=d[2]
        require(d[:2]==[0x4c49464544525631,100] and token>0 and d[3:10]==[int.from_bytes(addr,"big"),
                int.from_bytes(ns[8*g:8*g+8],"big"),len(frames[g]),sum(map(len,frames[g])),(1<<32)|40,launch["mtu"],1]
                and d[10]==60 and d[15]==0 and (not g or d[11]==NEG-116),"driver lifetime or stale queue")
        require(p==[0x4c49464550454552,100,*d[2:7],NEG-13,NEG-13,NEG-13,60,0],"peer generation and authority")
        require(probed["registered"]==opened["registered"]==target[0] and
                probed["tx_setting"][0:4]==[L["journals"][0]["napi_tx"][0],int(g==0),0,g+1] and
                probed["tx_setting"][4]!=0 and probed["tx_setting"][5:7]==[0,token] and probed["tx"]==b"\0",
                "upstream driver registration and selected module parameter")
        require(probed["registry"][2:4]==[g+1,g] and probed["registry"][8]==probed["registry"][22]!=0 and
                opened["registry"][13:16]==[tables["virtnet_netdev"]["address"],target[4],target[5]] and
                opened["registry"][23]==1 and ack["registry"][31]==0,"generation registry ownership")
        removed=shots["final" if g else "first-removed"]["registry"]
        require(removed[0:4]==[g+1]*4 and removed[22:24]==[0,0] and not removed[31],"netdevice unregister and free")
        counts=([1,0,0,0] if not g else [2,2,2,1])
        phases=range(7) if not g else (0,1,2,3,6)
        for phase in phases:
            ops=last["ops"][(g*7+phase)*12:(g*7+phase+1)*12]
            t=last["transport"][(g*7+phase)*5:(g*7+phase+1)*5]
            if phase in (1,4): counts[1]+=1
            if phase in (3,5): counts[2]+=1
            if phase==6: counts[3]+=1
            require(ops==target+counts+[token],"registered original operation pointer or lifecycle count")
            if phase==6:
                require(t[:3]==[NEG-19,NEG-116,NEG-116] and t[4]==0,"post-remove device ownership")
            else:
                require(t[0]==15 and t[4]==token and all(q&0xffffffff==0x200001 and q>>32>=32 for q in t[1:3])
                        and (phase!=0 or t[3]==0) and (phase not in (1,4) or t[3]==32),"device queue lifecycle state")
        require(probed["driver"][g*16+1]==1 and not any(probed["rings"][0][:584]) and
                struct.unpack_from("<H",probed["rings"][0],586)[0]==0 and
                opened["driver"][g*16+1]==2 and opened["irq"][19]==token and opened["irq"][22]==1,
                "empty post-probe RX queues or blocked post-open owner")
        irq.poll_gate.ring_state("ready",opened["rings"],opened["buffers"],L["buffers"],[],0)
        irq.poll_gate.ring_state("active",ack["rings"],ack["buffers"],L["buffers"],frames[g],len(frames[g]))
        require(ack["peer"][g*12+5]==len(frames[g]) and ack["irq"][19]==token and ack["irq"][22]==1 and
                ack["irq"][7]>0,"live receive, IRQ or terminal wait")
        generations.append(token)
    require(generations[0]<generations[1] and shots["first-removed"]["driver"][1]==4,"distinct rebind")
    for stage in ("first-removed","final"):
        require(shots[stage]["registered"]==0 and not any(shots[stage]["buffers"]) and shots[stage]["irq"][26]==1,"device removal and buffer retirement")
    path=out/"machine.log"
    require(path.stat().st_size<=MAX_LOG,"emulator trace size")
    lines=path.read_bytes().splitlines()
    starts={}
    fixed={"init_module":2,"cleanup_module":2,"__register_virtio_driver":2,"unregister_virtio_driver":2,
           "register_netdevice":2,"jnox_lifecycle_net_up_v1":3,"jnox_lifecycle_net_down_v1":3,"virtnet_probe":2,"virtnet_remove":2,"virtnet_open":3,"virtnet_close":3,"virtnet_find_vqs":2,
           "gro_receive_skb":83}
    for name,fn in c.items():
        _,indices=callback_attempts(lines,fn["entry"])
        count=fixed.get(name,len(indices))
        require(count>=2,"no two-generation execution for "+name)
        rx.verify_execution(lines,fn,count)
        starts[name]=indices
    writes=[]
    for i,line in enumerate(lines):
        m=re.search(rb"memory_region_ops_write cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size 4",line)
        if m and int(m[1],16)==NIC+112:
            writes.append((i,int(m[2],16)))
    require([v for _,v in writes]==[0,1,3,11,15,0]*2,"two complete hardware status lifecycles")
    for g in range(2):
        reset=writes[g*6+5][0]
        limit=writes[6][0] if not g else len(lines)
        reads=set()
        selected=None
        acknowledged=False
        for line in lines[reset+1:limit]:
            m=re.search(rb"memory_region_ops_(read|write) cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size 4",line)
            if not m:
                continue
            op,r,v=m[1],int(m[2],16)-NIC,int(m[3],16)
            if (op,r,v)==(b"read",112,0):
                acknowledged=True
            elif (op,r)==(b"write",48):
                selected=v
            elif (op,r,v)==(b"read",68,0) and acknowledged:
                reads.add(selected)
        require(acknowledged and {0,1}<=reads,"remove did not confirm hardware and RX/TX queue reset")
    epochs=irq.delivery_epochs(lines)
    irq.wire_delivery(lines,epochs,False)
    for g in range(2):
        live,reset=writes[g*6+4][0],writes[g*6+5][0]
        probe=starts["virtnet_probe"][g]
        opened=starts["virtnet_open"][0 if not g else 2]
        removed=starts["virtnet_remove"][g]
        posted=[i for i in starts["virtqueue_add_inbuf_premapped"] if writes[g*6][0]<i<reset]
        cb=[i for i in starts["skb_recv_done"] if live<i<reset]
        poll=[i for i in starts["virtnet_poll"] if live<i<reset]
        gro=[i for i in starts["gro_receive_skb"] if live<i<reset]
        irqs=[e for e in epochs if live<e["iar"]<reset and e["status"]&1]
        require(starts["init_module"][g]<starts["__register_virtio_driver"][g]<probe and
                opened>starts["jnox_lifecycle_net_up_v1"][0 if not g else 2] and
                removed<starts["cleanup_module"][g]<starts["unregister_virtio_driver"][g],
                "actual module registration, dispatch and unregister")
        require(posted and len(posted)>=32 and probe<starts["virtnet_find_vqs"][g]<starts["register_netdevice"][g]<live<opened<posted[0] and
                posted[0]<irqs[0]["eoi"]<cb[0] and any(cb[0]<i<gro[0] for i in poll) and len(gro)==len(frames[g])+1 and
                gro[-1]<starts["virtnet_close"][0 if not g else 2]<removed<reset,
                "original probe, DRIVER_OK, open, first post, IRQ, poll or remove chronology")
    uart=(out/"uart.log").read_bytes()
    require(b"NATIVE EL0 FAULT" not in uart and set(re.findall(rb"(?m)^NATIVE EL0 EXIT PID ([01]) (-?\d+)\r?$",uart))
            =={(b"0",b"0"),(b"1",b"0")},"native role exits")
    return dict(tokens=generations,matched_driver=hex(target[0]),registry_pointers=[hex(v) for v in target],
                executions={k:len(v) for k,v in starts.items()},hardware_rx_irqs=len(epochs),peer_frames=83,
                checkpoints=list(STAGES))


def controls(pkg,imp,out,through_open):
    L=layout(pkg,imp)
    results=[]
    with tempfile.TemporaryDirectory(prefix="jnox-lifecycle-controls-") as tmp:
        work=Path(tmp)/"capture"
        shutil.copytree(out,work)
        def reject(label,path,payload):
            prev=path.read_bytes()
            try:
                path.write_bytes(payload)
                try:
                    replay(pkg,imp,work,through_open)
                except ValueError:
                    results.append(label)
                else:
                    raise ValueError("accepted corrupted "+label)
            finally:
                path.write_bytes(prev)
        def corrupt(stage,slot,where,size,offset,label):
            path=work/stage/"ram.json"
            records=load(path)
            while size:
                take=min(size,4096-where%4096)
                if offset<take:
                    desc=RAM(records).leaf(L["roots"][slot],where)
                    key=f"{(desc&PA_MASK)+where%4096:x}/{take}"
                    raw=bytearray.fromhex(records[key])
                    raw[offset]^=1
                    records[key]=raw.hex()
                    reject(label,path,json.dumps(records).encode())
                    return
                where+=take
                size-=take
                offset-=take
            raise ValueError("missing mutation offset")
        t=L["tables"]["virtnet_netdev"]
        corrupt("first-probed",0,t["address"],t["size"],t["members"]["virtnet_open"][0],"live original ndo_open pointer")
        a,n=L["journals"][0][DRIVER[3]]
        corrupt("first-open",0,a,n,22*8,"registered netdevice identity")
        log=work/"machine.log"
        lines=log.read_bytes().splitlines(keepends=True)
        _,indices=callback_attempts(lines,L["code"]["virtnet_probe"]["entry"])
        i=indices[0]
        # Replacing the unique PC prevents a second generation from masking the missing first probe.
        reject("missing first original probe entry",log,b"".join(lines[:i]+lines[i+1:]))
        if not through_open:
            a,n=L["journals"][1][PEER[3]]
            corrupt("final",1,a,n,22,"second-generation private RX payload")
    return results


if __name__=="__main__":
    cli=argparse.ArgumentParser(description=__doc__)
    for n in ("package","driver-manifest","out"):
        cli.add_argument("--"+n,type=Path,required=True)
    cli.add_argument("--replay",action="store_true")
    cli.add_argument("--mutation-tests",action="store_true")
    cli.add_argument("--through-open",action="store_true",help="capture or replay through the first live post-open queue")
    cli.add_argument("--timeout",type=float,default=120)
    cli.add_argument("--qemu",default="qemu-system-aarch64")
    cli.add_argument("--mtu",type=int)
    cli.add_argument("--mac",type=str.lower)
    a=cli.parse_args()
    pkg,imp,out=(p.resolve() for p in (a.package,a.driver_manifest,a.out))
    if not a.replay:
        mac=a.mac or "52:54:00:19:26:41"
        mtu=1410 if a.mtu is None else a.mtu
        require(re.fullmatch(r"(?:[0-9a-f]{2}:){5}[0-9a-f]{2}",mac) and 100<=mtu<=65535,"NIC identity")
        capture(pkg,imp,out,a.qemu,mtu,mac,a.timeout,a.through_open)
    launch=load(out/"launch.json")
    require((a.mac is None or a.mac==launch["mac"]) and (a.mtu is None or a.mtu==launch["mtu"]),"CLI NIC differs")
    result=replay(pkg,imp,out,a.through_open)
    if a.mutation_tests:
        result["rejected_mutations"]=controls(pkg,imp,out,a.through_open)
    if not a.replay:
        save(out/"result.json",result)
    print(json.dumps(result,indent=2))
