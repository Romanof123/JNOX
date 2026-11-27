#!/usr/bin/env python3
"""Verify native VirtIO SPI delivery, original receive callback dispatch and NAPI ingress."""
import argparse
import gzip
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

import verify_linux_source_net_rx_poll as poll_gate
from verify_linux_source_net_rx_poll import ring_state

TRACE += "gic_acknowledge_irq\ngic_cpu_read\ngic_cpu_write\n"
FUNCTIONS=(*poll_gate.FUNCTIONS,"virtnet_find_vqs","skb_recv_done")
KERNEL_METRICS="jnox_linux_net_irq_metrics"
IRQ_JOURNALS=("jnox_rx_irq_owner","jnox_rx_irq_phase","jnox_rx_irq_kernel","jnox_rx_irq_peer")
JOURNALS=("jnox_rx_poll_receipt","jnox_rx_napi_receipt","jnox_rx_skb_receipt")


def provenance(pkg,imp,live=False):
    m=load(imp)
    require(m["mode"]=="rx-irq-service" and m["callback_input"]=="virtio_net.packet.o" and
            m["source"]["sha256"]==m["upstream"]["sha256"]==rx.PIN and m["upstream"]["path"]=="drivers/net/virtio_net.c"
            and m["upstream"]["commit"]=="4700e3adf3b24b39dd4929e9c6d62811ba3ccea8","Linux intake/profile mismatch")
    if live:
        require(sha(Path(m["source"]["path"]))==rx.PIN,"original Linux TU mismatch")
    require(sha(imp.parent/"virtio_net.original.o")==m["objects"]["virtio_net.original.o"]["sha256"] and
            sha(imp.parent/"virtio_net.packet.o")==m["driver_object"]["sha256"] and
            sha(imp.parent/"virtio_net.callback.o")==m["callback_object"]["sha256"] and
            sha(pkg/"user.elf")==m["service"]["sha256"] and sha(imp.parent/"service.map")==m["link_map_sha256"],
            "driver object, native image or link map identity")
    reg=m["irq_registration"]
    require((reg["registrar"],reg["callback"],reg["source_object"],reg["queue_index"])==
            ("virtnet_find_vqs","skb_recv_done","virtio_net.callback.o",0) and
            reg["object_address_references"] and m["irq_rearm_test"]["test_only"] is race_profile(pkg),
            "original callback registration or directed fixture mismatch")
    link_map=(imp.parent/"service.map").read_text().splitlines()
    layouts={}
    with (pkg/"user.elf").open("rb") as f, (pkg/"native_peer.elf").open("rb") as p:
        elf,peer=ELFFile(f),ELFFile(p)
        require(elf["e_type"]=="ET_EXEC" and elf["e_machine"]=="EM_AARCH64" and elf.get_section_by_name(".note.jnox.abi"),
                "native driver ELF type")
        for name in FUNCTIONS:
            original=name in (*FUNCTIONS[:2],*FUNCTIONS[-2:])
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
                if name=="virtnet_find_vqs":
                    targets=[]
                    for sec in obj.iter_sections():
                        if not isinstance(sec,RelocationSection) or sec["sh_info"]!=osym["st_shndx"]:
                            continue
                        symbols=obj.get_section(sec["sh_link"])
                        for r in sec.iter_relocations():
                            target=symbols.get_symbol(r["r_info_sym"])
                            if isinstance(target["st_shndx"],int):
                                targets.append((obj.get_section(target["st_shndx"]).name,r["r_info_type"],r["r_addend"]))
                    require(any(t==".text.skb_recv_done" and kind in (275,277,311,312) and add==0
                                for t,kind,add in targets),"original callback address relocation")
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
    board_dir=Path(__file__).resolve().parents[1]/"docs/evidence/native-linux-net-irq-board"
    board=load(board_dir/"board.json")
    require(hashlib.sha256(gzip.decompress((board_dir/"virt-net.dtb.gz").read_bytes())).hexdigest()==
            board["sha256"]=="d029746308665dfa81ee4792518425c995b2356df990c46297e5c52e84dc2212" and
            board["interrupts"]==[0,47,1] and board["intid"]==79 and board["device_reg"]==[0,NIC,0,512],
            "virtio board DTB routing")
    return dict(board_dtb_sha256=board["sha256"],kernel_sha256=sha(pkg/"kernel.elf"),driver_sha256=sha(pkg/"user.elf"),
                manifest_sha256=sha(pkg/"manifest.json"),import_sha256=sha(imp),original_source=rx.PIN,
                routines=layouts,machine_layout=dict(filter=",".join(layouts[n]["filter"] for n in FUNCTIONS)))


def irq_words(mem,pkg,slot,symbol):
    kernel=pkg/"kernel.elf"
    roots=linked_roots(kernel,2)
    if slot is None:
        addr,size=journal_location(kernel,symbol)
        require(size==(256 if symbol==KERNEL_METRICS else 40) and 0x40000000<=addr<addr+size<=0x48000000,"kernel IRQ metrics extent")
        for root in roots:
            permissions(mem,root,addr,"kernel rw")
        raw=mem.read(addr,size)
    else:
        addr,size=journal_location(pkg/("user.elf","native_peer.elf")[slot],symbol)
        require(addr%8==0 and size%8==0 and 0x400000<=addr<addr+size<=0x800000,"IRQ journal extent")
        for page in range(addr//4096,(addr+size-1)//4096+1):
            desc=int(permissions(mem,roots[slot],page*4096,"user rw"),16)
            other=mem.leaf(roots[1-slot],page*4096)
            require(other is None or other&PA_MASK!=desc&PA_MASK,"IRQ journal shared with peer")
        raw=vread(mem,roots[slot],addr,size)
    return list(struct.unpack(f"<{size//8}Q",raw))


def snapshot(mem,pkg):
    result=poll_gate.snapshot(mem,pkg)
    for name in IRQ_JOURNALS:
        irq_words(mem,pkg,int(name=="jnox_rx_irq_peer"),name)
    irq_words(mem,pkg,None,KERNEL_METRICS)
    irq_words(mem,pkg,None,"jnox_linux_net_irq_scheduler")
    if race_profile(pkg):
        irq_words(mem,pkg,0,"jnox_irq_race_gate")
    return result


def delivery_epochs(lines):
    """Decode each actual GICC_IAR to GICC_EOIR interval and the NIC register transactions inside."""
    pending=False
    current=None
    deliveries=[]
    for i,line in enumerate(lines):
        if re.search(rb"\bgic_acknowledge_irq (?:v?cpu) 0 acknowledged irq 79$",line):
            require(not pending and current is None,"unpaired GIC ACK")
            pending=True
        m=re.search(rb"\bgic_cpu_read cpu 0 iface read at 0x0000000c: (0x[0-9a-f]+)$",line)
        if m and int(m[1],16)&1023==79:
            require(pending and current is None,"IAR without GIC acknowledgement")
            current={"token":int(m[1],16),"iar":i,"status":[],"ack":[]}
            pending=False
        if current is None:
            continue
        m=re.search(rb"memory_region_ops_(read|write) cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size (\d+)",line)
        if m and int(m[2],16) in (NIC+96,NIC+100):
            off=int(m[2],16)-NIC
            require(int(m[4])==4 and (off,m[1]) in ((96,b"read"),(100,b"write")),"incorrect IRQ status/ACK register access")
            current["status" if off==96 else "ack"].append(int(m[3],16))
        m=re.search(rb"\bgic_cpu_write cpu 0 iface write at 0x00000010 (0x[0-9a-f]+)$",line)
        if m:
            require(int(m[1],16)==current["token"],"GICC EOIR full token mismatch")
            status=current["status"]
            require(len(status)==1 and status[0] in range(4) and current["ack"]==([status[0]] if status[0] else []),
                    "unpaired device interrupt status/ACK inside IAR/EOIR")
            deliveries.append(dict(iar=current["iar"],eoi=i,token=current["token"],status=status[0]))
            current=None
    require(deliveries and not pending and current is None,"missing complete NIC SPI 79")
    return deliveries

def replay(pkg,imp,out):
    meta=provenance(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["identities"]==meta and launch["schema"]==1 and (out/"net.events").read_text()==TRACE and
            launch["argv"]==model_command(launch["argv"][0],Path(launch["out"]),Path(launch["kernel"]),launch["fd"],
                                         launch["mac"],launch["mtu"],meta["machine_layout"]),"poll profile launch identity")
    manifest=load(pkg/"manifest.json")
    irq=manifest["linux_source_net_rx_irq"]
    require((irq["intid"],irq["driver_slot"],irq["peer_slot"],irq["queue_index"],irq["callback"],irq["registrar"])==
            (79,0,1,0,"skb_recv_done","virtnet_find_vqs") and irq["syscalls"]["wait"]==174,"IRQ authority manifest")
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
    directed=race_profile(pkg)
    first=2 if directed else 0
    burst=load(out/"burst.json")
    require(burst["count"]==16 and burst["stream_bytes"]==sum(4+len(f) for f in data[:first+16]) and
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
            d[26:29]==[11,60,NEG-19] and d[29]==0 and d[30:36]==[NEG-95,NEG-95,0,NEG-90,NEG-14,NEG-16] and
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
    irq_results=irq_evidence(pkg,out,lines,meta,phases)
    # The IRQ driver delegates ACK to EL1. Raw hardware bits are independently decoded above.
    status_bits=0
    for epoch in irq_results["hardware_epochs"]:
        status_bits |= epoch["status"]
    transport_expected=d.copy()
    transport_expected[29]=status_bits
    notify,acked=poll_gate.check_mmio(lines,pool,transport_expected,napi[7])
    return dict(irq=irq_results,executed_entries=execs,registered_poll_va=hex(poll[0]),budget=poll[1],budget_exhaustions=poll[4],
                data_frames=80,terminal_ack_frames=1,typed_gro_normal_deliveries=81,queue_notifications=notify,
                irq_acknowledged_bits=acked,phases=phases)


def capture(pkg,imp,out,binary,mtu,mac,timeout):
    qemu=shutil.which(binary)
    require(qemu,"missing QEMU")
    meta=provenance(pkg,imp,True)
    root=linked_roots(pkg/"kernel.elf",2)[0]
    kernel_irq_addr,_=journal_location(pkg/"kernel.elf",KERNEL_METRICS)
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
                    if value==phase and (name!="active" or
                            int.from_bytes(read(kernel_irq_addr+22*8,8),"little")==1):
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
                    quiet_gate(stream,read,pkg,out,healthy)
                    stage("ready",3)
                    records=rx.frames(macbytes,nonce)
                    first=0
                    if race_profile(pkg):
                        first=2
                        request(stream,"cont")
                        send(records[0],sent)
                        race_stage(stream,read,pkg,out,healthy,1)
                        request(stream,"cont")
                        send(records[1],sent)
                        race_stage(stream,read,pkg,out,healthy,3)
                    before=request(stream,"query-status")
                    for frame in records[first:first+16]:
                        send(frame,sent)
                    after=request(stream,"query-status")
                    save(out/"burst.json",dict(count=16,before=before,after=after,stream_bytes=sent.tell()))
                    request(stream,"cont")
                    # The original poll refills at a threshold. Pace against actual peer consumption.
                    def consumed(n):
                        while int.from_bytes(vread(RAM({},read),peer_root,peer_va+6*8,8),"little")<n:
                            healthy()
                            time.sleep(.002)
                    consumed(first+16)
                    for i,frame in enumerate(records[first+16:],first+17):
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




def race_profile(pkg):
    tag=load(pkg/"manifest.json")["linux_source_net_rx_irq"]["rearm_test_only"]
    require(type(tag) is bool,"invalid IRQ test profile")
    return tag


def quiet_gate(stream,read,pkg,out,healthy):
    kernel,_=journal_location(pkg/"kernel.elf",KERNEL_METRICS)
    peer,_=journal_location(pkg/"native_peer.elf","jnox_rx_irq_peer")
    root=linked_roots(pkg/"kernel.elf",2)[1]
    while True:
        healthy()
        if int.from_bytes(read(kernel+22*8,8),"little")==1:
            tick=int.from_bytes(vread(RAM({},read),root,peer+8*8,8),"little")
            if tick==3:
                break
        time.sleep(.002)


def race_stage(stream,read,pkg,out,healthy,phase):
    symbol="jnox_irq_race_gate"
    addr,size=journal_location(pkg/"user.elf",symbol)
    require(size==128,"race gate layout")
    root=linked_roots(pkg/"kernel.elf",2)[0]
    while True:
        healthy()
        got=int.from_bytes(vread(RAM({},read),root,addr+8,8),"little")
        require(got!=255,"directed rearm guest reported failure")
        if got==phase:
            break
        time.sleep(.001)
    where=out/f"race-{phase}"
    where.mkdir()
    capture_halt(stream,where)
    mem=RAM({},read)
    words=irq_words(mem,pkg,0,symbol)
    require(words[1]==phase,"directed phase changed during halt")
    irq_words(mem,pkg,None,KERNEL_METRICS)
    ring,_=dma_layout(pkg)
    mem.read(ring,4096)
    save(where/"ram.json",mem.records)


def irq_evidence(pkg,out,lines,meta,phases):
    epochs=delivery_epochs(lines)
    directed=race_profile(pkg)
    samples={}
    for stage,_ in PHASES:
        ram=RAM(load(out/stage/"ram.json"))
        k=irq_words(ram,pkg,None,KERNEL_METRICS)
        sched=irq_words(ram,pkg,None,"jnox_linux_net_irq_scheduler")
        o,flat,ks,p=[irq_words(ram,pkg,int(n==IRQ_JOURNALS[-1]),n) for n in IRQ_JOURNALS]
        require([len(o),len(flat),len(ks),len(p)]==[32,32,81,16],"IRQ journal dimensions")
        phases_irq=[flat[i:i+8] for i in range(0,32,8)]
        kernel_phase=[ks[i:i+27] for i in range(0,81,27)]
        token=phases[stage]["token"]
        require(o[:3]==[0x4a4e4952514f3031,token,{"ready":1,"active":2,"final":3}[stage]] and
                p[:4]==[0x4a4e495251503031,token,NEG-13,NEG-13] and o[7:11]==[NEG-22]*4 and k[19:21]==[token,63] and
                p[12]==(NEG-13 if directed else 0),"event ownership, epoch or peer authority")
        require(sched==[k[i] for i in (6,7,25,9,10)],"scheduler block/wake and device counts differ")
        require(p[8]==3 and p[4]>0 and all(p[4+i]>=p[4]+i*10_000_000 and p[4+i]>p[3+i] for i in (1,2,3)),
                "quiet peer absolute timer progression")
        if stage!="ready":
            require(p[9]>=p[7] and p[10]==p[9]+10_000_000<=p[11],"active peer timer")
        a=phases_irq[{"ready":0,"active":1,"final":2}[stage]]
        u=kernel_phase[{"ready":0,"active":1,"final":2}[stage]]
        require(a[:3]==[meta["routines"]["skb_recv_done"]["entry"],0,int.from_bytes(b"input.0","little")] and
                u[19:21]==[token,63],"real queue registration or saved epoch")
        if stage=="ready":
            require(k[0]==k[5]==k[14]==int(directed) and k[1:5]==[0]*4 and k[6:8]==[1,0] and
                    k[16]>=3 and k[21:23]==[0,1] and not any(a[3:]) and not any(p[9:12]),
                    "quiet blocked IRQ owner or spurious RX dispatch")
        else:
            require(a[3]>0 and a[3]==u[7]+u[9] and a[5]<=u[21] and a[6]==a[3]+o[3] and
                    k[7]>0 and k[21]==k[2] and k[22]==int(stage=="active"),
                    "actual RX events, blocked wake, or callback accounting")
        samples[stage]=dict(kernel=k,scheduler=sched,adapter=a,owner=o,peer=p)
    ready,active,final=[samples[s] for s in ("ready","active","final")]
    k=final["kernel"]
    require(k[0]==k[14]==len(epochs) and k[11]==k[15]==epochs[-1]["token"] and
            k[1]==sum(bool(e["status"]&1) for e in epochs) and k[3]==sum(bool(e["status"]&2) for e in epochs) and
            k[5]==sum(not e["status"] for e in epochs)==int(directed) and k[6]==k[7] and
            k[21]>active["kernel"][21]>0 and k[7]>active["kernel"][7] and k[17]<k[18]==81 and k[26]==1,
            "IRQ status, EOIR, blocked ACK epoch or teardown counters")
    require(o[5:7]==[NEG-116]*2 and phases_irq[2]==phases_irq[3] and
            all(a[i]>=active["adapter"][i] for i in (3,4,5,6,7)) and a[5]>active["adapter"][5],
            "terminal ACK sequence or stale epoch authority")
    callback=meta["routines"]["skb_recv_done"]
    rx.verify_execution(lines,meta["routines"]["virtnet_find_vqs"],1)
    rx.verify_execution(lines,callback,a[6])
    _,callbacks=callback_attempts(lines,callback["entry"])
    _,polls=callback_attempts(lines,meta["routines"]["virtnet_poll"]["entry"])
    genuine=[e for e in epochs if e["status"]&1]
    require(genuine[0]["eoi"]<callbacks[0]<polls[0] and genuine[-1]["eoi"]<callbacks[-1]<polls[-1] and
            all(any(c<j<end for j in polls) for c,end in zip(callbacks,callbacks[1:]+[len(lines)])),
            "original callback and NAPI dispatch order")
    race=None
    if directed:
        require(epochs[0]["status"]==0 and o[11]==1 and o[12]==0,"directed zero-status GIC source")
        for n,used in ((1,1),(3,2)):
            where=out/f"race-{n}"
            halted=load(where/"halted.json")
            require(halted["status"]=="paused" and not halted["running"],"directed checkpoint is running")
            mem=RAM(load(where/"ram.json"))
            gate=irq_words(mem,pkg,0,"jnox_irq_race_gate")
            hw=irq_words(mem,pkg,None,KERNEL_METRICS)
            ring,_=dma_layout(pkg)
            raw=mem.read(ring,4096)
            require(mem.used==set(mem.records) and struct.unpack_from("<H",raw,586)[0]==used and hw[18]==used and
                    gate[:5]==[0x4952515241434531,n,k[19],1,1] and gate[11]==1,"directed ring and original snapshot")
        require(gate[5]<gate[7]<=gate[10] and gate[6]==2 and gate[8:10]==[1,1] and gate[12:15]==[2,k[19],0]
                and gate[15]>0 and phases["final"]["napi"][6]>0,"positive post-rearm pending check")
        race=gate
    return dict(hardware_epochs=epochs,observations=samples,rearm_race=race,
                registered_callback=hex(callback["entry"]),completed_callbacks=len(callbacks),
                irq_sequence=k[21],catchups=k[10],blocked_rx_wakes=k[7],idle_rx_wakes=k[25],
                software_pended_zero_status_irqs=int(directed))

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
        for name in ("virtnet_poll","gro_receive_skb","skb_recv_done"):
            _,entries=callback_attempts(lines,layouts[name]["entry"])
            i=entries[-1]
            reject("removed executed "+name,path,b"".join(lines[:i]+lines[i+1:]),"entry count")
        path=work/"ready"/"ram.json"
        journal=load(path)
        addr,_=journal_location(pkg/"kernel.elf","jnox_linux_net_irq_scheduler")
        key=f"{addr:x}/40"
        raw=bytearray.fromhex(journal[key])
        struct.pack_into("<Q",raw,0,0)
        reject("missing committed scheduler block",path,json.dumps(dict(journal,**{key:raw.hex()})).encode(),
               "scheduler block/wake")
        path=work/"machine.log"
        e=delivery_epochs(lines)[0]
        i=e["eoi"]
        wrong=lines[i].replace(b"0x0000004f",b"0x0000044f")
        require(wrong!=lines[i],"could not mutate full EOI")
        reject("wrong full EOIR token",path,b"".join(lines[:i]+[wrong]+lines[i+1:]),"EOIR full token")
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
