#!/usr/bin/env python3
"""Capture and replay simultaneous native Linux virtio-net RX/TX on one QEMU NIC."""
import argparse
import json
from pathlib import Path
import re
import secrets
import select
import shutil
import socket
import struct
import subprocess
import threading
import time

from elftools.elf.elffile import ELFFile
from elftools.elf.relocation import RelocationSection
from native_amc_roots import linked_roots
from native_el0_replay import PA_MASK, permissions, vread
from qemu_capture import capture_halt, journal_location, request
import verify_linux_source_net_irq as irq
import verify_linux_source_net_rx as rx
from verify_linux_source_net import RAM, NIC, NEG, require, load, save, sha
from verify_linux_source_net_lifecycle import command, data, symbol, unpack
from verify_linux_source_net_packets import MAX_LOG, STRIDE, area, challenge, dma_layout, exact, pcap_frames, socket_frames
from verify_linux_source_net_queues import routine

STAGES = {"ready":3, "held":4, "overlap":5, "completed":5, "ack":6, "final":100}
DRIVER = ("jnox_duplex_driver","jnox_duplex_tx","jnox_duplex_tx_queue","jnox_duplex_rx_queue")
PEER = ("jnox_duplex_peer","jnox_duplex_peer_requests","jnox_duplex_peer_replies","jnox_duplex_peer_ack")
ORIGINAL = ("virtnet_probe","virtnet_open","skb_recv_done","virtnet_poll","start_xmit","virtnet_close","virtnet_remove")


def layout(pkg, imp):
    m, native = load(imp), load(pkg/"manifest.json")
    require(m["mode"]=="lifecycle-duplex-service" and native["linux_source_net_duplex"] and
            m["upstream"]["sha256"]==m["source"]["sha256"]==rx.PIN and m["callback_input"]=="virtio_net.packet.o",
            "duplex original driver input")
    kernel=pkg/"kernel.elf"
    user=pkg/"user.elf"
    require(native["kernel_elf_sha256"]==sha(kernel) and native["policy"]=="fifo" and len(native["slots"])==2
            and sha(user)==m["service"]["sha256"],"duplex image identity")
    for slot in native["slots"]:
        f=pkg/slot["elf"]
        require(sha(f)==slot["elf_sha256"] and f.read_bytes() in kernel.read_bytes(),"embedded slot identity")
    for name,expected in (("virtio_net.original.o",m["objects"]["virtio_net.original.o"]["sha256"]),
                          ("virtio_net.packet.o",m["driver_object"]["sha256"]),("virtio_net.callback.o",m["callback_object"]["sha256"]),
                          ("service.map",m["link_map_sha256"])):
        require(sha(imp.parent/name)==expected,"import identity "+name)
    tx_irq = native["slots"][0]["argv"][1:] == ["t"]
    require(tx_irq == bool(native.get("linux_source_net_tx_irq")), "TX IRQ package feature and arguments")
    code={}
    linkmap=(imp.parent/"service.map").read_text().splitlines()
    with user.open("rb") as uf,(imp.parent/"virtio_net.packet.o").open("rb") as of:
        elf,obj=ELFFile(uf),ELFFile(of)
        for name in ORIGINAL + (("skb_xmit_done",) if tx_irq else ()):
            sym,body=routine(elf,name)
            ref,raw=routine(obj,name)
            reloc={r["r_offset"]-ref["st_value"] for section in obj.iter_sections() if isinstance(section,RelocationSection)
                   and section["sh_info"]==ref["st_shndx"] for r in section.iter_relocations()}
            sect=obj.get_section(ref["st_shndx"]).name
            require(len(body)==len(raw) and all(body[i:i+4]==raw[i:i+4] for i in range(0,len(raw),4) if i not in reloc)
                    and sum(l.strip().endswith(f"virtio_net.callback.o:({sect})") for l in linkmap)==1,"original instructions "+name)
            code[name]=dict(entry=sym["st_value"],size=len(body),prefix=body[:16].hex(),peer=None)
        tx_callback=symbol(elf,"skb_xmit_done")["st_value"]
        tab=symbol(elf,"virtnet_netdev")
        orig=symbol(obj,"virtnet_netdev")
        target=symbol(obj,"start_xmit")
        offsets=[]
        for sec in obj.iter_sections():
            if isinstance(sec,RelocationSection) and sec["sh_info"]==orig["st_shndx"]:
                for r in sec.iter_relocations():
                    rs=obj.get_section(sec["sh_link"]).get_symbol(r["r_info_sym"])
                    if r["r_info_type"]==257 and rs["st_shndx"]==target["st_shndx"] and rs["st_value"]+r["r_addend"]==target["st_value"]:
                        offsets.append(r["r_offset"]-orig["st_value"])
        require(len(offsets)==1 and struct.unpack_from("<Q",data(elf,tab),offsets[0])[0]==code["start_xmit"]["entry"],
                "upstream ndo_start_xmit relocation")
        ops=tab["st_value"]
    with (pkg/native["slots"][1]["elf"]).open("rb") as pf:
        peer_elf=ELFFile(pf)
        for seg in peer_elf.iter_segments():
            if seg["p_type"]!="PT_LOAD" or not seg["p_flags"]&1:
                continue
            for fn in code.values():
                offset=fn["entry"]-seg["p_vaddr"]
                if 0<=offset<seg["p_filesz"]:
                    fn["peer"]=seg.data()[offset:offset+16].hex()
    roots=linked_roots(kernel,2)
    ring,buf=dma_layout(pkg)
    journal=[{n:journal_location(pkg/native["slots"][slot]["elf"],n) for n in names}
             for slot,names in enumerate((DRIVER + (("jnox_duplex_tx_irq",) if tx_irq else ()),PEER + (("jnox_duplex_peer_tx_irq_denial",) if tx_irq else ())))]
    tx_events = journal_location(kernel,"jnox_linux_net_tx_irq_events") if tx_irq else None
    return dict(tx_events=tx_events,code=code,ops=ops,tx_callback=tx_callback,ndo=ops+offsets[0],roots=roots,journal=journal,ring=ring,buf=buf,
                irq=journal_location(kernel,"jnox_linux_net_irq_metrics"),
                grants=journal_location(kernel,"jnox_linux_net_lifecycle_grants"))


def snapshot(mem,L):
    groups=[]
    pages=[]
    protected=set(range(L["ring"],L["ring"]+8192,4096))|set(range(L["buf"],L["buf"]+64*STRIDE,4096))
    irq_addr,irq_size=L["irq"]
    grant_addr,grant_size=L["grants"]
    require(grant_size==32*24,"protected grant table ABI")
    for addr,size in (L["irq"],L["grants"]) + ((L["tx_events"],) if L["tx_events"] else ()):
        protected.update(range(addr&~4095,(addr+size+4095)&~4095,4096))
    grants=list(struct.iter_unpack("<QQQ",mem.read(grant_addr,grant_size)))
    for token,va,n in grants:
        if token:
            require(0x400000<=va<va+n<=0x800000 and n==1530,"RX grant range")
            for page in range(va//4096,(va+n-1)//4096+1):
                permissions(mem,L["roots"][0],page*4096,"user rw")
    for root,syms in zip(L["roots"],L["journal"]):
        group={}
        private=set()
        for name,(addr,n) in syms.items():
            group[name]=vread(mem,root,addr,n)
            for page in range(addr//4096,(addr+n-1)//4096+1):
                private.add(int(permissions(mem,root,page*4096,"user rw"),16)&PA_MASK)
        pages.append(private)
        groups.append(group)
    for i,root in enumerate(L["roots"]):
        for a in protected:
            permissions(mem,root,a,"kernel rw")
        tables={root&PA_MASK}
        for depth in range(3):
            next_tables=set()
            for pa in tables:
                for (pte,) in struct.iter_unpack("<Q",mem.read(pa,4096)):
                    if pte&1:
                        require(pte&3==3,"unexpected block mapping")
                        if depth<2:
                            next_tables.add(pte&PA_MASK)
                        elif pte&64:
                            require(pte&PA_MASK not in protected|pages[1-i],"user alias of peer or DMA")
            tables=next_tables
    require(int.from_bytes(vread(mem,L["roots"][0],L["ndo"],8),"little")==L["code"]["start_xmit"]["entry"],"live netdev operation")
    events = mem.read(*L["tx_events"]) if L["tx_events"] else b""
    callbacks = groups[0].get("jnox_duplex_tx_irq", b"")
    peer_denial = int.from_bytes(groups[1].get("jnox_duplex_peer_tx_irq_denial", b""),"little",signed=True)
    return dict(peer_denial=peer_denial,events=list(struct.iter_unpack("<8Q",events)), callbacks=list(struct.iter_unpack("<8Q",callbacks)), grants=grants,driver=unpack(groups[0][DRIVER[0]]),tx=unpack(groups[0][DRIVER[1]]),peer=unpack(groups[1][PEER[0]]),
                qtx=unpack(groups[0][DRIVER[2]]),qrx=unpack(groups[0][DRIVER[3]]),wire=[groups[1][n] for n in PEER[1:]],irq=unpack(mem.read(irq_addr,irq_size)),
                rings=[area(mem,L["ring"]+i*4096,4096) for i in range(2)],buffers=area(mem,L["buf"],64*STRIDE))


def protocol(mac,nonce):
    requests=rx.frames(mac,nonce)
    replies=[]
    for seq,f in enumerate(requests):
        reply=bytearray(f)
        reply[:22]=f[6:12]+f[:6]+b"\x88\xb6JNOXRESP"
        for i in range(34,len(reply)):
            reply[i]=f[i]^nonce[(i-34+seq)&7]^0xa5
        replies.append(bytes(reply))
    return requests,replies,challenge(mac,nonce,True)


def capture(pkg,imp,out,qemu,mac,mtu,timeout):
    L=layout(pkg,imp)
    binary=shutil.which(qemu)
    require(binary,"QEMU missing")
    out.mkdir(parents=True,exist_ok=False)
    nonce=secrets.token_bytes(8)
    (out/"nonce.bin").write_bytes(nonce)
    frames,replies,ack=protocol(bytes.fromhex(mac.replace(":","")),nonce)
    (out/"net.events").write_text(irq.TRACE)
    host,nic=socket.socketpair()
    deadline=time.monotonic()+timeout
    with host,nic,(out/"stderr.log").open("xb") as err,(out/"ingress.bin").open("xb") as ins,(out/"egress.bin").open("xb") as outs:
        fd=nic.fileno()
        argv=command(binary,out,pkg/"kernel.elf",fd,mac,mtu,L)
        with subprocess.Popen(argv,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,pass_fds=(fd,),bufsize=0) as proc:
            nic.close()
            class Pipe:
                readline=proc.stdout.readline
                write=proc.stdin.write
            q=Pipe()
            timer=threading.Timer(timeout+20,proc.kill)
            timer.start()
            save(out/"launch.json",dict(argv=argv,fd=fd,mac=mac,mtu=mtu,out=str(out),kernel=str(pkg/"kernel.elf"),
                                        kernel_sha256=sha(pkg/"kernel.elf"),import_sha256=sha(imp),schema=1))
            def read(pa,n):
                request(q,"pmemsave",val=pa,size=n,filename=str(out/"probe.bin"))
                return (out/"probe.bin").read_bytes()
            def uart():
                path=out/"uart.log"
                return path.read_bytes() if path.exists() else b""
            def send(f):
                packet=struct.pack("!I",len(f))+f
                host.sendall(packet)
                ins.write(packet)
                ins.flush()
            def receive(seq):
                header=exact(host,4,deadline)
                n=struct.unpack("!I",header)[0]
                require(0<n<=1518,"unexpected egress size")
                frame=exact(host,n,deadline)
                outs.write(header+frame)
                outs.flush()
                require(frame==replies[seq],"guest egress mismatch "+str(seq))
            def stop(stage):
                va=L["journal"][0][DRIVER[0]][0]
                root=L["roots"][0]
                while True:
                    buf=uart()
                    require(b"NATIVE EL0 FAULT" not in buf and not re.search(rb"NATIVE EL0 EXIT PID [01] (?!0\r?$)-?[1-9]\d*",buf,re.M),"native fault")
                    mem=RAM({},read)
                    if stage=="final":
                        ready=len(re.findall(rb"(?m)^NATIVE EL0 EXIT PID [01] 0\r?$",buf))==2
                    else:
                        ready=mem.leaf(root,va+8) and int.from_bytes(vread(mem,root,va+8,8),"little")==STAGES[stage]
                    if ready and stage=="ready":
                        ready=unpack(read(*L["irq"]))[22]==1
                    if ready and stage=="completed":
                        ready=int.from_bytes(vread(mem,root,va+64,8),"little")==3
                    if ready:
                        break
                    require(proc.poll() is None and time.monotonic()<deadline,"timeout at "+stage)
                    require(not (out/"machine.log").exists() or (out/"machine.log").stat().st_size<=MAX_LOG,"trace bound")
                    time.sleep(.005)
                where=out/stage
                where.mkdir()
                capture_halt(q,where)
                save(where/"trace.json",{"machine_log_bytes":(out/"machine.log").stat().st_size})
                mem=RAM({},read)
                try:
                    s=snapshot(mem,L)
                    require(s["driver"][1]==STAGES[stage],"missed phase "+stage)
                finally:
                    save(where/"ram.json",mem.records)
                request(q,"cont")
            try:
                require("QMP" in json.loads(q.readline()),"QMP greeting")
                request(q,"qmp_capabilities")
                stop("ready")
                send(frames[0])
                stop("held")
                send(frames[1])
                stop("overlap")
                require(not select.select([host],[],[],0)[0],"premature guest TX before flush")
                send(frames[2])
                for seq in range(3):
                    receive(seq)
                stop("completed")
                for seq in range(3,80):
                    send(frames[seq])
                    receive(seq)
                stop("ack")
                send(ack)
                stop("final")
                request(q,"quit")
                require(proc.wait(timeout=5)==0 and host.recv(1)==b"","QEMU exit or extra egress")
                (out/"probe.bin").unlink(missing_ok=True)
            except Exception:
                if proc.poll() is None:
                    request(q,"stop")
                    mem=RAM({},read)
                    try:
                        snapshot(mem,L)
                    finally:
                        save(out/"failure-ram.json",mem.records)
                raise
            finally:
                timer.cancel()
                if proc.poll() is None:
                    proc.kill()
                proc.wait()

def check_rings(s,stage,L,frames,replies):
    n,posted,done={"ready":(0,0,0),"held":(1,1,0),"overlap":(2,1,0),"completed":(3,3,3),"ack":(80,80,80),"final":(81,80,80)}[stage]
    r,t=s["rings"]
    u16=lambda raw,off: struct.unpack_from("<H",raw,off)[0]
    grants=s["grants"]
    if stage=="final":
        require(all(g==(0,0,0) for g in grants),"grant survived retirement")
        require(not any(s["buffers"]) and u16(r,586)==81 and (u16(t,514),u16(t,586))==(80,80),"DMA buffer scrub and terminal used indices after remove")
        return
    require(u16(r,586)==n and 28<=u16(r,514)-n<=32 and (u16(t,514),u16(t,586))==(posted,done),
            "RX posted and used versus TX published and consumed at "+stage)
    avail=u16(r,514)
    owned=[u16(r,516+2*(i%32)) for i in range(n,avail)]
    require(len(set(owned))==len(owned) and all(i<32 for i in owned),"duplicate or invalid RX avail ID")
    spans=[]
    for idx,(token,va,length) in enumerate(grants):
        if idx in owned:
            require(token==s["driver"][2]>0 and length==1530,"posted RX grant owner or capacity")
            spans.append((va,va+length))
        else:
            require((token,va,length)==(0,0,0),"inactive descriptor retains a grant")
    spans.sort()
    require(all(a[1]<=b[0] for a,b in zip(spans,spans[1:])),"overlapping active grants")
    for i in range(32):
        require(struct.unpack_from("<QIHH",r,16*i)==(L["buf"]+i*STRIDE,1530,2,0),"registered RX descriptor")
    rxids={}
    for seq in range(max(0,n-32),n):
        idx,length=struct.unpack_from("<II",r,588+8*(seq%32))
        require(idx<32 and length==12+len(frames[seq]),"RX used entry")
        rxids[idx]=seq
    for idx,seq in rxids.items():
        pos=idx*STRIDE+12
        require(s["buffers"][pos:pos+len(frames[seq])]==frames[seq],"RX descriptor DMA bytes")
    ids={}
    for seq in range(max(0,posted-32),posted):
        pos=seq%32
        idx=u16(t,516+pos*2)
        require(idx<32,"TX avail ID")
        if seq<done:
            require(struct.unpack_from("<II",t,588+pos*8)==(idx,0),"TX hardware completion ID")
        ids[idx]=seq
    for idx,seq in ids.items():
        f=replies[seq]
        off=(32+idx)*STRIDE
        require(struct.unpack_from("<QIHH",t,idx*16)==(L["buf"]+off,len(f)+12,0,0)
                and s["buffers"][off+12:off+12+len(f)]==f,"TX descriptor and DMA bytes")


def check_mmio(lines,L):
    regs=[]
    for line in lines:
        m=re.search(rb"memory_region_ops_(read|write) cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size 4",line)
        if m and NIC<=int(m[2],16)<NIC+256:
            regs.append((m[1],int(m[2],16)-NIC,int(m[3],16)))
    require([v for op,r,v in regs if (op,r)==(b"write",112)]==[0,1,3,11,15,0],"one hardware status lifetime")
    writes=[[],[]]
    ready=[[],[]]
    kicks=[0,0]
    selected=None
    live=False
    for op,r,v in regs:
        if (op,r)==(b"write",112):
            live=v==15
        if (op,r)==(b"write",48):
            selected=v
        if op==b"write" and r in (56,128,132,144,148,160,164,68):
            writes[selected].append((r,v))
        if op==b"read" and r==68:
            ready[selected].append(v)
        if (op,r)==(b"write",80):
            require(live and v in (0,1),"invalid queue notify")
            kicks[v]+=1
    for i in range(2):
        addr=L["ring"]+4096*i
        expected=[(56,32)]
        for reg,off in ((128,0),(144,512),(160,584)):
            expected.extend([(reg,(addr+off)&0xffffffff),(reg+4,(addr+off)>>32)])
        require(writes[i]==expected+[(68,1)] and ready[i][0]==ready[i][-1]==0 and 1 in ready[i],"queue programming/reset")
    require(kicks[0]>0 and kicks[1]==80,"RX refill and TX doorbell counts")
    return kicks


def replay(pkg,imp,out,mutation=None):
    L=layout(pkg,imp)
    a=load(out/"launch.json")
    require(a["schema"]==1 and a["import_sha256"]==sha(imp) and a["kernel_sha256"]==sha(pkg/"kernel.elf")
            and a["argv"]==command(a["argv"][0],Path(a["out"]),Path(a["kernel"]),a["fd"],a["mac"],a["mtu"],L)
            and (out/"net.events").read_text()==irq.TRACE,"launch and trace selection")
    nonce=(out/"nonce.bin").read_bytes()
    require(len(nonce)==8 and any(nonce),"external challenge nonce")
    mac=bytes.fromhex(a["mac"].replace(":",""))
    frames,replies,ack=protocol(mac,nonce)
    require(socket_frames((out/"ingress.bin").read_bytes())==frames+[ack] and
            socket_frames((out/"egress.bin").read_bytes())==replies,"full duplex socket streams")
    expected=frames[:3]+replies[:3]+[f for pair in zip(frames[3:],replies[3:]) for f in pair]+[ack]
    wire=pcap_frames(out/"wire.pcap")
    if mutation=="pcap":
        wire.pop(3)
    require(len(wire)==161 and all(p in (f,bytes(10)+f,bytes(12)+f) for p,f in zip(wire,expected)),"ordered NIC ingress and egress")
    stages={}
    boundaries=[]
    for stage,phase in STAGES.items():
        path=out/stage
        boundaries.append(load(path/"trace.json")["machine_log_bytes"])
        q,h=load(path/"qmp.json"),load(path/"halted.json")
        require(q["status"]["running"] and q["stop_return"]=={} and [x["cpu-index"] for x in q["cpus"]]==[0]
                and h["status"]=="paused" and not h["running"],"single stopped CPU")
        mem=RAM(load(path/"ram.json"))
        s=snapshot(mem,L)
        stages[stage]=s
        require(mem.used==set(mem.records) and s["driver"][0:2]==[0x4455504c44525631,phase],"stopped driver phase")
        if mutation=="used" and stage=="overlap":
            ring=bytearray(s["rings"][1])
            struct.pack_into("<H",ring,586,1)
            s["rings"][1]=ring
        if mutation=="grant" and stage=="overlap":
            idx=next(i for i,g in enumerate(s["grants"]) if g[0])
            s["grants"][idx]=(0,0,0)
        check_rings(s,stage,L,frames+[ack],replies)
    if L["tx_events"]:
        require(stages["final"]["peer_denial"] == -13,"peer cannot claim current-owner TX IRQ")
        if mutation == "tx-event":
            stages["ack"]["events"][0] = (0,)*8
        if mutation == "tx-callback":
            stages["ack"]["callbacks"][0] = (0,)*8
        for stage,total in zip(STAGES,(0,0,0,3,80,80)):
            stopped=stages[stage]
            for i,(event,callback) in enumerate(zip(stopped["events"],stopped["callbacks"],strict=True)):
                if i >= total:
                    require(not any(event) and not any(callback),"premature TX IRQ / callback at "+stage)
                    continue
                token=stopped["driver"][2]
                require(event[:2]==(token,i+1) and event[2]&1023==79 and event[3]&1
                        and event[4]==i+1 and event[5]<32 and event[6] in (0,1) and event[7]==i+1,
                        "protected TX interrupt and claim at "+stage)
                if i >= total-32 and stage != "final":
                    ring=stopped["rings"][1]
                    used=struct.unpack_from("<II",ring,588+(i%32)*8)
                    require(used==(event[5],0),"TX IRQ descriptor versus hardware used ring")
                require(callback[:4]==(token,i,i+1,L["tx_callback"]) and callback[5]==callback[4]+1
                        and callback[6:]==(0,1),"registered TX wake and cookie release")
    require(stages["ready"]["irq"][22]==1 and stages["ready"]["irq"][2]==0 and stages["completed"]["irq"][2]>0,"genuine parked RX owner and subsequent device event")
    final=stages["final"]
    d,p=final["driver"],final["peer"]
    total=sum(map(len,frames))
    require(d[2]>0 and d[3:11]==[int.from_bytes(mac,"big"),int.from_bytes(nonce,"big"),80,80,80,80,total,total]
            and d[12:25]==[a["mtu"],1,15,0,32,0,0,60,NEG-19,NEG-116,NEG-116,0,0]
            and d[27]==NEG-116 and d[30:32]==[15,0],"owner traffic, release and stale token")
    require(p==[0x4455504c50454552,100,*d[2:5],80,80,total,total,*([NEG-13]*4),60,NEG-13,0],"peer authority and completion")
    histories=[b"".join(f.ljust(91,b"\0") for f in records) for records in (frames,replies)]
    if mutation=="peer":
        histories[1]=histories[0]
    require(final["wire"]==histories+[ack],"separate peer private byte history")
    for stage,counts in (("held",[1,0,0]),("overlap",[1,0,0]),("completed",[3,3,3]),("ack",[80,80,80])):
        tx=stages[stage]["tx"]
        require(tx[0]==d[2] and all(tx[1:7]) and tx[3]!=tx[4] and tx[5]==L["code"]["start_xmit"]["entry"]
                and tx[6]==L["tx_callback"] and tx[13]==L["ops"] and tx[7:10]==counts and tx[14]==int(stage in ("held","overlap")),"registered TX and real cookie completion")
    for stage,row,rx_done,tx_done in (("held",2,1,0),("overlap",3,2,0),("ack",4,80,80)):
        s=stages[stage]
        tq,rq=s["qtx"][8*row:8*(row+1)],s["qrx"][8*row:8*(row+1)]
        require(rq[1]==rx_done and 28<=rq[2]<=32 and tq[:3]==([1,0,0] if not tx_done else [80]*3)
                and tq[6]==int(not tx_done) and tq[7]==0,"sampled Linux RX/TX cookie lifetime")
    require((out/"machine.log").stat().st_size<=MAX_LOG,"capture exceeds trace bound")
    raw=(out/"machine.log").read_bytes()
    require(all(0<offset<=len(raw) for offset in boundaries) and boundaries==sorted(boundaries),"trace checkpoint extent")
    pattern=rb"memory_region_ops_write cpu 0 mr [^\n]+? addr 0x0*%x value 0x0*1 size 4" % (NIC+80)
    tx_bells=[m.end() for m in re.finditer(pattern,raw)]
    for stage,offset,count in zip(STAGES,boundaries,(0,0,0,3,80,80)):
        require(sum(pos<=offset for pos in tx_bells)==count,"TX notifications at stopped "+stage)
    lines=raw.splitlines()
    kicks=check_mmio(lines,L)
    epochs=irq.delivery_epochs(lines)
    irq.wire_delivery(lines,epochs,False)
    from verify_linux_source_net import callback_attempts
    entries={}
    for name,fn in L["code"].items():
        _,hits=callback_attempts(lines,fn["entry"])
        required={"virtnet_probe":1,"virtnet_open":1,"virtnet_close":1,"virtnet_remove":1,"start_xmit":80,"skb_xmit_done":80}
        require(hits,"missing original execution "+name)
        rx.verify_execution(lines,fn,required.get(name,len(hits)))
        entries[name]=hits
    require(epochs and entries["virtnet_probe"][0]<entries["virtnet_open"][0]<entries["start_xmit"][0]
            and entries["start_xmit"][-1]<entries["virtnet_close"][0]<entries["virtnet_remove"][0]
            and any(epochs[0]["eoi"]<c<t<entries["start_xmit"][0] for c in entries["skb_recv_done"] for t in entries["virtnet_poll"]),
            "original IRQ callback, NAPI and netdev lifetime")
    if L["tx_events"]:
        for i,(submit,callback) in enumerate(zip(entries["start_xmit"],entries["skb_xmit_done"],strict=True)):
            next_submit=(entries["start_xmit"]+[entries["virtnet_close"][0]])[i+1]
            require(submit<callback<next_submit and any(submit<e["iar"]<e["eoi"]<callback
                    and e["status"]&1 for e in epochs),"executed TX callback after real queue IRQ")
    uart=(out/"uart.log").read_bytes()
    require(b"NATIVE EL0 FAULT" not in uart and sorted(re.findall(rb"(?m)^NATIVE EL0 EXIT PID ([01]) (-?\d+)\r?$",uart))
            ==[(b"0",b"0"),(b"1",b"0")],"signed exits")
    return dict(token=d[2],reply_frames=80,incoming_frames=81,queue_notifications=kicks,original_entries={n:len(v) for n,v in entries.items()})


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for opt in ("package","driver-manifest","out"):
        parser.add_argument("--"+opt,required=True,type=Path)
    parser.add_argument("--require-tx-irq",action="store_true")
    parser.add_argument("--replay",action="store_true")
    parser.add_argument("--mutation-tests",action="store_true")
    parser.add_argument("--qemu",default="qemu-system-aarch64")
    parser.add_argument("--mtu",type=int,default=1410)
    parser.add_argument("--mac",default="52:54:00:44:10:01")
    parser.add_argument("--timeout",type=int,default=180)
    a=parser.parse_args()
    pkg,imp,out=a.package.resolve(),a.driver_manifest.resolve(),a.out.resolve()
    if a.require_tx_irq:
        require(layout(pkg,imp)["tx_events"],"TX-IRQ mode requires primary argument t")
    if not a.replay:
        capture(pkg,imp,out,a.qemu,a.mac,a.mtu,a.timeout)
    result=replay(pkg,imp,out)
    if a.mutation_tests:
        for case in ("pcap","used","peer","grant") + (("tx-event","tx-callback") if a.require_tx_irq else ()):
            try:
                replay(pkg,imp,out,case)
            except (ValueError,RuntimeError,AssertionError) as exc:
                result.setdefault("rejected",{})[case]=str(exc)
            else:
                raise ValueError("accepted corruption "+case)
    print(json.dumps(result,indent=2))


if __name__=="__main__":
    main()
