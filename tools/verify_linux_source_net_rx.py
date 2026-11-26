#!/usr/bin/env python3
"""Replay the original Linux RX buffer allocation against native peer RAM and Ethernet evidence."""
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

from verify_linux_source_net_packets import (COUNT, HOST, TYPE, STRIDE, TRACE, MAX_LOG, PHASES, model_command,
                                            pcap_frames, socket_frames, dma_layout, area, challenge)

PROBES = (("jnox_rx_driver_result",32), ("jnox_rx_peer_result",32))
RX_CAPACITY = 1530


def routine_provenance(pkg,imp,name,live):
    m=load(imp)
    require(m["mode"]=="rx-service" and m["source"]["sha256"]==m["upstream"]["sha256"]==PIN and
            m["upstream"]["path"]=="drivers/net/virtio_net.c" and
            m["upstream"]["commit"]=="4700e3adf3b24b39dd4929e9c6d62811ba3ccea8","pinned Linux source")
    if live:
        require(sha(Path(m["source"]["path"]))==PIN,"original translation unit identity")
    require(m["callback_input"]=="virtio_net.packet.o" and
            sha(imp.parent/"virtio_net.original.o")==m["objects"]["virtio_net.original.o"]["sha256"],
            "stock whole-driver object")
    code,relocs=[],set()
    mapfile=imp.parent/"service.map"
    require(sha(mapfile)==m["link_map_sha256"] and
            sum(s.strip().endswith(f"virtio_net.callback.o:(.text.{name})") for s in mapfile.read_text().splitlines())==1,
            "linked original routine provenance: "+name)
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
            for n in range(0,len(code[0]),4) if n//4 not in relocs),"linked add_recvbuf_small instructions")
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
                section_sha256=hashlib.sha256(code[0]).hexdigest(),layout=layout)


def provenance(pkg,imp,live=False):
    added=routine_provenance(pkg,imp,"add_recvbuf_small",live)
    getter=routine_provenance(pkg,imp,"virtnet_rq_get_buf",False)
    added["get_buf_layout"]=getter["layout"]
    added["get_buf_section_sha256"]=getter["section_sha256"]
    added["machine_layout"]={"filter":added["layout"]["filter"]+","+getter["layout"]["filter"]}
    return added


def verify_execution(lines,layout,expected=None):
    entry=layout["entry"]
    attempts,finished=callback_attempts(lines,entry)
    require(len(finished)==expected if expected is not None else len(finished)>=81,
            "wrong executed original RX entry count")
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
                    (layout["peer"] is None or not layout["peer"].startswith(prefix.hex())),"wrong translated add_recvbuf_small")
            translations.append(i)
    require(len(translations)==1 and translations[0]<attempts[0][0] and all(r==attempts[0][1] for _,r in attempts)
            and not any(EXEC.match(s) or s.startswith(b"IN:") for s in lines[translations[0]+1:attempts[0][0]]),
            "executed block not bound to original callback")
    return len(finished)


def snapshot(ram,pkg):
    m=load(pkg/"manifest.json")
    require(m["linux_net_packets"]["driver_slot"]==0 and m["linux_net_packets"]["unauthorized_slots"]==[1] and
            m["linux_net_packets"]["virtio_net_header_bytes"]==12 and
            (m["board"],m["profile"],m["policy"])==("virt","native","fifo") and len(m["slots"])==2,
            "packet ownership profile")
    require(m["pipes"] is True and m["pipe_links"]==[
            dict(writer=0,writer_fd=3,reader=1,reader_fd=3),dict(writer=1,writer_fd=4,reader=0,reader_fd=4)],
            "native two-role pipe endpoints")
    require(m["linux_source_net_rx"] == dict(driver_slot=0,consumer_slot=1,unauthorized_slots=[1],
            driver_source="drivers/net/virtio_net.c",queue_size=32,grant_bytes=1530,virtio_net_header_bytes=12,
            frame_count=80,frame_ethertype=0x88b5,syscalls={"start":170,"post":171,"reap":172}),
            "native RX grant manifest")
    kernel=pkg/"kernel.elf"
    ring,buf=dma_layout(pkg)
    protected=set(range(ring,ring+8192,4096))|set(range(buf&~4095,buf+64*STRIDE,4096))
    user_pages=[]
    receipts=[]
    peer_raw=[]
    user_mappings=[]
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
            ctrl,extent=journal_location(elf,"jnox_rx_owner_controls")
            require(extent==32 and ctrl%8==0 and 0x400000<=ctrl<ctrl+32<=0x800000 and
                    (va+size<=ctrl or ctrl+32<=va),"owner control symbol extent")
            for vpn in range(ctrl//4096,(ctrl+31)//4096+1):
                user_pages[i].add(int(permissions(ram,root,vpn*4096,"user rw"),16)&PA_MASK)
            words.extend(struct.unpack("<4Q",vread(ram,root,ctrl,32)))
        if i==1:
            spans=[(va,va+size)]
            for name,n in (("jnox_rx_peer_frames",80*91),("jnox_rx_peer_ack",60)):
                addr,extent=journal_location(elf,name)
                require(extent==n and 0x400000<=addr<addr+n<=0x800000 and
                        all(addr+n<=a or addr>=b for a,b in spans),"peer Ethernet journal extent")
                spans.append((addr,addr+n))
                for vpn in range(addr//4096,(addr+n-1)//4096+1):
                    user_pages[i].add(int(permissions(ram,root,vpn*4096,"user rw"),16)&PA_MASK)
                peer_raw.append(vread(ram,root,addr,n))
        receipts.append(words)
        permissions(ram,root,NIC&~4095,"kernel rw",device=True)
        for pa in protected:
            permissions(ram,root,pa,"kernel rw")
        aliases=set()
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
                    elif desc&64:
                        aliases.add(desc&PA_MASK)
                        require((desc&PA_MASK) not in protected,"EL0 alias of protected DMA operand")
            tables=children
        user_mappings.append(aliases)
    require(user_pages[1].isdisjoint(user_mappings[0]) and user_pages[0].isdisjoint(user_mappings[1]),
            "private Ethernet or driver receipt pages exposed to other role")
    require(user_pages[0].isdisjoint(user_pages[1]) and not (user_pages[0]|user_pages[1])&protected,
            "private operand alias")
    return receipts,[area(ram,ring+i*4096,4096) for i in (0,1)],area(ram,buf,64*STRIDE),peer_raw

def frames(mac, nonce):
    records=[]
    for seq in range(COUNT):
        header=mac+HOST+TYPE+b"JNOXRECV"+nonce+seq.to_bytes(4,"big")
        records.append(header+bytes((seq+i)&255 for i in range(34,60+seq%32)))
    return records


def ring_state(stage,rings,buffers,base,records,posts):
    rx,tx=rings
    used={"ready":0,"active":80,"final":81}[stage]
    assert len(records)==81
    u16=lambda raw,off:struct.unpack_from("<H",raw,off)[0]
    require((u16(rx,514),u16(rx,586))==(posts,used) and
            (u16(tx,514),u16(tx,586))==(0,0) and not any(tx[:512]),"RX/TX available or used ring state")
    require(32<=posts<=32+used,"unexpected original refill count")
    for i in range(32):
        addr,n,flags,nxt=struct.unpack_from("<QIHH",rx,16*i)
        require((addr,n,flags,nxt)==(base+i*STRIDE,RX_CAPACITY,2,0),"RX descriptor DMA bounds/length")
    if stage=="ready":
        require([u16(rx,516+i*2) for i in range(32)]==list(range(32)) and not any(buffers),
                "original initial grants or clean DMA buffers")
    retained={}
    for seq in range(max(0,used-32),used):
        pos=seq%32
        idx,n=struct.unpack_from("<II",rx,588+pos*8)
        require(idx<32 and n==12+len(records[seq]),"device used length or descriptor ID")
        retained[idx]=seq
    if stage=="active":
        require(len(retained)==32,"expected 32 distinct retained RX completions")
        for idx,seq in retained.items():
            off=idx*STRIDE+12
            require(buffers[off:off+len(records[seq])]==records[seq],"RX DMA bytes differ from original Ethernet")
    if stage=="final":
        require(not any(buffers),"retirement did not clear DMA storage")


def mmio(lines,pool,driver,posts):
    regs=[]
    for line in lines:
        if b"memory_region_ops_" not in line:
            continue
        m=re.search(rb"memory_region_ops_(read|write) cpu (-?\d+) mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size (\d+)",line,re.I)
        require(m,"malformed MMIO trace")
        off=int(m[3],16)-NIC
        if 0<=off<512:
            require(m[2]==b"0","NIC access outside CPU0")
            regs.append((m[1].decode(),off,int(m[4],16),int(m[5])))
    require([v for op,r,v,w in regs if (op,r,w)==("write",112,4)]==[0,1,3,11,15,0],"device status lifecycle")
    selected=None
    writes=[[],[]]
    ready=[[],[]]
    maxes=[set(),set()]
    notify=acked=last_irq=0
    status=None
    seen_status=set()
    for op,reg,value,width in regs:
        if reg==112 and op=="write":
            status=value
        if reg==112 and op=="read":
            require(value==status,"unexpected transport status readback")
            seen_status.add(value)
        if reg==48 and op=="write":
            require(width==4 and value in (0,1),"invalid queue selection")
            selected=value
        if op=="write" and reg in (56,68,128,132,144,148,160,164):
            require(selected is not None and width==4,"unselected queue write")
            writes[selected].append((reg,value))
        if op=="read" and reg in (52,68):
            require(selected is not None and width==4,"unselected queue read")
            if reg==68:
                ready[selected].append(value)
            else:
                maxes[selected].add(value)
        if (op,reg)==("write",80):
            require(value==0 and status==15,"queue notification outside RX ownership")
            notify+=1
        if (op,reg)==("read",96):
            last_irq=value&3
        if (op,reg)==("write",100):
            require(status==15 and 0<value==last_irq,"interrupt ack differs from device status")
            acked|=value
            last_irq=0
    for idx in (0,1):
        base=pool+4096*idx
        wanted=[(56,32)]
        for reg,offset in ((128,0),(144,512),(160,584)):
            addr=base+offset
            wanted.extend([(reg,addr&0xffffffff),(reg+4,addr>>32)])
        require(writes[idx]==wanted+[(68,1)] and ready[idx][0]==ready[idx][-1]==0 and 1 in ready[idx],
                "queue programming or confirmed reset")
        require(maxes[idx]=={driver[24+idx]>>32} and (driver[24+idx]&0xffffffff)==((32<<16)|1),
                "queue capacity or grant mismatch")
    require({0,11,15} <= seen_status and notify==1+posts-32 and driver[29]==acked and acked&1,
            "live notifications or polled hardware IRQ acknowledgement")
    return notify,acked


def replay(pkg,imp,out):
    meta=provenance(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["identities"]==meta and launch["schema"]==1 and (out/"net.events").read_text()==TRACE,
            "import or trace-selection identity")
    cmd=launch["argv"]
    require(cmd==model_command(cmd[0],Path(launch["out"]),Path(launch["kernel"]),launch["fd"],launch["mac"],
                               launch["mtu"],meta["machine_layout"]),"noncanonical native board launch")
    mac=bytes.fromhex(launch["mac"].replace(":",""))
    nonce=(out/"nonce.bin").read_bytes()
    require(len(nonce)==8 and any(nonce),"invalid external nonce")
    data=frames(mac,nonce)
    ack=challenge(mac,nonce,True)
    records=[*data,ack]
    require(socket_frames((out/"host-to-guest.bin").read_bytes())==records and
            not (out/"guest-to-host.bin").read_bytes(),"host socket frame sequence, bytes or direction")
    wire=pcap_frames(out/"wire.pcap")
    require(len(wire)==len(records) and all(a in (b,bytes(10)+b,bytes(12)+b) for a,b in zip(wire,records)),
            "Ethernet PCAP length, order, nonce or payload")
    ring,buf=dma_layout(pkg)
    states=[]
    expected_peer=b"".join(f.ljust(91,b"\0") for f in data)
    for stage,phase in PHASES:
        where=out/stage
        qmp,halt=load(where/"qmp.json"),load(where/"halted.json")
        require(qmp["status"]["running"] and [c["cpu-index"] for c in qmp["cpus"]]==[0] and
                qmp["stop_return"]=={} and halt["status"]=="paused" and not halt["running"],"CPU stop witness")
        ram=RAM(load(where/"ram.json"))
        (d,p),rings,buffers,(pframes,pack)=snapshot(ram,pkg)
        require(set(ram.records)==ram.used and d[:3]==[0x4a4e525844525631,phase,0],"driver phase or physical read set")
        if stage=="ready":
            require(not any(pframes) and not any(pack) and d[10]==0 and
                    p[:6]==[0x4a4e525850454531,3,0,NEG-13,NEG-13,NEG-13],
                    "peer grant denial and pre-injection journal")
        else:
            require(pframes==expected_peer and (pack==ack if stage=="final" else not any(pack)),
                    "private peer-owned frame journal")
            require(d[10:14]==[80,sum(map(len,data)),80,79] and p[:3]==
                    [0x4a4e525850454531,phase,0] and p[6:11]==
                    [80,sum(map(len,data)),int.from_bytes(nonce,"big"),79,int.from_bytes(mac,"big")],
                    "peer or driver data-count receipts")
        posts=struct.unpack_from("<H",rings[0],514)[0]
        wanted_posts={"ready":32,"active":112,"final":113}[stage]
        require(posts==wanted_posts,"original grant count")
        ring_state(stage,rings,buffers,buf,records,posts)
        states.append((d,p))
    d,p=states[-1]
    require(states[0][0][3]==states[1][0][3]==d[3]>0 and d[4:10]==[(1<<32)|40,int.from_bytes(mac,"big"),0,0,15,
            int.from_bytes(nonce,"big")] and d[14:22]==[0xa6,0,NEG-116,NEG-116,0,launch["mtu"],1,0] and
            d[26:29]==[11,60,NEG-19] and d[30:36]==[NEG-95,NEG-95,NEG-11,NEG-90,NEG-14,NEG-16] and p[3:6]==[NEG-13]*3 and p[11]==60,"grant lifecycle, peer denial or terminal ACK")
    uart=(out/"uart.log").read_bytes().splitlines()
    for slot,prefix in zip(load(pkg/"manifest.json")["slots"],(b"NATIVE EL0 ELF ",b"NATIVE EL0 PEER ELF ")):
        require(prefix+slot["elf_sha256"].encode() in uart,"booted ELF")
    require({x for x in uart if x.startswith(b"NATIVE EL0 EXIT PID ")}==
            {b"NATIVE EL0 EXIT PID 0 0",b"NATIVE EL0 EXIT PID 1 0"} and
            not any(b"NATIVE EL0 FAULT" in x for x in uart),"native exits")
    path=out/"machine.log"
    require(path.stat().st_size<=MAX_LOG,"oversize QEMU trace")
    lines=path.read_bytes().splitlines()
    entries=verify_execution(lines,meta["layout"],posts)
    get_entries=verify_execution(lines,meta["get_buf_layout"])
    kicks,irq=mmio(lines,ring,d,posts)
    return dict(original_add_recvbuf_small_entries=entries,original_virtnet_rq_get_buf_entries=get_entries,rx_frames=80,ethernet_bytes=sum(map(len,data)),
                hw_used_including_ack=81,rx_notifications=kicks,polled_interrupt_bits=irq,
                peer_private_frames=80,original_virtnet_poll_linked=False,original_gro_execution_verified=False)


def capture(pkg,imp,out,binary,mtu,mac,timeout):
    qemu=shutil.which(binary)
    require(qemu,"missing QEMU")
    meta=provenance(pkg,imp,True)
    root=linked_roots(pkg/"kernel.elf",2)[0]
    va,_=journal_location(pkg/"user.elf",PROBES[0][0])
    out.mkdir(parents=True,exist_ok=False)
    (out/"net.events").write_text(TRACE)
    nonce=secrets.token_bytes(8)
    while not any(nonce):
        nonce=secrets.token_bytes(8)
    (out/"nonce.bin").write_bytes(nonce)
    macbytes=bytes.fromhex(mac.replace(":",""))
    end=time.monotonic()+timeout
    host,device=socket.socketpair(socket.AF_UNIX,socket.SOCK_STREAM)
    with host,device,(out/"stderr.log").open("xb") as stderr:
        fd=device.fileno()
        cmd=model_command(qemu,out,pkg/"kernel.elf",fd,mac,mtu,meta["machine_layout"])
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
                    (d,_p),_,_,_=snapshot(mem,pkg)
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
                    ring,_=dma_layout(pkg)
                    for i,frame in enumerate(frames(macbytes,nonce),1):
                        healthy()
                        send(frame,sent)
                        if i%8==0:
                            # Advance only after these used entries have been reaped and refilled.
                            while int.from_bytes(read(ring+514,2),"little")<32+i:
                                healthy()
                                time.sleep(.002)
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
    results=[]
    with tempfile.TemporaryDirectory(prefix="jnox-rx-controls-") as tmp:
        work=Path(tmp)/"capture"
        shutil.copytree(out,work)
        def reject(label,path,data,reason):
            before=path.read_bytes()
            try:
                path.write_bytes(data)
                try:
                    replay(pkg,imp,work)
                except ValueError as exc:
                    require(reason in str(exc),label+" failed at unrelated check: "+str(exc))
                else:
                    raise ValueError("accepted mutation "+label)
            finally:
                path.write_bytes(before)
            results.append(label)
        for filename,reason in (("host-to-guest.bin","host socket"),("wire.pcap","Ethernet PCAP")):
            path=work/filename
            payload=bytearray(path.read_bytes())
            payload[-1]^=1
            reject(filename,path,payload,reason)
        reads=work/"active"/"ram.json"
        ring,_=dma_layout(pkg)
        original=load(reads)
        key=f"{ring:x}/4096"
        raw=bytearray.fromhex(original[key])
        struct.pack_into("<H",raw,586,79)
        reject("hardware used count",reads,json.dumps(dict(original,**{key:raw.hex()})).encode(),"used ring state")
        for slot,symbol,delta,reason in (
                (1,"jnox_rx_peer_frames",0,"private peer-owned frame journal"),
                (1,"jnox_rx_peer_result",24,"grant lifecycle"),
                (0,"jnox_rx_owner_controls",0,"grant lifecycle")):
            stage="active" if symbol=="jnox_rx_peer_frames" else "final"
            path=work/stage/"ram.json"
            data=load(path)
            addr,size=journal_location(pkg/("native_peer.elf" if slot else "user.elf"),symbol)
            ram=RAM(data)
            root=linked_roots(pkg/"kernel.elf",2)[slot]
            leaf=ram.leaf(root,addr)
            key=f"{(leaf&PA_MASK)+addr%4096:x}/{min(size,4096-addr%4096)}"
            payload=bytearray.fromhex(data[key])
            require(delta<len(payload),"mutation offset crosses page")
            payload[delta]^=1
            data[key]=payload.hex()
            reject(symbol,path,json.dumps(data).encode(),reason)
        log=work/"machine.log"
        lines=log.read_bytes().splitlines(keepends=True)
        layout=load(work/"launch.json")["identities"]["layout"]
        _,entries=callback_attempts(lines,layout["entry"])
        i=entries[-1]
        reject("removed original refill",log,b"".join(lines[:i]+lines[i+1:]),"entry count")
        getter=load(work/"launch.json")["identities"]["get_buf_layout"]
        _,gets=callback_attempts(lines,getter["entry"])
        removed=set(gets[80:])
        reject("missing original getter coverage",log,b"".join(l for n,l in enumerate(lines) if n not in removed),"entry count")
    return results


if __name__=="__main__":
    cli=argparse.ArgumentParser(description=__doc__)
    for key in ("package","driver-manifest","out"):
        cli.add_argument("--"+key,type=Path,required=True)
    cli.add_argument("--mtu",type=int)
    cli.add_argument("--mac",type=str.lower)
    cli.add_argument("--qemu",default="qemu-system-aarch64")
    cli.add_argument("--timeout",type=float,default=90)
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
