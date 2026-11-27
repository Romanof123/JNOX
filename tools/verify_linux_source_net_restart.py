#!/usr/bin/env python3
"""Capture one native VirtIO owner exit, verified retirement and isolated replacement."""
import argparse
import hashlib
import json
import os
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
from verify_linux_source_net import NEG, NIC, RAM, callback_attempts, load, require, sha, save
from verify_linux_source_net_packets import (MAX_LOG, STRIDE, area, challenge, dma_layout,
                                            model_command, pcap_frames, socket_frames)
from verify_linux_source_net_queues import routine

STAGES=("preexit","middle","spare","active","final")
GATES=(0x45584954,0x42494e44)
D=("jnox_restart_driver","jnox_restart_grant_addresses","jnox_restart_callback",
   "jnox_restart_poll","jnox_restart_skb")
P=("jnox_restart_peer","jnox_restart_gates","jnox_restart_primary_frame",
   "jnox_restart_frames","jnox_restart_ack")
K=("jnox_linux_net_restart_journal","jnox_linux_net_restart_scheduler","jnox_linux_net_irq_metrics",
   "jnox_linux_net_restart_roles","jnox_native_startup_slots","jnox_linux_net_restart_grants")
FUNCS=("virtnet_find_vqs","skb_recv_done","virtnet_poll","gro_receive_skb")
TRACE=irq.TRACE


def words(b):
    return list(struct.unpack(f"<{len(b)//8}Q",b))


def layout(pkg,imp):
    manifest=load(pkg/"manifest.json")
    intake=load(imp)
    require(intake["mode"]=="rx-restart-service" and intake["source"]["sha256"]==intake["upstream"]["sha256"]==rx.PIN,
            "original Linux TU identity")
    require(manifest["linux_source_net_restart"]==dict(primary_slot=0,survivor_slot=1,dormant_spare_slot=2,
            intid=79,max_replacements=1) and len(manifest["slots"])==3 and manifest["policy"]=="fifo","replacement topology")
    kernel=pkg/"kernel.elf"
    kernel_bytes=kernel.read_bytes()
    require(sha(kernel)==manifest["kernel_elf_sha256"],"kernel ELF digest")
    artifacts={}
    for name,expected in (("virtio_net.packet.o",intake["driver_object"]["sha256"]),
                          ("virtio_net.callback.o",intake["callback_object"]["sha256"]),
                          ("virtio_net.original.o",intake["objects"]["virtio_net.original.o"]["sha256"])):
        require(sha(imp.parent/name)==expected,"original object digest")
        artifacts[name]=expected
    roots=linked_roots(kernel,3)
    syms=[]
    callbacks=[]
    role_ids=[]
    for slot,record in enumerate(manifest["slots"]):
        app=pkg/record["elf"]
        r=intake["restart_roles"][slot]
        require(record["slot"]==r["slot"]==slot and sha(app)==record["elf_sha256"]==r["sha256"] and
                app.read_bytes() in kernel_bytes and sha(imp.parent/r["map"])==r["map_sha256"],"embedded role identity")
        role_ids.append(r["sha256"])
        names=P if slot==1 else D
        syms.append({n:journal_location(app,n) for n in names})
        funcs={}
        if slot!=1:
            linkmap=(imp.parent/r["map"]).read_text().splitlines()
            with app.open("rb") as f:
                elf=ELFFile(f)
                for name in FUNCS:
                    supplier="rx_skb.o" if name=="gro_receive_skb" else "virtio_net.callback.o"
                    require(any(l.strip().endswith(f"{supplier}:(.text.{name})") for l in linkmap),"callback link map")
                    sym,body=routine(elf,name)
                    objname="virtio_net.packet.o" if name!="gro_receive_skb" else supplier
                    with (imp.parent/objname).open("rb") as g:
                        obj=ELFFile(g)
                        osym,code=routine(obj,name)
                        reloc={r["r_offset"]//4 for s in obj.iter_sections() if isinstance(s,RelocationSection)
                               and s["sh_info"]==osym["st_shndx"] for r in s.iter_relocations()}
                    require(len(body)==len(code) and all(body[i:i+4]==code[i:i+4] for i in range(0,len(body),4)
                            if i//4 not in reloc),"original linked instructions")
                    funcs[name]=dict(entry=sym["st_value"],size=len(body),prefix=body[:16].hex(),peer=None)
        callbacks.append(funcs)
    require(len(set(role_ids))==3 and len(set(roots))==3,"same-root or duplicate process instance")
    ring,buffers=dma_layout(pkg)
    entries=[]
    for record in manifest["slots"]:
        with (pkg/record["elf"]).open("rb") as f:
            entries.append(ELFFile(f)["e_entry"])
    return dict(roots=roots,entries=entries,user=syms,kernel={n:journal_location(kernel,n) for n in K},
                ring=ring,buffers=buffers,routines=callbacks,elf=role_ids,objects=artifacts,
                kernel_sha256=sha(kernel),manifest_sha256=sha(pkg/"manifest.json"),import_sha256=sha(imp))


def snapshot(mem,L,watch=None):
    users=[]
    physical=[]
    for slot,(root,syms) in enumerate(zip(L["roots"],L["user"])):
        data={}
        pages={}
        for name,(va,size) in syms.items():
            for page in range(va//4096,(va+size-1)//4096+1):
                pages[page]=int(permissions(mem,root,page*4096,"user rw"),16)&PA_MASK
            data[name]=vread(mem,root,va,size)
        users.append(data)
        physical.append(set(pages.values()))
    require(all(not physical[i]&physical[j] for i,j in ((0,1),(1,2),(0,2))),"shared private role page")
    protected=[(L["ring"],8192),(L["buffers"],64*STRIDE),*L["kernel"].values()]
    for base,size in protected:
        for p in range(base//4096,(base+size-1)//4096+1):
            for root in L["roots"]:
                permissions(mem,root,p*4096,"kernel rw")
    kernel={n:words(area(mem,pa,size)) for n,(pa,size) in L["kernel"].items()}
    roles=kernel[K[3]]
    require(len(roles)==21,"observed role journal extent")
    for slot,root in enumerate(L["roots"]):
        row=roles[slot*7:(slot+1)*7]
        for tick,pc,actual in (row[:3],row[4:7]):
            if tick:
                require(actual==root and 0x400000<=pc-4<0x800000,"same-root or wrong live SVC TTBR0")
                permissions(mem,root,pc-4,"user rx")
                require(int.from_bytes(vread(mem,root,pc-4,4),"little")==(0xd4000001|(0x4a01<<5)),
                        "recorded EL0 PC does not follow native SVC")
    rings=area(mem,L["ring"],8192)
    buffers=area(mem,L["buffers"],64*STRIDE)
    if watch is None:
        used_id=struct.unpack_from("<I",rings,596)[0]
        require(used_id<32,"pending descriptor ID")
        watch=kernel[K[5]][3*used_id+1]
    require(0x400000<=watch<watch+1530<=0x800000,"old private RX span")
    for vpn in range(watch//4096,(watch+1529)//4096+1):
        physical[0].add(int(permissions(mem,L["roots"][0],vpn*4096,"user rw"),16)&PA_MASK)
    privileged={page*4096 for addr,size in protected for page in range(addr//4096,(addr+size-1)//4096+1)}
    for i,root in enumerate(L["roots"]):
        tables={root&PA_MASK}
        forbidden=privileged.union(*(physical[j] for j in range(3) if j!=i))
        for level in range(3):
            children=set()
            for table in tables:
                for (desc,) in struct.iter_unpack("<Q",mem.read(table,4096)):
                    if not desc&1:
                        continue
                    require(desc&3==3,"unexpected native mapping")
                    if level<2:
                        children.add(desc&PA_MASK)
                    elif desc&64:
                        require((desc&PA_MASK) not in forbidden,"user alias of another role or kernel DMA")
            tables=children
    old=vread(mem,L["roots"][0],watch,1530)
    gate=L["user"][1][P[1]][0]
    gatepa=(int(permissions(mem,L["roots"][1],gate,"user rw"),16)&PA_MASK)+(gate%4096)
    require(gate%8==0 and gate%4096<=4080,"gate crosses page")
    return dict(users=users,kernel=kernel,rings=rings,buffers=buffers,old=old,watch=watch,gatepa=gatepa)


def rsp(stream,command):
    raw=command.encode("ascii")
    stream.write(b"$"+raw+b"#"+f"{sum(raw)&255:02x}".encode())
    acknowledged=False
    while True:
        prefix=stream.read(1)
        if prefix==b"+":
            acknowledged=True
            continue
        require(prefix==b"$","GDB reply prefix")
        reply=bytearray()
        while True:
            ch=stream.read(1)
            require(ch,"GDB disconnected")
            if ch==b"#":
                break
            reply+=ch
        require(int(stream.read(2),16)==sum(reply)&255,"GDB reply checksum")
        stream.write(b"+")
        if reply[:1] in (b"S",b"T"):
            continue
        require(acknowledged,"GDB command acknowledgement")
        return reply.decode("ascii")


def authorize_gate(stream,read,stage,shot,out):
    i=("preexit","middle").index(stage)
    pa=shot["gatepa"]+8*i
    require(read(pa,8)==bytes(8),"survivor gate already released")
    require(rsp(stream,"Qqemu.PhyMemMode:1")=="OK" and rsp(stream,"qqemu.PhyMemMode")=="1","GDB physical mode")
    data=struct.pack("<Q",GATES[i])
    cmd=f"M{pa:x},8:{data.hex()}"
    require(rsp(stream,cmd)=="OK" and read(pa,8)==data,"survivor gate write readback")
    save(out/stage/"gate.json",dict(physical=pa,command=cmd,before=bytes(8).hex(),after=data.hex()))


def command(binary,out,kernel,fd,mac,mtu,L):
    filters=sorted({(v["entry"],v["size"]) for fs in L["routines"] for v in fs.values()})
    filters=sorted(set(filters)|{(pc,4) for pc in L["entries"]})
    ranges=",".join(f"0x{a:x}+0x{n:x}" for a,n in filters)
    return [*model_command(binary,out,kernel,fd,mac,mtu,{"filter":ranges}),"-chardev",
            f"socket,path={out/'gate.sock'},server=on,wait=off,id=gate","-gdb","chardev:gate"]


def capture(pkg,imp,out,binary,mac,mtu,timeout):
    L=layout(pkg,imp)
    binary=shutil.which(binary)
    require(binary,"QEMU missing")
    out.mkdir(parents=True,exist_ok=False)
    (out/"net.events").write_text(TRACE)
    old=secrets.token_bytes(8)
    while not any(old):
        old=secrets.token_bytes(8)
    new=secrets.token_bytes(8)
    while new==old or not any(new):
        new=secrets.token_bytes(8)
    (out/"nonce.bin").write_bytes(old+new)
    addr=bytes.fromhex(mac.replace(":",""))
    initial=rx.frames(addr,old)
    frames=rx.frames(addr,new)
    deadline=time.monotonic()+timeout
    host,nic=socket.socketpair()
    with host,nic,socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as gdb,(out/"stderr.log").open("xb") as err:
        fd=nic.fileno()
        cmd=command(binary,out,pkg/"kernel.elf",fd,mac,mtu,L)
        with subprocess.Popen(cmd,pass_fds=(fd,),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,bufsize=0) as proc:
            nic.close()
            class Pipe:
                write=proc.stdin.write
                readline=proc.stdout.readline
            q=Pipe()
            watchdog=threading.Timer(timeout+10,proc.kill)
            watchdog.start()
            save(out/"launch.json",dict(schema=1,argv=cmd,kernel=str(pkg/"kernel.elf"),out=str(out),fd=fd,mac=mac,mtu=mtu,
                                        pid=proc.pid,qemu_sha256=sha(Path(binary)),roles=L["elf"],import_sha256=sha(imp)))
            def read(pa,size):
                request(q,"pmemsave",val=pa,size=size,filename=str(out/"probe.bin"))
                return (out/"probe.bin").read_bytes()
            def healthy():
                uart=(out/"uart.log").read_bytes() if (out/"uart.log").exists() else b""
                require(proc.poll() is None and time.monotonic()<deadline and b"NATIVE EL0 FAULT" not in uart,"QEMU timeout or native fault")
                for pid,status in re.findall(rb"NATIVE EL0 EXIT PID (\d+) (-?\d+)",uart):
                    require(int(status)==(86 if pid==b"0" else 0),"unexpected guest exit")
                require(not (out/"machine.log").exists() or (out/"machine.log").stat().st_size<=MAX_LOG,"trace limit")
            def until(predicate):
                while True:
                    healthy()
                    if predicate():
                        return
                    time.sleep(.003)
            def field(slot,name,n):
                va=L["user"][slot][name][0]
                mem=RAM({},read)
                if mem.leaf(L["roots"][slot],va) is None:
                    return -1
                return int.from_bytes(vread(mem,L["roots"][slot],va+8*n,8),"little")
            def metric(n):
                return int.from_bytes(read(L["kernel"][K[2]][0]+8*n,8),"little")
            def stop(stage,watch):
                dest=out/stage
                dest.mkdir()
                capture_halt(q,dest)
                mem=RAM({},read)
                try:
                    return snapshot(mem,L,watch)
                finally:
                    save(dest/"ram.json",mem.records)
            try:
                require("QMP" in json.loads(q.readline()),"QMP greeting")
                request(q,"qmp_capabilities")
                with (out/"host.bin").open("xb") as log:
                    def send(frame):
                        blob=len(frame).to_bytes(4,"big")+frame
                        host.settimeout(max(.001,deadline-time.monotonic()))
                        host.sendall(blob)
                        log.write(blob)
                        log.flush()
                    until(lambda:metric(22)==1 and field(0,D[0],1)==2)
                    send(initial[0])
                    until(lambda:field(1,P[0],1)==2)
                    send(initial[1])
                    until(lambda:int.from_bytes(read(L["ring"]+586,2),"little")==2 and field(1,P[0],9)>0)
                    pre=stop("preexit",None)
                    gdb.settimeout(max(.001,deadline-time.monotonic()))
                    gdb.connect(str(out/"gate.sock"))
                    with gdb.makefile("rwb",buffering=0) as channel:
                        authorize_gate(channel,read,"preexit",pre,out)
                        request(q,"cont")
                        until(lambda:field(1,P[0],1)==3 and field(1,P[0],10)>0)
                        mid=stop("middle",pre["watch"])
                        authorize_gate(channel,read,"middle",mid,out)
                        request(q,"cont")
                        until(lambda:field(2,D[0],1)==2 and metric(22)==1)
                        stop("spare",pre["watch"])
                        for f in frames[:16]:
                            send(f)
                        request(q,"cont")
                        until(lambda:field(1,P[0],7)>=16)
                        for i,f in enumerate(frames[16:],17):
                            send(f)
                            if i%8==0:
                                until(lambda:field(1,P[0],7)>=i)
                        until(lambda:field(2,D[0],1)==3 and metric(22)==1)
                        stop("active",pre["watch"])
                        request(q,"cont")
                        send(challenge(addr,new,True))
                        until(lambda:len(re.findall(rb"(?m)^NATIVE EL0 EXIT PID [012] (?:0|86)\r?$",(out/"uart.log").read_bytes()))==3)
                        stop("final",pre["watch"])
                        request(q,"quit")
                        require(proc.wait(timeout=5)==0 and host.recv(1)==b"","QEMU shutdown or outbound packet")
                (out/"probe.bin").unlink(missing_ok=True)
            except Exception as exc:
                save(out/"failure.json",dict(error=repr(exc),returncode=proc.poll()))
                raise
            finally:
                watchdog.cancel()
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
                watchdog.join()


def mmio(lines,L):
    reg=[]
    for i,line in enumerate(lines):
        m=re.search(rb"memory_region_ops_(read|write) cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size (\d+)",line)
        if m and NIC<=int(m[2],16)<NIC+512:
            reg.append((i,m[1],int(m[2],16)-NIC,int(m[3],16),int(m[4])))
    status=[(i,v) for i,op,r,v,w in reg if (op,r,w)==(b"write",112,4)]
    require([v for _,v in status]==[0,1,3,11,15,0]*2,"two hardware owner lifecycles")
    for start,live in ((0,4),(6,10)):
        selected=None
        written=[[],[]]
        read_ready=[[],[]]
        for pos,op,r,v,w in reg:
            if not status[start][0]<pos<status[live][0]:
                continue
            if op==b"write" and r==48:
                require(v in (0,1) and w==4,"invalid queue selection")
                selected=v
            if r in (56,68,128,132,144,148,160,164) and op==b"write":
                require(selected is not None and w==4,"queue register write extent")
                written[selected].append((r,v))
            if selected is not None and r==68 and op==b"read":
                read_ready[selected].append(v)
        for queue in (0,1):
            base=L["ring"]+queue*4096
            expected=[(56,32)]
            for r,off in ((128,0),(144,512),(160,584)):
                expected.extend([(r,(base+off)&0xffffffff),(r+4,(base+off)>>32)])
            require(written[queue]==expected+[(68,1)] and 1 in read_ready[queue],"instance queue address programming")
    for n in (5,11):
        end=status[n+1][0] if n+1<len(status) else len(lines)
        selected=None
        reset=False
        queues=set()
        for i,op,r,v,w in reg:
            if not status[n][0]<i<end or w!=4:
                continue
            if op==b"read" and r==112 and v==0:
                reset=True
            if r==48 and op==b"write":
                selected=v
            if r==68 and op==b"read" and reset and v==0:
                queues.add(selected)
        require(reset and queues.issuperset((0,1)),"retirement status or queue-ready readback")
    return status


def replay(pkg,imp,out):
    L=layout(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["schema"]==1 and launch["roles"]==L["elf"] and launch["import_sha256"]==sha(imp) and
            launch["argv"]==command(launch["argv"][0],Path(launch["out"]),Path(launch["kernel"]),launch["fd"],
                                    launch["mac"],launch["mtu"],L) and (out/"net.events").read_text()==TRACE,
            "launch or trace profile")
    nonce=(out/"nonce.bin").read_bytes()
    old,new=nonce[:8],nonce[8:]
    require(len(nonce)==16 and old!=new and any(old) and any(new),"reused host nonce")
    mac=bytes.fromhex(launch["mac"].replace(":",""))
    primary=rx.frames(mac,old)[:2]
    fresh=rx.frames(mac,new)
    ack=challenge(mac,new,True)
    expected=[*primary,*fresh,ack]
    require(socket_frames((out/"host.bin").read_bytes())==expected,"host ingress differs")
    packets=pcap_frames(out/"wire.pcap")
    require(len(packets)==len(expected) and all(p in (f,bytes(10)+f,bytes(12)+f) for p,f in zip(packets,expected)),
            "NIC PCAP ingress differs")
    shots={}
    watch=None
    for name in STAGES:
        dest=out/name
        q=load(dest/"qmp.json")
        h=load(dest/"halted.json")
        require(q["status"]["running"] and q["stop_return"]=={} and [c["cpu-index"] for c in q["cpus"]]==[0]
                and h["status"]=="paused" and not h["running"],"QMP checkpoint")
        mem=RAM(load(dest/"ram.json"))
        s=snapshot(mem,L,watch)
        watch=s["watch"]
        require(mem.used==set(mem.records),"unaccounted physical read")
        shots[name]=s
    pre,mid,spare,active,end=[shots[n] for n in STAGES]
    for i,name in enumerate(STAGES[:2]):
        s=shots[name]
        gates=words(s["users"][1][P[1]])
        require(gates==([0,0] if i==0 else [GATES[0],0]),"gate sequencing")
        gate=load(out/name/"gate.json")
        addr=s["gatepa"]+8*i
        data=struct.pack("<Q",GATES[i]).hex()
        require(gate==dict(physical=addr,command=f"M{addr:x},8:{data}",before=bytes(8).hex(),after=data),"unapproved gate write")
    oldtoken=words(pre["users"][0][D[0]])[3]
    token=words(spare["users"][2][D[0]])[3]
    require(0<oldtoken<token,"queue generation reuse")
    for name,s in shots.items():
        u=s["users"]
        d,p,r=words(u[0][D[0]]),words(u[1][P[0]]),words(u[2][D[0]])
        require(s["gatepa"]==pre["gatepa"] and s["old"]==pre["old"],"old driver's private RX destination changed")
        require(u[1][P[2]]==primary[0] and p[3]==oldtoken and p[5]==int.from_bytes(old,"big")
                and p[20:23]==[NEG-13]*3,"survivor old delivery or authority")
        require(d[:3]==[0x5245535444525630,3 if name=="preexit" else 86,0 if name=="preexit" else 86] and
                d[3]==oldtoken and d[5:8]==[int.from_bytes(old,"big"),1,60],"primary exited or consumed unexpected work")
        if name=="preexit":
            require(not any(r) and p[1]==2,"spare ran before exit")
        else:
            require(r[0]==0x5245535444525632 and r[1]=={"middle":1,"spare":2,"active":3,"final":100}[name],
                    "replacement did not start from dormant state")
        if name in ("preexit","middle","spare"):
            require(not any(u[1][P[3]]) and not any(u[1][P[4]]),"stale delivery before spare traffic")
        else:
            require(u[1][P[3]]==b"".join(f.ljust(91,b"\0") for f in fresh) and p[6:9]==[int.from_bytes(new,"big"),80,sum(map(len,fresh))]
                    and u[1][P[4]]==(ack if name=="final" else bytes(60)),"replacement consumer nonce or payload")
        if name in ("spare","active","final"):
            require(p[4]==token and p[23:26]==[NEG-13]*3 and r[3]==token and r[10:18]==
                    [oldtoken,int.from_bytes(old,"big")]+[NEG-116]*6 and words(u[1][P[1]])==list(GATES),
                    "stale grant or unauthorized peer accepted")
    require(p[1:3]==[100,0] and p[26]==r[18]==60 and r[19]==0 and r[2]==0,"final receipt")
    require(all(p[9+i]>0 and p[16+i]>=p[13+i]+5_000_000 for i in range(3)) and
            p[16]<p[14]<=p[17]<p[15]<=p[18],"survivor timer continuity")
    ring=pre["rings"]
    used_id,used_len=struct.unpack_from("<II",ring,596)
    require(struct.unpack_from("<H",ring,586)[0]==2 and used_len==12+len(primary[1]) and
            pre["buffers"][used_id*STRIDE+12:used_id*STRIDE+used_len]==primary[1],"unreaped DMA identity")
    grants=pre["kernel"][K[5]]
    require(grants[3*used_id:3*used_id+3]==[oldtoken,watch,1530] and not any(mid["kernel"][K[5]]) and
            not any(end["kernel"][K[5]]),"live or scrubbed RX grants")
    require(not any(mid["buffers"]) and not any(end["buffers"]),"retirement did not scrub DMA")
    require(primary[1] not in pre["old"],"unreaped frame already copied into primary destination")
    for label,s,total in (("ready",spare,0),("active",active,80)):
        rings=(s["rings"][:4096],s["rings"][4096:])
        irq.poll_gate.ring_state(label,rings,s["buffers"],L["buffers"],[*fresh,ack],total)
    require(not any(mid["rings"]) and not any(end["rings"]),"retired split rings not scrubbed")
    retired=end["kernel"][K[0]][48:]
    require(retired[7:10]==[81,81,81] and retired[12]==0 and retired[24:26]==[81,token] and
            28<=retired[6]-81<=32,"terminal pre-reset hardware head and harvested/delivered cursors")
    row=mid["kernel"][K[0]][:48]
    other=row[18]
    require(row[:6]==[1,0,1,86,oldtoken,15] and row[7:10]==[2,1,1] and row[12:17]==[1,used_id,used_len,watch,1530]
            and row[24]==2 and row[25]==oldtoken and row[17]==L["buffers"]+used_id*STRIDE and other!=used_id and other<32 and row[21]==oldtoken and
            row[22]&(1<<other) and row[23]&(1<<other) and row[23]&(1<<used_id),"retired grant or pending ownership")
    require(struct.unpack_from("<QIHH",ring,16*other)==(L["buffers"]+other*STRIDE,1530,2,0) and
            grants[3*other:3*other+3]==[oldtoken,row[20],1530],"distinct outstanding writable grant")
    require(row[19]==(1530|(2<<32)),"retired hardware descriptor length and flags")
    require(row[31]==0 and row[37:42]==[0,0,0,0,1] and end["kernel"][K[0]][:48]==row and
            end["kernel"][K[0]][48:54]==[1,2,1,0,token,15],"two retirement decisions")
    scheduler=end["kernel"][K[1]]
    require(0<scheduler[0]<scheduler[3] and scheduler[1:3]==[86,1] and scheduler[4]==0 and min(scheduler[5:7])>0,
            "FIFO restart decision or hardware wakes")
    require(row[26:29]==[oldtoken,0,1] and row[32:37]==[0]*5 and
            end["kernel"][K[0]][48+32:48+41]==[0]*9,"IRQ epoch revocation and reset")
    last=end["kernel"][K[3]]
    require(not any(pre["kernel"][K[3]][14:21]) and all(last[7*i] and last[7*i+4] for i in range(3)) and
            last[0]<last[4]<=last[14] and last[7]<last[4]<last[11] and not any(mid["kernel"][K[3]][18:21]) and
            [end["kernel"][K[4]][4*i] for i in range(3)]==L["entries"],"process first/exit SVC and survivor continuity")
    path=out/"machine.log"
    require(path.stat().st_size<=MAX_LOG,"emulator trace size")
    lines=path.read_bytes().splitlines()
    status=mmio(lines,L)
    epochs=irq.delivery_epochs(lines)
    irq.wire_delivery(lines,epochs,False)
    require(all(any(status[a][0]<e["iar"]<status[b][0] and e["status"]&1 for e in epochs) for a,b in ((4,5),(10,11))),
            "missing epoch-specific CPU IRQ")
    for pc in set(L["entries"]):
        _,starts=callback_attempts(lines,pc)
        require(len(starts)==L["entries"].count(pc) and
                sum(i>status[5][0] for i in starts)==int(pc==L["entries"][2]),"cold role entry execution")
    executions={}
    for slot,(lo,hi),s,count in ((0,(0,status[5][0]),pre,1),(2,(status[5][0],len(lines)),end,81)):
        u=s["users"][slot]
        cb,pol,skb=(words(u[n]) for n in D[2:])
        funcs=L["routines"][slot]
        require(cb[0]==funcs["skb_recv_done"]["entry"] and pol[0]==funcs["virtnet_poll"]["entry"] and
                cb[6]>=cb[3]>0 and pol[3]==pol[5]==count and skb==[count]*6+[0,0],"registered callback and NAPI accounting")
        for name,expected_count in zip(FUNCS,(1,cb[6],pol[2],count)):
            executions[f"{slot}/{name}"]=rx.verify_execution(lines[lo:hi],funcs[name],expected_count)
    uart=(out/"uart.log").read_bytes()
    require(set(re.findall(rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$",uart))=={(b"0",b"86"),(b"1",b"0"),(b"2",b"0")} and
            b"NATIVE EL0 FAULT" not in uart,"incorrect native exits")
    return dict(roots=L["roots"],epochs=[oldtoken,token],ingress_packets=len(packets),unreaped_id=used_id,outstanding_id=other,
                frozen_old_span_sha256=hashlib.sha256(pre["old"]).hexdigest(),scheduler=scheduler,executions=executions)


def controls(pkg,imp,out):
    L=layout(pkg,imp)
    rejected=[]
    with tempfile.TemporaryDirectory(prefix="jnox-restart-") as temp:
        work=Path(temp)/"capture"
        shutil.copytree(out,work,ignore=shutil.ignore_patterns("*.sock","probe.bin"))
        def reject(label,path,changed,reason):
            saved=path.read_bytes()
            try:
                path.write_bytes(changed)
                try:
                    replay(pkg,imp,work)
                except ValueError as exc:
                    require(reason in str(exc),label+" triggered unrelated rejection: "+str(exc))
                else:
                    raise ValueError("accepted corruption: "+label)
            finally:
                path.write_bytes(saved)
            rejected.append(label)
        def ram_word(stage,symbol,slot,index,value,label,reason):
            path=work/stage/"ram.json"
            records=load(path)
            if slot is None:
                addr,size=L["kernel"][symbol]
            else:
                addr,size=L["user"][slot][symbol]
            off=index*8
            while size:
                n=min(size,4096-addr%4096)
                if off<n:
                    pa=addr if slot is None else (RAM(records).leaf(L["roots"][slot],addr)&PA_MASK)+addr%4096
                    key=f"{pa:x}/{n}"
                    raw=bytearray.fromhex(records[key])
                    struct.pack_into("<Q",raw,off,value)
                    records[key]=raw.hex()
                    break
                size-=n
                off-=n
                addr+=n
            reject(label,path,json.dumps(records).encode(),reason)
        ram_word("final",K[3],None,16,L["roots"][0],"same-root spare","same-root")
        ram_word("final",D[0],2,13,0,"authorized stale grant","stale grant")
        path=work/"nonce.bin"
        nonces=path.read_bytes()
        reject("reused nonce",path,nonces[:8]*2,"reused host nonce")
        path=work/"preexit"/"ram.json"
        records=load(path)
        ring=bytes.fromhex(records[f"{L['ring']:x}/4096"])
        idx=struct.unpack_from("<I",ring,596)[0]
        address=L["buffers"]+idx*STRIDE+12+22
        for key,value in records.items():
            base,n=key.split("/")
            base=int(base,16)
            if base<=address<base+int(n):
                buf=bytearray.fromhex(value)
                buf[address-base]^=1
                records[key]=buf.hex()
                break
        reject("pending DMA bytes",path,json.dumps(records).encode(),"unreaped DMA")
        path=work/"machine.log"
        lines=path.read_bytes().splitlines(keepends=True)
        status=mmio(lines,L)
        lo,hi=status[5][0],status[6][0]
        drop=set()
        selected=None
        for i in range(lo+1,hi):
            m=re.search(rb"memory_region_ops_(read|write) cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size 4",lines[i])
            if m:
                op,r,v=m[1],int(m[2],16)-NIC,int(m[3],16)
                if (op,r)==(b"write",48):
                    selected=v
                if (op,r,v,selected)==(b"read",68,0,1):
                    drop.add(i)
        require(drop,"no second queue retirement readback")
        reject("missing TX QueueReady reset readbacks",path,b"".join(l for i,l in enumerate(lines) if i not in drop),
               "retirement status or queue-ready")
        path=work/"active"/"ram.json"
        records=load(path)
        addr,size=L["user"][1][P[3]]
        n=min(size,4096-addr%4096)
        leaf=RAM(records).leaf(L["roots"][1],addr)
        key=f"{(leaf&PA_MASK)+addr%4096:x}/{n}"
        require(n>=30,"first stored frame nonce crosses page")
        buf=bytearray.fromhex(records[key])
        buf[22:30]=nonces[:8]
        records[key]=buf.hex()
        reject("old nonce in replacement delivery",path,json.dumps(records).encode(),"replacement consumer nonce")
        path=work/"middle"/"gate.json"
        gate=load(path)
        gate["physical"]+=8
        reject("gate outside authorized word",path,json.dumps(gate).encode(),"unapproved gate write")
    return rejected


if __name__=="__main__":
    cli=argparse.ArgumentParser(description=__doc__)
    for key in ("package","driver-manifest","out"):
        cli.add_argument("--"+key,type=Path,required=True)
    cli.add_argument("--qemu",default="qemu-system-aarch64")
    cli.add_argument("--mac",type=str.lower)
    cli.add_argument("--mtu",type=int)
    cli.add_argument("--timeout",type=float,default=120)
    cli.add_argument("--replay",action="store_true")
    cli.add_argument("--mutation-tests",action="store_true")
    a=cli.parse_args()
    pkg,imp,out=[p.resolve() for p in (a.package,a.driver_manifest,a.out)]
    if not a.replay:
        mac=a.mac or "52:54:00:19:26:41"
        mtu=a.mtu if a.mtu is not None else 1410
        require(re.fullmatch(r"(?:[0-9a-f]{2}:){5}[0-9a-f]{2}",mac) and 100<=mtu<=65535,"NIC identity")
        capture(pkg,imp,out,a.qemu,mac,mtu,a.timeout)
    launch=load(out/"launch.json")
    require((a.mtu is None or a.mtu==launch["mtu"]) and (a.mac is None or a.mac==launch["mac"]),"CLI NIC differs")
    result=replay(pkg,imp,out)
    if a.mutation_tests:
        result["rejected_mutations"]=controls(pkg,imp,out)
    if not a.replay:
        save(out/"result.json",result)
    print(json.dumps(result,indent=2))
