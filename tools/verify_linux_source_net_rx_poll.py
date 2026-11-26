#!/usr/bin/env python3
"""Observe native Linux virtnet NAPI dispatch and its typed skb ingress boundary."""
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
import verify_linux_source_net_rx as rx
from verify_linux_source_net import NEG, NIC, RAM, callback_attempts, load, require, save, sha
from verify_linux_source_net_packets import (COUNT, MAX_LOG, PHASES, STRIDE, TRACE, challenge, dma_layout,
                                            model_command, pcap_frames, socket_frames)
from verify_linux_source_net_queues import routine
from native_amc_roots import linked_roots
from native_el0_replay import PA_MASK, permissions, vread
from qemu_capture import capture_halt, journal_location, request

FUNCTIONS=("virtnet_alloc_queues","virtnet_poll","gro_receive_skb","jnox_rx_disable_cb_v1","virtqueue_enable_cb_prepare")
JOURNALS=("jnox_rx_poll_receipt","jnox_rx_napi_receipt","jnox_rx_skb_receipt")


def provenance(pkg,imp,live=False):
    m=load(imp)
    require(m["mode"]=="rx-poll-service" and m["callback_input"]=="virtio_net.packet.o" and
            m["source"]["sha256"]==m["upstream"]["sha256"]==rx.PIN and m["upstream"]["path"]=="drivers/net/virtio_net.c"
            and m["upstream"]["commit"]=="4700e3adf3b24b39dd4929e9c6d62811ba3ccea8","Linux intake/profile mismatch")
    if live:
        require(sha(Path(m["source"]["path"]))==rx.PIN,"original Linux TU mismatch")
    require(sha(imp.parent/"virtio_net.original.o")==m["objects"]["virtio_net.original.o"]["sha256"] and
            sha(imp.parent/"virtio_net.packet.o")==m["driver_object"]["sha256"] and
            sha(imp.parent/"virtio_net.callback.o")==m["callback_object"]["sha256"] and
            sha(pkg/"user.elf")==m["service"]["sha256"] and sha(imp.parent/"service.map")==m["link_map_sha256"],
            "driver object, native image or link map identity")
    link_map=(imp.parent/"service.map").read_text().splitlines()
    layouts={}
    with (pkg/"user.elf").open("rb") as f, (pkg/"native_peer.elf").open("rb") as p:
        elf,peer=ELFFile(f),ELFFile(p)
        require(elf["e_type"]=="ET_EXEC" and elf["e_machine"]=="EM_AARCH64" and elf.get_section_by_name(".note.jnox.abi"),
                "native driver ELF type")
        for name in FUNCTIONS:
            original=name in FUNCTIONS[:2]
            supplier="virtio_net.callback.o" if original else ("rx_skb.o" if name=="gro_receive_skb" else "rx_virtqueue.o")
            require(sum(l.strip().endswith(f"{supplier}:(.text.{name})") for l in link_map)==1,
                    "retained ELF link-map supplier: "+name)
            sym,body=routine(elf,name)
            source=imp.parent/("virtio_net.packet.o" if original else supplier)
            with source.open("rb") as objfile:
                obj=ELFFile(objfile)
                osym,code=routine(obj,name)
                reloc={r["r_offset"]//4 for sec in obj.iter_sections() if isinstance(sec,RelocationSection) and
                       sec["sh_info"]==osym["st_shndx"] for r in sec.iter_relocations()}
            if original:
                with (imp.parent/supplier).open("rb") as promoted:
                    psym,pcode=routine(ELFFile(promoted),name)
                    require(osym["st_info"]["bind"]=="STB_LOCAL" and psym["st_info"]["bind"]=="STB_GLOBAL" and
                            pcode==code,"original promotion changed "+name)
            require(len(code)==len(body) and all(code[i:i+4]==body[i:i+4] for i in range(0,len(code),4) if i//4 not in reloc),
                    "linked non-relocated instructions differ: "+name)
            va=sym["st_value"]
            meta=dict(entry=va,size=len(body),prefix=body[:16].hex(),peer=None,filter=f"0x{va:x}+0x{len(body):x}",
                      section_sha256=hashlib.sha256(code).hexdigest())
            for seg in peer.iter_segments():
                start=seg["p_vaddr"]
                if (seg["p_type"]=="PT_LOAD" and seg["p_flags"]&1 and
                    start&~4095 <= va < (start+seg["p_memsz"]+4095)&~4095):
                    require(va>=start,"ambiguous peer instruction prefix")
                    off=va-start
                    meta["peer"]=seg.data()[off:off+16].ljust(16,b"\0").hex()
            layouts[name]=meta
    return dict(kernel_sha256=sha(pkg/"kernel.elf"),driver_sha256=sha(pkg/"user.elf"),
                manifest_sha256=sha(pkg/"manifest.json"),import_sha256=sha(imp),original_source=rx.PIN,
                routines=layouts,machine_layout=dict(filter=",".join(layouts[n]["filter"] for n in FUNCTIONS)))


def snapshot(mem,pkg):
    (d,p),rings,buffers,journal=rx.snapshot(mem,pkg)
    root,peer=linked_roots(pkg/"kernel.elf",2)
    for name,want_size in (("jnox_rx_napi_fault",4),("jnox_rx_poll_fault_id",8)):
        va,size=journal_location(pkg/"user.elf",name)
        require(size==want_size and vread(mem,root,va,size)==bytes(size),
                "native NAPI or unadmitted Linux import fault: "+name)
    for name in JOURNALS:
        va,size=journal_location(pkg/"user.elf",name)
        require(size==64 and va%8==0 and 0x400000<=va<va+size<=0x800000,"poll receipt extent")
        for n in range(va//4096,(va+size-1)//4096+1):
            desc=int(permissions(mem,root,n*4096,"user rw"),16)
            alias=mem.leaf(peer,n*4096)
            require(alias is None or (alias&PA_MASK)!=(desc&PA_MASK),"poll receipt shared with peer")
        d.extend(struct.unpack("<8Q",vread(mem,root,va,size)))
    return (d,p),rings,buffers,journal


def ring_state(stage,rings,buffers,base,frames,received):
    rx_ring,tx=rings
    u16=lambda b,n:struct.unpack_from("<H",b,n)[0]
    posted=u16(rx_ring,514)
    held=posted-received
    # virtnet_receive refills when num_free > min(budget, ring_size)/2.
    require(u16(rx_ring,586)==received and 28<=held<=32 and posted>=32 and
            (stage!="ready" or posted==32) and u16(rx_ring,512) in (0,1),"RX refill threshold, used count or flag")
    require(not any(tx[:512]) and (u16(tx,514),u16(tx,586))==(0,0),"TX queue active on RX-only profile")
    available=[u16(rx_ring,516+2*(i%32)) for i in range(received,posted)]
    require(len(set(available))==held and all(i<32 for i in available),"duplicate or invalid live RX descriptor")
    for i in range(32):
        desc=struct.unpack_from("<QIHH",rx_ring,16*i)
        require(desc==(base+i*STRIDE,1530,2,0),"RX private grant extent")
    last={}
    for i in range(max(0,received-32),received):
        idx,n=struct.unpack_from("<II",rx_ring,588+8*(i%32))
        require(idx<32 and n==12+len(frames[i]),"RX used ID or length")
        if i>=posted-32:
            require(idx==u16(rx_ring,516+2*(i%32)),"retained used and available IDs differ")
        last[idx]=i
    if stage=="active":
        for idx,seq in last.items():
            pos=idx*STRIDE+12
            require(buffers[pos:pos+len(frames[seq])]==frames[seq],"active DMA payload mismatch")
    else:
        require(not any(buffers),"cold or retired DMA buffers not clear")
    return posted,held,u16(rx_ring,512)

def check_mmio(lines,pool,driver,kicks):
    statuses=[]
    ready=[[],[]]
    queues=[[],[]]
    maxes=[set(),set()]
    selected=None
    notified=last_cause=acked=0
    status=None
    for line in lines:
        if b"memory_region_ops_" not in line:
            continue
        m=re.search(rb"memory_region_ops_(read|write) cpu (-?\d+) mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size (\d+)",line,re.I)
        require(m,"malformed memory trace")
        reg=int(m[3],16)-NIC
        if not 0<=reg<512:
            continue
        require(m[2]==b"0","NIC accessed outside CPU0")
        write=m[1]==b"write"
        value=int(m[4],16)
        if reg==112:
            if write:
                status=value
                statuses.append(value)
            else:
                require(value==status,"inconsistent device status")
        if reg==48 and write:
            require(value in (0,1),"queue selector")
            selected=value
        if reg in (52,56,68,128,132,144,148,160,164):
            require(selected is not None and int(m[5])==4,"queue register extent")
            if write:
                queues[selected].append((reg,value))
            elif reg==68:
                ready[selected].append(value)
            elif reg==52:
                maxes[selected].add(value)
        if reg==80 and write:
            require(value==0 and status==15,"notify outside active RX")
            notified+=1
        if reg==96 and not write:
            last_cause=value&3
        if reg==100 and write:
            require(status==15 and 0<value==last_cause,"acknowledged IRQ differs")
            acked|=value
            last_cause=0
    for i in (0,1):
        expected=[(56,32)]
        for reg,off in ((128,0),(144,512),(160,584)):
            addr=pool+4096*i+off
            expected.extend([(reg,addr&0xffffffff),(reg+4,addr>>32)])
        require(queues[i]==expected+[(68,1)] and ready[i][0]==ready[i][-1]==0 and 1 in ready[i] and
                maxes[i]=={driver[24+i]>>32} and driver[24+i]&0xffffffff==(32<<16)|1,
                "transport queue programming, hardware size, or reset")
    require(statuses==[0,1,3,11,15,0] and notified==1+kicks and kicks>0 and
            driver[29]==acked and acked&1,"IRQ ACK or explicit live kick count")
    return notified,acked


def replay(pkg,imp,out):
    meta=provenance(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["identities"]==meta and launch["schema"]==1 and (out/"net.events").read_text()==TRACE and
            launch["argv"]==model_command(launch["argv"][0],Path(launch["out"]),Path(launch["kernel"]),launch["fd"],
                                         launch["mac"],launch["mtu"],meta["machine_layout"]),"poll profile launch identity")
    manifest=load(pkg/"manifest.json")
    require(manifest["linux_source_net_rx_poll"]==dict(driver_slot=0,syscalls={"rx_callbacks":173},
            modes={"disable":0,"enable_prepare":1,"poll":2,"refill_kick":3},
            initial_kick="driver_ok",live_refill_kick="explicit"),"poll-specific authority manifest")
    nonce=(out/"nonce.bin").read_bytes()
    mac=bytes.fromhex(launch["mac"].replace(":",""))
    require(len(nonce)==8 and any(nonce),"invalid external nonce")
    data=rx.frames(mac,nonce)
    ack=challenge(mac,nonce,True)
    frames=[*data,ack]
    require(socket_frames((out/"host-to-guest.bin").read_bytes())==frames and
            not (out/"guest-to-host.bin").read_bytes(),"socket bytes or direction")
    burst=load(out/"burst.json")
    require(burst["count"]==16 and burst["stream_bytes"]==sum(4+len(f) for f in data[:16]) and
            all(burst[k]["status"]=="paused" and burst[k]["running"] is False for k in ("before","after")),
            "initial stopped-CPU burst")
    wire=pcap_frames(out/"wire.pcap")
    require(len(wire)==len(frames) and all(a in (b,bytes(10)+b,bytes(12)+b) for a,b in zip(wire,frames)),
            "Ethernet PCAP differs")
    pool,buf=dma_layout(pkg)
    expected_peer=b"".join(s.ljust(91,b"\0") for s in data)
    phases={}
    for stage,want in PHASES:
        where=out/stage
        qmp,halt=load(where/"qmp.json"),load(where/"halted.json")
        require(qmp["status"]["running"] and [c["cpu-index"] for c in qmp["cpus"]]==[0] and
                qmp["stop_return"]=={} and halt["status"]=="paused" and not halt["running"],"QMP halt")
        ram=RAM(load(where/"ram.json"))
        (d,p),rings,buffers,(peer,peer_ack)=snapshot(ram,pkg)
        require(ram.used==set(ram.records) and d[:3]==[0x4a4e525844525631,want,0] and
                p[:6]==[0x4a4e525850454531,want,0,NEG-13,NEG-13,NEG-13] and p[12]==NEG-13,
                "phase publication or peer authority")
        received={"ready":0,"active":80,"final":81}[stage]
        require(peer==(expected_peer if received else bytes(len(expected_peer))) and
                peer_ack==(ack if stage=="final" else bytes(60)),"private peer Ethernet journal")
        if received:
            require(d[10:14]==[80,sum(map(len,data)),80,79] and p[6:11]==[80,sum(map(len,data)),
                    int.from_bytes(nonce,"big"),79,int.from_bytes(mac,"big")],"guest and peer Ethernet totals")
        posted,held,flags=ring_state(stage,rings,buffers,buf,frames,received)
        poll,napi,skb=d[36:44],d[44:52],d[52:60]
        require(poll[0:2]==[meta["routines"]["virtnet_poll"]["entry"],8],"registered NAPI pointer or budget")
        require(napi[:2]==[1,1] and poll[2]==napi[2] and poll[3]==received==poll[5] and poll[4]==napi[3] and
                0<=poll[4]<=poll[7]<=poll[2] and poll[6]<=8,"poll dispatch, budget or work totals")
        require(skb==[received]*6+[0,0],"skb build, typed GRO handoff, dequeue, release or drop")
        if stage=="ready":
            require(not any(poll[2:]) and not any(napi[2:]),"poll ran before injected frames")
        else:
            require(poll[4]>=1 and napi[4]>0 and napi[5]>=napi[4] and napi[6]<=napi[5] and napi[7]>0,
                    "no budget exhaustion, completion, rearm or live kick")
        phases[stage]=dict(posts=posted,device_owned=held,available_flags=flags,poll=poll,napi=napi,skb=skb,token=d[3])
    require(all(row["token"]==d[3]>0 for row in phases.values()) and d[4:10]==[(1<<32)|40,int.from_bytes(mac,"big"),
            0,0,15,int.from_bytes(nonce,"big")] and d[14:22]==[0xa6,0,NEG-116,NEG-116,0,launch["mtu"],1,0] and
            d[26:29]==[11,60,NEG-19] and d[30:36]==[NEG-95,NEG-95,NEG-11,NEG-90,NEG-14,NEG-16] and
            p[11]==60,"retirement, stale handle, or final delivery receipts")
    uart=(out/"uart.log").read_bytes().splitlines()
    for slot,prefix in zip(manifest["slots"],(b"NATIVE EL0 ELF ",b"NATIVE EL0 PEER ELF ")):
        require(prefix+slot["elf_sha256"].encode() in uart,"booted guest ELF")
    require({l for l in uart if l.startswith(b"NATIVE EL0 EXIT PID ")}=={b"NATIVE EL0 EXIT PID 0 0",b"NATIVE EL0 EXIT PID 1 0"}
            and not any(b"NATIVE EL0 FAULT" in l for l in uart),"native exits")
    log=out/"machine.log"
    require(log.stat().st_size<=MAX_LOG,"QEMU trace bound")
    lines=log.read_bytes().splitlines()
    execs={}
    for name,expected in zip(FUNCTIONS,(1,poll[2],81,None,napi[5])):
        layout=meta["routines"][name]
        if expected is None:
            _,dispatch=callback_attempts(lines,layout["entry"])
            expected=len(dispatch)
            require(expected>=poll[2],"missing callback suppression")
        execs[name]=rx.verify_execution(lines,layout,expected)
    notify,acked=check_mmio(lines,pool,d,napi[7])
    return dict(executed_entries=execs,registered_poll_va=hex(poll[0]),budget=poll[1],budget_exhaustions=poll[4],
                data_frames=80,terminal_ack_frames=1,typed_gro_normal_deliveries=81,queue_notifications=notify,
                polled_interrupt_bits=acked,phases=phases)


def capture(pkg,imp,out,binary,mtu,mac,timeout):
    qemu=shutil.which(binary)
    require(qemu,"missing QEMU")
    meta=provenance(pkg,imp,True)
    root=linked_roots(pkg/"kernel.elf",2)[0]
    va,_=journal_location(pkg/"user.elf",rx.PROBES[0][0])
    peer_root=linked_roots(pkg/"kernel.elf",2)[1]
    peer_va,_=journal_location(pkg/"native_peer.elf",rx.PROBES[1][0])
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
                if name=="active":
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
                    records=rx.frames(macbytes,nonce)
                    before=request(stream,"query-status")
                    for frame in records[:16]:
                        send(frame,sent)
                    after=request(stream,"query-status")
                    save(out/"burst.json",dict(count=16,before=before,after=after,stream_bytes=sent.tell()))
                    request(stream,"cont")
                    # The original poll refills at a threshold. Pace against actual peer consumption.
                    def consumed(n):
                        while int.from_bytes(vread(RAM({},read),peer_root,peer_va+6*8,8),"little")<n:
                            healthy()
                            time.sleep(.002)
                    consumed(16)
                    for i,frame in enumerate(records[16:],17):
                        healthy()
                        send(frame,sent)
                        if i%8==0:
                            consumed(i)
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
                diagnostic=dict(error=repr(exc),returncode=proc.poll())
                if proc.poll() is None:
                    try:
                        request(stream,"stop")
                        for role,elf,symbols in (
                            (root,"user.elf",(rx.PROBES[0][0],"jnox_rx_napi_fault","jnox_rx_poll_fault_id",*JOURNALS)),
                            (peer_root,"native_peer.elf",(rx.PROBES[1][0],))):
                            for symbol in symbols:
                                location,size=journal_location(pkg/elf,symbol)
                                data=vread(RAM({},read),role,location,size)
                                width=4 if size==4 else 8
                                diagnostic[symbol]=[int.from_bytes(data[i:i+width],"little") for i in range(0,size,width)]
                    except Exception as error:
                        diagnostic["snapshot_error"]=repr(error)
                save(out/"failure.json",diagnostic)
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
        path=work/"burst.json"
        reject("running CPU at burst",path,json.dumps(dict(load(path),count=15)).encode(),"stopped-CPU burst")
        for filename,reason in (("wire.pcap","Ethernet PCAP"),("host-to-guest.bin","socket bytes")):
            path=work/filename
            blob=bytearray(path.read_bytes())
            blob[-1]^=1
            reject(filename,path,blob,reason)
        path=work/"active"/"ram.json"
        for words,reason in ((((0,0),),"registered NAPI pointer"),(((0,4),(1,3)),"no budget exhaustion"),
                             (((2,3),),"skb build")):
            records=load(path)
            root=linked_roots(pkg/"kernel.elf",2)[0]
            for journal,index in words:
                addr,size=journal_location(pkg/"user.elf",JOURNALS[journal])
                offset=index*8
                while size:
                    take=min(size,4096-addr%4096)
                    if offset<take:
                        leaf=RAM(records).leaf(root,addr)
                        key=f"{(leaf&PA_MASK)+addr%4096:x}/{take}"
                        buf=bytearray.fromhex(records[key])
                        struct.pack_into("<Q",buf,offset,0)
                        records[key]=buf.hex()
                        break
                    size-=take
                    addr+=take
                    offset-=take
            reject(reason,path,json.dumps(records).encode(),reason)
        path=work/"machine.log"
        lines=path.read_bytes().splitlines(keepends=True)
        layouts=load(work/"launch.json")["identities"]["routines"]
        for name in ("virtnet_poll","gro_receive_skb"):
            _,entries=callback_attempts(lines,layouts[name]["entry"])
            i=entries[-1]
            reject("removed executed "+name,path,b"".join(lines[:i]+lines[i+1:]),"entry count")
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
