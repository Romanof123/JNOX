#!/usr/bin/env python3
"""Capture and replay containment of one deliberate EL0 BRK in a live NIC owner."""
import argparse
import copy
import json
import re
import secrets
import shutil
import socket
import struct
import subprocess
import threading
import time
from pathlib import Path

from elftools.elf.elffile import ELFFile
import verify_linux_source_net_lifecycle as life
from native_el0_replay import PA_MASK, permissions, vread
from qemu_capture import capture_halt, journal_location, request
from verify_linux_source_net import RAM, NEG, NIC, callback_attempts, load, require, save, sha
from verify_linux_source_net_packets import MAX_LOG, STRIDE, area, pcap_frames, socket_frames

DRIVER = ("jnox_assert_driver", "jnox_assert_rx_slots")
PEER = ("jnox_assert_peer", "jnox_assert_peer_frame")
STAGES = ("armed", "final")
BRK = bytes.fromhex("008820d4")


def layout(pkg, imp):
    # This image terminates by BRK. Its administrative down wrapper has no
    # callers, while the original ndo_stop still lives in the checked table.
    adapters = {name: obj for name, obj in life.ADAPTER.items() if name != "jnox_lifecycle_net_down_v1"}
    L = life.layout(pkg, imp, journal_names=(DRIVER, PEER), adapters=adapters)
    require(load(imp)["lifecycle_assert_test"]["immediate"] == 0x440, "assertion-only fixture")
    driver = pkg/"user.elf"
    with driver.open("rb") as f:
        elf = ELFFile(f)
        s = life.symbol(elf, "jnox_lifecycle_assert_trap")
        require(life.data(elf, s)[:4] == BRK and s["st_size"] >= 4, "frozen EL0 BRK instruction")
        L["trap"] = s["st_value"]
        L["code"]["fixture_brk"] = dict(entry=L["trap"], size=4, prefix=BRK.hex(), peer=None)
    with (pkg/"native_peer.elf").open("rb") as f:
        for seg in ELFFile(f).iter_segments():
            if seg["p_type"]!="PT_LOAD" or not seg["p_flags"]&1:
                continue
            base=seg["p_vaddr"]
            for fn in L["code"].values():
                off=fn["entry"]-base
                if 0<=off<((seg["p_memsz"]+4095)//4096)*4096:
                    fn["peer"]=seg.data()[off:off+16].ljust(16,b"\0").hex()
    for group, sizes in zip(L["journals"], ((64,256),(96,91))):
        require(tuple(n for _,n in group.values()) == sizes, "fixture receipt size")
    kernel = pkg/"kernel.elf"
    L["assertions"] = journal_location(kernel,"jnox_linux_net_assertions")
    L["decision"] = journal_location(kernel,"jnox_linux_net_assert_exit")
    L["grants"] = journal_location(kernel,"jnox_linux_net_lifecycle_grants")
    require(L["assertions"][1] == 160 and L["decision"][1] == 96 and
            L["grants"][1] == 768, "protected journal ABI")
    return L


def snapshot(mem, L):
    roots = L["roots"]
    groups = []
    physical = [set(),set()]
    for slot,(syms,root) in enumerate(zip(L["journals"],roots)):
        values = {}
        for name,(va,n) in syms.items():
            for page in range(va>>12, ((va+n-1)>>12)+1):
                physical[slot].add(int(permissions(mem,root,page<<12,"user rw"),16)&PA_MASK)
            values[name] = vread(mem,root,va,n)
        groups.append(values)
    ranges = (L["assertions"],L["decision"],L["grants"],L["kernel_irq"],(L["ring"],8192),(L["buffers"],64*STRIDE))
    protected = {page<<12 for a,n in ranges for page in range(a>>12, ((a+n-1)>>12)+1)}
    for root in roots:
        for pa in protected:
            permissions(mem,root,pa,"kernel rw")
        permissions(mem,root,NIC&~4095,"kernel rw",device=True)
    raw_grants = area(mem,*L["grants"])
    grants = list(struct.iter_unpack("<QQQ",raw_grants))
    translations = []
    for token,va,n in grants:
        if not token:
            require((va,n)==(0,0),"empty grant has stale destination")
            translations.append(None)
            continue
        require(n==1530 and 0x400000<=va<va+n<=0x800000 and va%4096+n<=4096,"RX grant bounds")
        pa = (int(permissions(mem,roots[0],va,"user rw"),16)&PA_MASK) + va%4096
        translations.append(pa)
        physical[0].add(pa&~4095)
        mem.read(pa,n)
    for i,root in enumerate(roots):
        forbidden = protected | physical[1-i]
        tables = {root&PA_MASK}
        for depth in range(3):
            children = set()
            for table in tables:
                for (pte,) in struct.iter_unpack("<Q",mem.read(table,4096)):
                    if pte&1:
                        require(pte&3==3,"unexpected block mapping")
                        if depth<2:
                            children.add(pte&PA_MASK)
                        elif pte&64:
                            require(pte&PA_MASK not in forbidden,"EL0 DMA or cross-process alias")
            tables = children
    permissions(mem,roots[0],L["trap"],"user rx")
    require(vread(mem,roots[0],L["trap"],4)==BRK,"live executable differs from frozen BRK")
    exceptions=life.unpack(area(mem,*L["assertions"]))
    if exceptions[4]:
        permissions(mem,roots[0],exceptions[8]-1,"user rw")
        permissions(mem,roots[0],exceptions[9]-4,"user rx")
    return dict(driver=life.unpack(groups[0][DRIVER[0]]),slots=life.unpack(groups[0][DRIVER[1]]),
                peer=life.unpack(groups[1][PEER[0]]),frame=groups[1][PEER[1]],
                assertions=exceptions,decision=life.unpack(area(mem,*L["decision"])),grants=grants,physical=translations,
                ring=area(mem,L["ring"],8192),buffers=area(mem,L["buffers"],64*STRIDE),
                irq=life.unpack(area(mem,*L["kernel_irq"])))


def capture(pkg,imp,out,qemu,mac,mtu,timeout):
    L=layout(pkg,imp)
    binary=shutil.which(qemu)
    require(binary,"QEMU not found")
    out.mkdir(parents=True,exist_ok=False)
    (out/"net.events").write_text(life.TRACE)
    nonce=secrets.randbelow((1<<64)-1)+1
    (out/"nonce.bin").write_bytes(nonce.to_bytes(8,"big"))
    frame=life.rx.frames(bytes.fromhex(mac.replace(":","")),nonce.to_bytes(8,"big"))[0]
    deadline=time.monotonic()+timeout
    host,nic=socket.socketpair()
    with host,nic,(out/"stderr.log").open("xb") as err:
        fd=nic.fileno()
        argv=life.command(binary,out,pkg/"kernel.elf",fd,mac,mtu,L)
        with subprocess.Popen(argv,pass_fds=(fd,),stdin=subprocess.PIPE,stdout=subprocess.PIPE,
                              stderr=err,bufsize=0) as proc:
            nic.close()
            class Pipe:
                readline=proc.stdout.readline
                write=proc.stdin.write
            q=Pipe()
            timer=threading.Timer(timeout+20,proc.kill)
            timer.start()
            binary_hash=sha(Path(binary))
            require(sha(Path(f"/proc/{proc.pid}/exe"))==binary_hash,"launched QEMU executable identity")
            save(out/"launch.json",dict(argv=argv,fd=fd,out=str(out),kernel=str(pkg/"kernel.elf"),
                 qemu_sha256=binary_hash,kernel_sha256=L["kernel_sha256"],import_sha256=sha(imp),
                 driver_sha256=L["driver_sha256"],mac=mac,mtu=mtu))
            def read(pa,n):
                request(q,"pmemsave",val=pa,size=n,filename=str(out/"probe.bin"))
                return (out/"probe.bin").read_bytes()
            def field(slot,index):
                root=L["roots"][slot]
                base=next(iter(L["journals"][slot].values()))[0]
                mem=RAM({},read)
                return int.from_bytes(vread(mem,root,base+8*index,8),"little") if mem.leaf(root,base) else 0
            def wait(label,ready,terminal=False):
                while True:
                    log=out/"uart.log"
                    raw=log.read_bytes() if log.exists() else b""
                    exits=re.findall(rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$",raw)
                    if terminal:
                        require(exits==[(b"0",b"-5"),(b"1",b"0")][:len(exits)],"unexpected signed exit")
                    else:
                        require(not exits,"early native exit before "+label)
                    require(b"NATIVE EL0 FAIL" not in raw and b"NATIVE EL0 FAULT" not in raw,"guest failure before "+label)
                    if (not terminal or len(exits)==2) and ready():
                        return
                    require(time.monotonic()<deadline and proc.poll() is None,"deadline: "+label)
                    require(not (out/"machine.log").exists() or (out/"machine.log").stat().st_size<MAX_LOG,"trace limit")
                    time.sleep(.005)
            def freeze(stage):
                where=out/stage
                where.mkdir()
                capture_halt(q,where)
                mem=RAM({},read)
                try:
                    return snapshot(mem,L)
                finally:
                    save(where/"ram.json",mem.records)
            try:
                require("QMP" in json.loads(q.readline()),"QMP greeting")
                request(q,"qmp_capabilities")
                def ready_for_ingress():
                    k=life.unpack(read(*L["kernel_irq"]))
                    return k[22]==1 and k[19]!=0 and field(1,0)==k[19] and field(1,1)==int(mac.replace(":",""),16)
                wait("active IRQ waiter and peer session",ready_for_ingress)
                wire=len(frame).to_bytes(4,"big")+frame
                host.sendall(wire)
                (out/"host.bin").write_bytes(wire)
                wait("pretrap owner and peer receipt",lambda:field(0,7)==1 and field(1,4)==1)
                pre=freeze("armed")
                require(pre["driver"][7]==1 and not any(pre["assertions"]),"missed pre-BRK stop")
                request(q,"cont")
                wait("signed terminal exits and peer completion",lambda:field(1,11)==100,terminal=True)
                freeze("final")
                request(q,"quit")
                proc.wait(timeout=8)
            except Exception as exc:
                diagnostic={"error":repr(exc)}
                try:
                    request(q,"stop")
                    diagnostic["status"]=request(q,"query-status")
                    mem=RAM({},read)
                    try:
                        snapshot(mem,L)
                    finally:
                        save(out/"failure-ram.json",mem.records)
                except Exception as stop_error:
                    diagnostic["stop_error"]=repr(stop_error)
                save(out/"failure.json",diagnostic)
                raise
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
                timer.cancel()
                timer.join()


def check_shots(L,pre,post,wire):
    d=pre["driver"]
    token,nonce,n,posted,status,q0,q1,armed=d
    require(token and nonce==int.from_bytes(wire[22:30],"big") and n==len(wire)==60 and armed==1 and
            status==15 and all(v&0xffffffff==0x200001 and v>>32>=32 for v in (q0,q1)),"armed driver")
    require(pre["frame"]==wire+bytes(31) and post["frame"]==pre["frame"],"wire to survivor private RAM")
    require(pre["peer"][:5]==[token,int.from_bytes(wire[:6],"big"),nonce,n,1] and not any(pre["peer"][5:]),
            "peer before EOF")
    require(not any(pre["assertions"]) and not any(pre["decision"]),"exception or terminal decision before checkpoint")
    ring=pre["ring"]
    avail,used=struct.unpack_from("<H",ring,514)[0],struct.unpack_from("<H",ring,586)[0]
    require(used==1 and 0<posted==avail-used<=32,"active ring progress")
    entries=struct.unpack_from("<32H",ring,516)
    live={entries[i%32] for i in range(used,avail)}
    require(len(live)==posted and all(i<32 for i in live),"outstanding descriptor IDs")
    for i,(epoch,va,length) in enumerate(pre["grants"]):
        require((i in live)==bool(epoch) and va==pre["slots"][i],"protected grant vs fixture slot")
        if i in live:
            addr,size,flags,nxt=struct.unpack_from("<QIHH",ring,16*i)
            require(epoch==token and length==size==1530 and (flags,nxt)==(2,0) and
                    addr==L["buffers"]+i*STRIDE and pre["physical"][i] is not None,
                    "private grant to physical DMA descriptor")
    used_id,used_len=struct.unpack_from("<II",ring,588)
    require(used_id<32 and used_len==72 and pre["buffers"][used_id*STRIDE+12:used_id*STRIDE+72]==wire,
            "used descriptor and external Ethernet bytes")
    a=post["assertions"]
    require(a[0]>0 and a[1:6]==[0,L["roots"][0],0x400,0xf2000440,L["trap"]] and
            a[6]&31==0 and a[8]%16==0 and a[8]!=0 and a[9]!=0 and not any(a[10:]),"EL1 BRK vector/ESR/ELR/root")
    require(post["decision"]==[a[0],256,0,1,NEG-5,a[5]]+[0]*6,
            "protected post-retirement FIFO decision at EL1 handling tick")
    require(post["driver"]==d and post["slots"]==pre["slots"] and not any(any(g) for g in post["grants"]) and
            not any(post["buffers"]) and pre["irq"][26]==0 and post["irq"][26]==1 and post["irq"][22]==0,
            "kernel exit teardown and dead driver state")
    p=post["peer"]
    require(p[:5]==pre["peer"][:5] and p[5]==0 and p[6]>0 and p[7]>=p[6]+100000000 and
            p[8:]==[NEG-13]*3+[100],"peer EOF, actual timer and authorization")
    return dict(token=token,posted=posted,used_id=used_id,tick=a[0],el1_handler_tick=a[0],
                protected_fifo_decision=post["decision"][:6],elr=hex(a[5]),esr=hex(a[4]),
                driver_ttbr=hex(a[2]),peer_ttbr=hex(L["roots"][1]),peer_elapsed_ns=p[7]-p[6])


def trace_evidence(L, lines):
    positions={}
    for name in ("virtnet_validate","virtnet_probe","virtnet_open","skb_recv_done","virtnet_poll","gro_receive_skb","fixture_brk"):
        fn=L["code"][name]
        _,hits=callback_attempts(lines,fn["entry"])
        require(bool(hits) and (name=="virtnet_poll" or len(hits)==1),"missing or repeated entry: "+name)
        life.rx.verify_execution(lines,fn,len(hits))
        positions[name]=hits[0]
        if name=="virtnet_poll":
            polls=hits
    # Opening the original netdev schedules an empty poll. Attribute the
    # delivered packet to the poll after its completion callback and before GRO.
    delivered=[i for i in polls if positions["skb_recv_done"]<i<positions["gro_receive_skb"]]
    require(len(delivered)==1,"missing unique packet-associated NAPI poll")
    positions["virtnet_poll"]=delivered[0]
    require(list(positions.values())==sorted(positions.values()),"original callback / assertion order")
    reg=[]
    pattern=rb"memory_region_ops_(read|write) cpu 0 mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size (\d+)"
    for i,line in enumerate(lines):
        m=re.search(pattern,line)
        if m and NIC<=int(m[2],16)<NIC+512:
            reg.append((i,m[1],int(m[2],16)-NIC,int(m[3],16),int(m[4])))
    states=[(i,val) for i,op,r,val,w in reg if (op,r,w)==(b"write",112,4)]
    require([v for _,v in states]==[0,1,3,11,15,0],"single hardware claim and reset")
    reset=states[-1][0]
    require(states[-2][0]<positions["fixture_brk"]<reset,"BRK before protected reset")
    ack=False
    selected=None
    ready=[]
    for i,op,r,v,w in reg:
        if i<=reset or w!=4:
            continue
        if (op,r,v)==(b"read",112,0):
            ack=True
        elif (op,r)==(b"write",48):
            selected=v
        elif (op,r)==(b"read",68):
            require(ack and v==0,"QueueReady after acknowledged reset")
            ready.append(selected)
    require(ready==[0,1],"both selected QueueReady readbacks after owner exit")
    return positions


def check_uart(uart,post):
    exits=re.findall(rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$",uart)
    require(exits==[(b"0",b"-5"),(b"1",b"0")],
            "driver contained, surviving task finished")
    rows=[list(map(int,row.split())) for row in re.findall(rb"(?m)^NATIVE FIXED EVENT ([\d ]+)\r?$",uart)]
    a=post["assertions"]
    faults=[i for i,r in enumerate(rows) if r[1]==256]
    drops=re.findall(rb"(?m)^NATIVE FIXED DROPPED (\d+)\r?$",uart)
    require(len(drops)==1 and rows and len(faults)<=1 and
            not any(r[1]==3 and r[3]==0 for r in rows),"fixed journal header or cooperative owner exit")
    dropped=int(drops[0])
    require(faults or (dropped and rows[-1][0]<a[0]),"assertion event missing inside retained scheduler coverage")
    later=[r for r in rows if r[0]>a[0]]
    if faults:
        r=rows[faults[0]]
        require(r[0]==a[0] and r[3]==0 and r[5:7]==[a[5],a[8]],"fixed assertion boundary")
    require(not any(r[3]==0 or r[4]==0 for r in later),"owner ran again after terminal BRK")
    sleep=any(r[3]==1 and r[1]==2 for r in later)
    finished=any(r[3]==1 and r[1]==3 for r in later)
    require(dropped or (sleep and finished),"surviving task terminal schedule")
    return dict(rows_retained=len(rows),rows_dropped=dropped,reason_256_observed=bool(faults),
                post_fault_peer_sleep_observed=sleep,post_fault_peer_exit_observed=finished)


def replay(pkg,imp,out,mutations=False):
    L=layout(pkg,imp)
    launch=load(out/"launch.json")
    argv=launch["argv"]
    require(argv==life.command(argv[0],Path(launch["out"]),Path(launch["kernel"]),launch["fd"],
            launch["mac"],launch["mtu"],L) and (out/"net.events").read_text()==life.TRACE and
            all(launch[k]==L[k] for k in ("kernel_sha256","driver_sha256","import_sha256")),
            "capture launch and imported artifact identity")
    shots=[]
    for stage in STAGES:
        path=out/stage
        q,h=load(path/"qmp.json"),load(path/"halted.json")
        require(q["status"]["running"] and q["stop_return"]=={} and
                [cpu["cpu-index"] for cpu in q["cpus"]]==[0] and
                not h["running"] and h["status"]=="paused","stopped CPU0 capture")
        mem=RAM(load(path/"ram.json"))
        shots.append(snapshot(mem,L))
        require(mem.used==set(mem.records),"unused retained memory observation")
    nonce=(out/"nonce.bin").read_bytes()
    require(len(nonce)==8 and any(nonce),"external nonce")
    wire=life.rx.frames(bytes.fromhex(launch["mac"].replace(":","")),nonce)[0]
    ingress=pcap_frames(out/"wire.pcap")
    require(socket_frames((out/"host.bin").read_bytes())==[wire] and len(ingress)==1 and
            ingress[0] in (wire,bytes(10)+wire,bytes(12)+wire),"external socket/PCAP evidence")
    pre,post=shots
    result=check_shots(L,pre,post,wire)
    uart=(out/"uart.log").read_bytes()
    result["scheduler"]=check_uart(uart,post)
    log=out/"machine.log"
    require(log.stat().st_size<=MAX_LOG,"trace size")
    lines=log.read_bytes().splitlines()
    result["trace_entry_lines"]={n:i+1 for n,i in trace_evidence(L,lines).items()}
    if mutations:
        rejected=[]
        def reject(label,action):
            try:
                action()
            except (AssertionError,ValueError,RuntimeError):
                rejected.append(label)
            else:
                raise ValueError("accepted mutation: "+label)
        for label,index,delta in (("wrong ELR",5,4),("wrong TTBR ASID",2,1<<48),
                                  ("wrong syndrome",4,1),("peer falsely trapped",14,1)):
            bad=copy.deepcopy(post)
            bad["assertions"][index]^=delta
            reject(label,lambda:check_shots(L,pre,bad,wire))
        for label,index in (("terminal decision at wrong tick",0),("ordinary scheduler reason",1),
                            ("wrong surviving PID",3),("false peer decision",7)):
            bad=copy.deepcopy(post)
            bad["decision"][index]^=1
            reject(label,lambda:check_shots(L,pre,bad,wire))
        bad=copy.deepcopy(pre)
        slot=next(i for i,g in enumerate(bad["grants"]) if g[0])
        bad["grants"][slot]=(0,0,0)
        reject("removed live DMA authority",lambda:check_shots(L,bad,post,wire))
        bad=copy.deepcopy(post)
        bad["peer"][7]=bad["peer"][6]
        reject("no survivor timer progress",lambda:check_shots(L,pre,bad,wire))
        reject("cooperative exit",lambda:check_uart(uart+b"\nNATIVE FIXED EVENT 1 3 0 0 1 0 0\n",post))
        reset=result["trace_entry_lines"]["fixture_brk"]
        last_ready=next(i for i in reversed(range(reset,len(lines))) if
                        re.search(rb"memory_region_ops_read .*addr 0x0*a003e44 value 0x0 size 4",lines[i]))
        reject("missing second QueueReady readback",lambda:trace_evidence(L,lines[:last_ready]+lines[last_ready+1:]))
        result["rejected_mutations"]=rejected
    return result


if __name__=="__main__":
    cli=argparse.ArgumentParser(description=__doc__)
    for name in ("package","driver-manifest","out"):
        cli.add_argument("--"+name,type=Path,required=True)
    cli.add_argument("--replay",action="store_true")
    cli.add_argument("--mutation-tests",action="store_true")
    cli.add_argument("--timeout",type=float,default=120)
    cli.add_argument("--qemu",default="qemu-system-aarch64")
    cli.add_argument("--mac",default=None)
    cli.add_argument("--mtu",type=int,default=None)
    args=cli.parse_args()
    pkg,imp,out=(p.resolve() for p in (args.package,args.driver_manifest,args.out))
    if not args.replay:
        capture(pkg,imp,out,args.qemu,args.mac or "52:54:00:19:26:41",args.mtu or 1410,args.timeout)
    launch=load(out/"launch.json")
    require((args.mac is None or args.mac==launch["mac"]) and
            (args.mtu is None or args.mtu==launch["mtu"]),"requested NIC differs from capture")
    result=replay(pkg,imp,out,args.mutation_tests)
    if not args.replay:
        save(out/"result.json",result)
    print(json.dumps(result,indent=2))
