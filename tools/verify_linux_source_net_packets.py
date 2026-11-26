#!/usr/bin/env python3
"""Check native upstream start_xmit against independent socket, PCAP, ring, and execution evidence."""
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
from ordinary_amc_capture import FLAGS,save,sha
from qemu_capture import capture_halt,journal_location,request
from verify_linux_source_net import EXEC, INSN, NEG, NIC, PIN, RAM, callback_attempts, load, require
from verify_linux_source_net_queues import routine

COUNT=80
STRIDE=1536
CAPACITY=1526
TRACE="memory_region_ops_read\nmemory_region_ops_write\n"
MAX_LOG=160<<20


def model_command(binary,out,kernel,fd,mac,mtu,layout):
    return [binary,*FLAGS,"-qmp","stdio","-kernel",str(kernel),"-serial","file:"+str(out/"uart.log"),
            "-global","virtio-mmio.force-legacy=false","-netdev",f"socket,id=n,fd={fd}",
            "-device",f"virtio-net-device,netdev=n,mac={mac},host_mtu={mtu}",
            "-object",f"filter-dump,id=dump,netdev=n,file={out/'wire.pcap'}",
            "-trace","events="+str(out/"net.events"),"-d","in_asm,exec,nochain",
            "-dfilter",layout["filter"],"-D",str(out/"machine.log")]


def provenance(pkg,imp,live=False):
    m=load(imp)
    require(m["mode"]=="packet-service" and m["source"]["sha256"]==m["upstream"]["sha256"]==PIN and
            m["upstream"]["path"]=="drivers/net/virtio_net.c" and
            m["upstream"]["commit"]=="4700e3adf3b24b39dd4929e9c6d62811ba3ccea8","pinned Linux source")
    if live:
        require(sha(Path(m["source"]["path"]))==PIN,"original translation unit identity")
    require(m["callback_input"]=="virtio_net.packet.o" and
            sha(imp.parent/"virtio_net.original.o")==m["objects"]["virtio_net.original.o"]["sha256"],
            "stock whole-driver object")
    name="start_xmit"
    code,relocs=[],set()
    mapfile=imp.parent/"service.map"
    require(sha(mapfile)==m["link_map_sha256"] and
            sum(s.strip().endswith("virtio_net.callback.o:(.text.start_xmit)") for s in mapfile.read_text().splitlines())==1,
            "linked start_xmit provenance")
    for i,(path,field) in enumerate(((imp.parent/"virtio_net.packet.o","driver_object"),
                                     (imp.parent/"virtio_net.callback.o","callback_object"),(pkg/"user.elf","service"))):
        require(sha(path)==m[field]["sha256"],"linked driver artifact digest")
        with path.open("rb") as f:
            elf=ELFFile(f)
            require(elf["e_machine"]=="EM_AARCH64" and elf.elfclass==64 and elf.little_endian,"wrong architecture")
            sym,body=routine(elf,name)
            code.append(body)
            require(sym["st_info"]["bind"]==("STB_LOCAL" if i==0 else "STB_GLOBAL"),"callback binding")
            if i==0:
                for sec in elf.iter_sections():
                    if isinstance(sec,RelocationSection) and sec["sh_info"]==sym["st_shndx"]:
                        relocs.update(r["r_offset"]//4 for r in sec.iter_relocations())
            if i==2:
                require(elf["e_type"]=="ET_EXEC" and elf.get_section_by_name(".note.jnox.abi") and
                        not any(s["p_type"] in ("PT_INTERP","PT_DYNAMIC") for s in elf.iter_segments()),
                        "dynamic or non-native service")
                require(not elf.get_section_by_name(".symtab").get_symbol_by_name("virtnet_poll"),"RX proof scope changed")
                entry=sym["st_value"]
                layout=dict(entry=entry,size=len(body),prefix=body[:16].hex(),peer=None,
                            filter=f"0x{entry:x}+0x{len(body):x}")
    require(code[0]==code[1] and len(code[0])==len(code[2]) and all(code[0][n:n+4]==code[2][n:n+4]
            for n in range(0,len(code[0]),4) if n//4 not in relocs),"linked start_xmit instructions")
    with (pkg/"native_peer.elf").open("rb") as f:
        for s in ELFFile(f).iter_segments():
            if s["p_type"]=="PT_LOAD" and s["p_flags"]&1:
                va=s["p_vaddr"]
                if (va&~4095)<=entry<((va+s["p_memsz"]+4095)&~4095):
                    require(entry>=va,"ambiguous peer mapping")
                    pos=entry-va
                    layout["peer"]=s.data()[pos:pos+16].ljust(16,b"\0").hex()
    return dict(original_source=PIN,import_sha256=sha(imp),package_sha256=sha(pkg/"manifest.json"),
                kernel_sha256=sha(pkg/"kernel.elf"),driver_sha256=sha(pkg/"user.elf"),
                start_xmit_section=hashlib.sha256(code[0]).hexdigest(),layout=layout)


def verify_execution(lines,layout):
    entry=layout["entry"]
    attempts,finished=callback_attempts(lines,entry)
    require(len(finished)==COUNT,"wrong executed start_xmit entry count")
    translations=[]
    for i,line in enumerate(lines):
        ins=INSN.match(line)
        if ins and int(ins[1],16)==entry:
            prefix=bytearray()
            for l in lines[i:i+4]:
                ins=INSN.match(l)
                if not ins or int(ins[1],16)!=entry+len(prefix):
                    break
                prefix.extend(int(ins[2],16).to_bytes(4,"little"))
            require(prefix and layout["prefix"].startswith(prefix.hex()) and
                    (layout["peer"] is None or not layout["peer"].startswith(prefix.hex())),"wrong translated start_xmit")
            translations.append(i)
    require(len(translations)==1 and translations[0]<attempts[0][0] and all(r==attempts[0][1] for _,r in attempts)
            and not any(EXEC.match(s) or s.startswith(b"IN:") for s in lines[translations[0]+1:attempts[0][0]]),
            "executed block not bound to original callback")
    return len(finished)


def pcap_frames(path):
    data=path.read_bytes()
    orders={b"\xd4\xc3\xb2\xa1":"<",b"\xa1\xb2\xc3\xd4":">",b"\x4d\x3c\xb2\xa1":"<",b"\xa1\xb2\x3c\x4d":">"}
    order=orders.get(data[:4])
    require(order and len(data)>=24 and struct.unpack_from(order+"I",data,20)[0]==1,"invalid Ethernet PCAP")
    pos=24
    frames=[]
    while pos<len(data):
        require(pos+16<=len(data),"short PCAP record")
        captured,original=struct.unpack_from(order+"II",data,pos+8)
        pos+=16
        require(0<captured==original<=65535 and pos+captured<=len(data),"truncated PCAP frame")
        frames.append(data[pos:pos+captured])
        pos+=captured
    return frames


def socket_frames(data):
    pos=0
    frames=[]
    while pos<len(data):
        require(pos+4<=len(data),"truncated socket prefix")
        size=int.from_bytes(data[pos:pos+4],"big")
        pos+=4
        require(14<=size<=1514 and pos+size<=len(data),"invalid stream record")
        frames.append(data[pos:pos+size])
        pos+=size
    return frames


def exact(conn,size,deadline):
    data=bytearray()
    while len(data)<size:
        conn.settimeout(max(.001,deadline-time.monotonic()))
        part=conn.recv(size-len(data))
        require(part,"premature network socket EOF")
        data+=part
    return bytes(data)


def dma_layout(pkg):
    kernel=pkg/"kernel.elf"
    rings,ring_size=journal_location(kernel,"jnox_linux_net_queue_pool")
    buffers,buf_size=journal_location(kernel,"jnox_linux_net_packet_buffers")
    require(ring_size==8192 and buf_size==2*32*STRIDE and rings%4096==0 and buffers%16==0 and
            (rings+ring_size<=buffers or buffers+buf_size<=rings),"kernel DMA operand extent")
    return rings,buffers


def area(mem,pa,size):
    raw=bytearray()
    while size:
        n=min(4096-pa%4096,size)
        raw+=mem.read(pa,n)
        pa+=n
        size-=n
    return bytes(raw)


def snapshot(ram,pkg):
    m=load(pkg/"manifest.json")
    require(m["linux_net_packets"]["driver_slot"]==0 and m["linux_net_packets"]["unauthorized_slots"]==[1] and
            m["linux_net_packets"]["virtio_net_header_bytes"]==12 and
            (m["board"],m["profile"],m["policy"])==("virt","native","fifo") and len(m["slots"])==2,
            "packet ownership profile")
    kernel=pkg/"kernel.elf"
    ring,buf=dma_layout(pkg)
    protected=set(range(ring,ring+8192,4096))|set(range(buf&~4095,buf+64*STRIDE,4096))
    user_pages=[]
    receipts=[]
    kernel_raw=kernel.read_bytes()
    require(sha(kernel)==m["kernel_elf_sha256"],"booted kernel image hash")
    for i,root in enumerate(linked_roots(kernel,2)):
        slot=m["slots"][i]
        require(slot["slot"]==i and slot["elf"]==("user.elf","native_peer.elf")[i],"task placement")
        elf=pkg/slot["elf"]
        require(sha(elf)==slot["elf_sha256"] and elf.read_bytes() in kernel_raw,"booted role ELF")
        va,size=journal_location(elf,PROBES[i][0])
        require(size==PROBES[i][1]*8 and 0x400000<=va<va+size<=0x800000,"receipt extent")
        user_pages.append({int(permissions(ram,root,n*4096,"user rw"),16)&PA_MASK
                           for n in range(va//4096,(va+size-1)//4096+1)})
        words=list(struct.unpack(f"<{size//8}Q",vread(ram,root,va,size)))
        if i==0:
            sg_va,sg_size=journal_location(elf,"jnox_packet_sg_counts")
            require(sg_size==16 and sg_va%8==0 and 0x400000<=sg_va<sg_va+16<=0x800000 and
                    (va+size<=sg_va or sg_va+16<=va),"driver SG counter extent")
            for page in range(sg_va//4096,(sg_va+15)//4096+1):
                permissions(ram,root,page*4096,"user rw")
            words.extend(struct.unpack("<2Q",vread(ram,root,sg_va,16)))
        receipts.append(words)
        permissions(ram,root,NIC&~4095,"kernel rw",device=True)
        for pa in protected:
            permissions(ram,root,pa,"kernel rw")
        tables={root&PA_MASK}
        for level in range(3):
            children=set()
            for table in tables:
                for (desc,) in struct.iter_unpack("<Q",ram.read(table,4096)):
                    if not desc&1:
                        continue
                    require(desc&3==3,"unexpected block/table type")
                    if level<2:
                        children.add(desc&PA_MASK)
                    elif desc&PA_MASK in protected:
                        require(not desc&64,"EL0 alias of protected DMA operand")
            tables=children
    require(user_pages[0].isdisjoint(user_pages[1]) and not (user_pages[0]|user_pages[1])&protected,
            "private operand alias")
    return receipts,[area(ram,ring+i*4096,4096) for i in (0,1)],area(ram,buf,64*STRIDE)

PROBES=(("jnox_packet_driver_result",32),("jnox_packet_peer_result",17))
HOST=bytes.fromhex("024a4e4f5801")
TYPE=bytes.fromhex("88b5")
PHASES=(("ready",3),("active",6),("final",100))


def challenge(mac,nonce,ack=False):
    return mac+HOST+TYPE+(b"JNOXACK!" if ack else b"JNOXCHAL")+nonce+bytes(30)


def output_frames(mac,nonce):
    return [HOST+mac+TYPE+b"JNOXDATA"+nonce+i.to_bytes(4,"big")+
            bytes((i+n)&255 for n in range(34,60+i%32)) for i in range(COUNT)]


def inspect_rings(stage,rings,buffers,base,expected,rx_frames):
    rx,tx=rings
    u16=lambda raw,off: struct.unpack_from("<H",raw,off)[0]
    posted=0 if stage=="ready" else COUNT
    received={"ready":0,"active":1,"final":2}[stage]
    require((u16(rx,514),u16(rx,586))==(32+received,received) and
            (u16(tx,514),u16(tx,586))==(posted,posted),"device used/available index mismatch")
    for idx in range(32):
        addr,length,flags,nxt=struct.unpack_from("<QIHH",rx,idx*16)
        require((addr,length,flags,nxt)==(base+idx*STRIDE,CAPACITY,2,0),"RX DMA bounds or descriptor flags")
    for i in range(received):
        idx,length=struct.unpack_from("<II",rx,588+i*8)
        require(idx<32 and idx==u16(rx,516+2*(i%32)) and length==12+len(rx_frames[i]),
                "hardware RX used receipt")
        if stage=="active":
            content=buffers[idx*STRIDE:idx*STRIDE+length]
            require(content[12:]==rx_frames[i], "active RX DMA bytes")
    if not posted:
        require(not any(tx[:512]) and not any(buffers),"cold TX descriptor or cold packet backing dirty")
        return
    last_by_id={}
    for seq in range(posted-32,posted):
        slot=seq%32
        idx,used_len=struct.unpack_from("<II",tx,588+slot*8)
        require(idx<32 and used_len==0 and idx==u16(tx,516+slot*2),"TX used ID/available ID mismatch")
        last_by_id[idx]=seq
    for idx in range(32):
        addr,length,flags,nxt=struct.unpack_from("<QIHH",tx,idx*16)
        if addr==0:
            require((length,flags,nxt)==(0,0,0) and idx not in last_by_id,"missing completed TX descriptor")
            continue
        require(addr==base+(32+idx)*STRIDE and 26<=length<=CAPACITY and (flags,nxt)==(0,0),
                "TX DMA bounds or descriptor flags")
        if idx in last_by_id:
            frame=expected[last_by_id[idx]]
            require(length==12+len(frame),"TX descriptor length")
            if stage=="active":
                pos=(32+idx)*STRIDE
                require(buffers[pos:pos+length]==bytes(12)+frame,"TX DMA bytes differ from socket packet")
    if stage=="final":
        require(not any(buffers),"packet storage not cleared on retirement")


def mmio_evidence(lines,pool,d):
    regs=[]
    for line in lines:
        if b"memory_region_ops_" not in line:
            continue
        m=re.search(rb"memory_region_ops_(read|write) cpu (-?\d+) mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size (\d+)",line,re.I)
        require(m,"malformed physical trace")
        off=int(m[3],16)-NIC
        if 0<=off<512:
            require(m[2]==b"0","non-CPU0 NIC access")
            regs.append((m[1].decode(),off,int(m[4],16),int(m[5])))
    status=[v for op,r,v,w in regs if (op,r,w)==("write",112,4)]
    require(status==[0,1,3,11,15,0],"unexpected claim, DRIVER_OK or retirement sequence")
    current=None
    programmed=[[],[]]
    ready=[[],[]]
    maxima=[set(),set()]
    kicks=[0,0]
    irq=0
    last_pending=0
    live=False
    for op,off,value,width in regs:
        if off==112 and op=="write":
            live=value==15
        if op=="write" and off==48:
            require(value in (0,1) and width==4,"unexpected queue selector")
            current=value
        if op=="write" and off in (56,128,132,144,148,160,164,68):
            require(current is not None and width==4,"queue programming width")
            programmed[current].append((off,value))
        if op=="read" and off==68:
            require(current is not None and width==4,"queue ready read")
            ready[current].append(value)
        if op=="read" and off==52:
            require(current is not None and width==4,"queue max read")
            maxima[current].add(value)
        if op=="write" and off==80:
            require(live and width==4 and value in (0,1),"notification outside DRIVER_OK")
            kicks[value]+=1
        if op=="read" and off==96:
            last_pending=value&3
        if op=="write" and off==100:
            require(live and width==4 and 0<value==last_pending,"IRQ ack differs from pending bits")
            irq|=value
            last_pending=0
    for i in (0,1):
        addr=pool+4096*i
        exp=[(56,32)]
        for reg,delta in ((128,0),(144,512),(160,584)):
            exp.extend([(reg,(addr+delta)&0xffffffff),(reg+4,(addr+delta)>>32)])
        require(programmed[i]==exp+[(68,1)] and ready[i][0]==ready[i][-1]==0 and 1 in ready[i],
                "ring addresses, size or hardware ready/reset")
        require(maxima[i]=={d[24+i]>>32} and d[24+i]&0xffffffff==(32<<16)|1,"queue hardware max/ready")
    require(kicks==[3,COUNT] and d[14]==irq and irq&1,"device kicks or polled interrupt cause")
    return dict(notifications=kicks,observed_and_acknowledged_irq_bits=irq)


def replay(pkg,imp,out):
    meta=provenance(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["identities"]==meta and launch["schema"]==1 and (out/"net.events").read_text()==TRACE,
            "launch source or trace identity")
    cmd=launch["argv"]
    require(cmd==model_command(cmd[0],Path(launch["out"]),Path(launch["kernel"]),launch["fd"],launch["mac"],
                               launch["mtu"],meta["layout"]), "unexpected process or network backend")
    mac=bytes.fromhex(launch["mac"].replace(":",""))
    nonce=(out/"nonce.bin").read_bytes()
    require(len(nonce)==8,"invalid challenge nonce")
    incoming=[challenge(mac,nonce),challenge(mac,nonce,True)]
    outgoing=output_frames(mac,nonce)
    require(socket_frames((out/"host-to-guest.bin").read_bytes())==incoming and
            socket_frames((out/"guest-to-host.bin").read_bytes())==outgoing,"nonce or sequence on socket stream")
    wire=pcap_frames(out/"wire.pcap")
    all_frames=[incoming[0],*outgoing,incoming[1]]
    require(len(wire)==len(all_frames) and all(a==b or a==bytes(10)+b or a==bytes(12)+b
            for a,b in zip(wire,all_frames)),"PCAP direction, nonce, sequence, length or payload")
    ring,buf=dma_layout(pkg)
    states=[]
    for stage,phase in PHASES:
        where=out/stage
        qmp,halt=load(where/"qmp.json"),load(where/"halted.json")
        require(qmp["status"]["running"] and [c["cpu-index"] for c in qmp["cpus"]]==[0] and
                qmp["stop_return"]=={} and halt["status"]=="paused" and not halt["running"],"stopped CPU evidence")
        ram=RAM(load(where/"ram.json"))
        (d,p),rings,buffers=snapshot(ram,pkg)
        require(ram.used==set(ram.records) and d[0:3]==[0x564e504b44525631,phase,0],
                "native phase, driver status or retained reads")
        require(d[32:34]==([0,0] if stage=="ready" else [40,40]),"physical SG shape counters")
        inspect_rings(stage,rings,buffers,buf,outgoing,incoming)
        states.append((d,p))
    begin,active,final=(s[0] for s in states)
    p=states[-1][1]
    require(begin[3]==active[3]==final[3]>0 and begin[10]==0 and active[10:14]==final[10:14]==
            [COUNT,COUNT,sum(map(len,outgoing)),COUNT-1],"native TX submission and completion receipts")
    require(final[4]==(1<<32)|40 and final[5]==int.from_bytes(mac,"big") and final[6:10]==
            [0,15,60,int.from_bytes(nonce,"big")] and final[15:22]==[0,NEG-116,NEG-116,0,launch["mtu"],1,0]
            and final[26:30]==[11,72,0,60],"packet feature, RX challenge, stale or adapter receipts")
    require(p[:10]==[0x564e504b50454531,100,0]+[NEG-13]*6+[6] and p[16]==NEG-13 and
            all(a<b for a,b in zip(p[10:15],p[11:16])),"peer access or progress")
    uart=(out/"uart.log").read_bytes().splitlines()
    manifest=load(pkg/"manifest.json")
    for slot,prefix in zip(manifest["slots"],(b"NATIVE EL0 ELF ",b"NATIVE EL0 PEER ELF ")):
        require(prefix+slot["elf_sha256"].encode() in uart,"booted native ELF")
    require({l for l in uart if l.startswith(b"NATIVE EL0 EXIT PID ")}==
            {b"NATIVE EL0 EXIT PID 0 0",b"NATIVE EL0 EXIT PID 1 0"} and not any(b"NATIVE EL0 FAULT" in l for l in uart),
            "native terminal state")
    require((out/"machine.log").stat().st_size<=MAX_LOG,"trace size bound exceeded")
    lines=(out/"machine.log").read_bytes().splitlines()
    entries=verify_execution(lines,meta["layout"])
    observations=mmio_evidence(lines,ring,final)
    return dict(start_xmit_entries=entries,one_sg_tx=final[32],two_sg_tx=final[33],rx_native_frames=2,tx_hardware_used=COUNT,tx_ring_wraps=COUNT//32,
                original_virtnet_poll_linked=False,tx_ethernet_bytes=sum(map(len,outgoing)),**observations)

def capture(pkg,imp,out,binary,mtu,mac,timeout):
    qemu=shutil.which(binary)
    require(qemu,"missing QEMU")
    meta=provenance(pkg,imp,True)
    root=linked_roots(pkg/"kernel.elf",2)[0]
    va,_=journal_location(pkg/"user.elf",PROBES[0][0])
    out.mkdir(parents=True,exist_ok=False)
    (out/"net.events").write_text(TRACE)
    nonce=secrets.token_bytes(8)
    (out/"nonce.bin").write_bytes(nonce)
    macbytes=bytes.fromhex(mac.replace(":",""))
    end=time.monotonic()+timeout
    host,device=socket.socketpair(socket.AF_UNIX,socket.SOCK_STREAM)
    with host,device,(out/"stderr.log").open("xb") as stderr:
        fd=device.fileno()
        cmd=model_command(qemu,out,pkg/"kernel.elf",fd,mac,mtu,meta["layout"])
        with subprocess.Popen(cmd,pass_fds=(fd,),stdin=subprocess.PIPE,stdout=subprocess.PIPE,
                              stderr=stderr,bufsize=0) as proc:
            device.close()
            save(out/"launch.json",dict(schema=1,identities=meta,argv=cmd,pid=proc.pid,fd=fd,
                 out=str(out),kernel=str(pkg/"kernel.elf"),mtu=mtu,mac=mac,qemu_sha256=sha(Path(qemu)),
                 qemu_version=subprocess.check_output([qemu,"--version"],text=True).splitlines()[0]))
            watchdog=threading.Timer(timeout+8,proc.kill)
            watchdog.start()
            class Pipe:
                write=proc.stdin.write
                readline=proc.stdout.readline
            stream=Pipe()
            dest=out/"probe.bin"
            def read(pa,size):
                request(stream,"pmemsave",val=pa,size=size,filename=str(dest))
                return dest.read_bytes()
            def healthy():
                require(time.monotonic()<end and proc.poll() is None,"deadline or unexpected QEMU exit")
                log=out/"machine.log"
                require(not log.exists() or log.stat().st_size<=MAX_LOG,"QEMU log exceeded bound")
            def stage(name,phase):
                while True:
                    healthy()
                    value=int.from_bytes(vread(RAM({},read),root,va+8,8),"little")
                    if value==phase:
                        break
                    uart=(out/"uart.log").read_bytes()
                    require(b"NATIVE EL0 FAULT" not in uart and b"NATIVE EL0 EXIT PID 0 " not in uart,
                            "driver exited before expected phase")
                    time.sleep(.02)
                where=out/name
                where.mkdir()
                capture_halt(stream,where)
                mem=RAM({},read)
                try:
                    (d,_p),_,_=snapshot(mem,pkg)
                    require(d[1]==phase,"receipt changed before snapshot")
                finally:
                    save(where/"ram.json",mem.records)
                if name!="final":
                    request(stream,"cont")
            def send(frame,sink):
                data=len(frame).to_bytes(4,"big")+frame
                host.settimeout(max(.001,end-time.monotonic()))
                host.sendall(data)
                sink.write(data)
                sink.flush()
            try:
                require("QMP" in json.loads(stream.readline()),"missing QMP greeting")
                request(stream,"qmp_capabilities")
                # Wait for the image before attempting to walk pre-boot page tables.
                peer_hash=load(pkg/"manifest.json")["slots"][1]["elf_sha256"].encode()
                while True:
                    healthy()
                    uart=out/"uart.log"
                    if uart.exists() and b"NATIVE EL0 PEER ELF "+peer_hash in uart.read_bytes():
                        break
                    time.sleep(.01)
                # Translations become valid only after native startup has installed the task tables.
                while True:
                    healthy()
                    probe=RAM({},read)
                    if probe.leaf(root,va) is not None:
                        break
                    time.sleep(.01)
                with (out/"host-to-guest.bin").open("xb") as sent,(out/"guest-to-host.bin").open("xb") as recv:
                    stage("ready",3)
                    send(challenge(macbytes,nonce),sent)
                    for frame in output_frames(macbytes,nonce):
                        healthy()
                        prefix=exact(host,4,end)
                        size=int.from_bytes(prefix,"big")
                        require(14<=size<=1514,"invalid device socket length")
                        raw=exact(host,size,end)
                        recv.write(prefix+raw)
                        recv.flush()
                        require(raw==frame,"live TX sequence, payload or nonce mismatch")
                    stage("active",6)
                    send(challenge(macbytes,nonce,True),sent)
                    while True:
                        healthy()
                        uart=(out/"uart.log").read_bytes()
                        if len(set(re.findall(rb"(?m)^NATIVE EL0 EXIT PID ([01]) -?\d+\r?$",uart)))==2:
                            break
                        require(b"NATIVE EL0 FAULT" not in uart,"native fault")
                        time.sleep(.01)
                    stage("final",100)
                request(stream,"quit")
                require(proc.wait(timeout=5)==0,"QEMU shutdown")
                host.settimeout(1)
                require(host.recv(1)==b"","extra device socket traffic")
                dest.unlink(missing_ok=True)
                save(out/"result.json",replay(pkg,imp,out))
            except Exception as exc:
                save(out/"failure.json",dict(error=repr(exc),returncode=proc.poll()))
                raise
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
                watchdog.cancel()
                watchdog.join()


def negative_controls(pkg,imp,out):
    # Replay altered independent observations without changing the retained capture.
    result=[]
    with tempfile.TemporaryDirectory(prefix="jnox-packet-checks-") as tmp:
        work=Path(tmp)/"capture"
        shutil.copytree(out,work)
        def reject(label,path,data,reason):
            before=path.read_bytes()
            try:
                path.write_bytes(data)
                try:
                    replay(pkg,imp,work)
                except ValueError as exc:
                    require(reason in str(exc),"unrelated mutation failure: "+str(exc))
                else:
                    raise ValueError("accepted mutation "+label)
            finally:
                path.write_bytes(before)
            result.append(label)
        for name,reason in (("wire.pcap","PCAP direction"),("guest-to-host.bin","socket stream")):
            path=work/name
            raw=bytearray(path.read_bytes())
            raw[-1]^=1
            reject(name,path,raw,reason)
        path=work/"machine.log"
        lines=path.read_bytes().splitlines(keepends=True)
        entry=load(work/"launch.json")["identities"]["layout"]["entry"]
        _,indices=callback_attempts(lines,entry)
        i=indices[-1]
        reject("missing original execution",path,b"".join(lines[:i]+lines[i+1:]),"start_xmit entry count")
        pool,_=dma_layout(pkg)
        path=work/"active"/"ram.json"
        data=load(path)
        key=f"{pool+4096:x}/4096"
        ring=bytearray.fromhex(data[key])
        struct.pack_into("<H",ring,586,COUNT-1)
        data[key]=ring.hex()
        reject("uncompleted hardware TX",path,json.dumps(data).encode(),"used/available index")
        path=work/"active"/"ram.json"
        data=load(path)
        sg_va,_=journal_location(pkg/"user.elf","jnox_packet_sg_counts")
        root=linked_roots(pkg/"kernel.elf",2)[0]
        mem=RAM(data)
        desc=mem.leaf(root,sg_va)
        sg_pa=(desc&PA_MASK)+sg_va%4096
        n=min(16,4096-sg_va%4096)
        key=f"{sg_pa:x}/{n}"
        changed=bytearray.fromhex(data[key])
        changed[0]^=1
        data[key]=changed.hex()
        reject("Linux SG shape counter",path,json.dumps(data).encode(),"physical SG shape counters")
        path=work/"final"/"ram.json"
        data=load(path)
        key=next(k for k,v in data.items() if v.startswith(struct.pack("<Q",0x564e504b50454531).hex()))
        peer=bytearray.fromhex(data[key])
        struct.pack_into("<Q",peer,24,0)
        data[key]=peer.hex()
        reject("unauthorized peer",path,json.dumps(data).encode(),"peer access")
    return result


if __name__=="__main__":
    cli=argparse.ArgumentParser(description=__doc__)
    for key in ("package","driver-manifest","out"):
        cli.add_argument("--"+key,type=Path,required=True)
    cli.add_argument("--mtu",type=int)
    cli.add_argument("--mac",type=str.lower)
    cli.add_argument("--qemu",default="qemu-system-aarch64")
    cli.add_argument("--timeout",type=float,default=60)
    cli.add_argument("--replay",action="store_true")
    cli.add_argument("--mutation-tests",action="store_true")
    a=cli.parse_args()
    pkg,imp,out=(p.resolve() for p in (a.package,a.driver_manifest,a.out))
    if not a.replay:
        mac=a.mac or "52:54:00:19:26:41"
        mtu=a.mtu if a.mtu is not None else 1410
        require(re.fullmatch(r"(?:[0-9a-f]{2}:){5}[0-9a-f]{2}",mac) and 100<=mtu<=65535,"invalid MAC/MTU")
        capture(pkg,imp,out,a.qemu,mtu,mac,a.timeout)
    launch=load(out/"launch.json")
    require((a.mtu is None or a.mtu==launch["mtu"]) and (a.mac is None or a.mac==launch["mac"]),"CLI NIC identity")
    result=replay(pkg,imp,out)
    if a.mutation_tests:
        result["rejected_mutations"]=negative_controls(pkg,imp,out)
    print(json.dumps(result,indent=2))
