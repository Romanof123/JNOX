#!/usr/bin/env python3
"""Verify the native Linux-source queue provision profile from QEMU and stopped RAM."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
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
from verify_linux_source_net import (EXEC, INSN, MMIO, NEG, NIC, PIN, RAM, TRACE, argv,
                                     callback_attempts, load, require, save, sha)

PROBES = (("jnox_queue_driver_result",32), ("jnox_queue_peer_result",16))
SYMBOLS = ("virtnet_validate","jnox_queue_find_vqs")
FORBIDDEN = ("virtnet_probe","virtnet_open","start_xmit","virtnet_poll")


def routine(elf, name):
    table = elf.get_section_by_name(".symtab")
    hits = table.get_symbol_by_name(name) or []
    require(len(hits) == 1 and hits[0]["st_info"]["type"] == "STT_FUNC", "missing ELF routine "+name)
    sym = hits[0]
    sec = elf.get_section(sym["st_shndx"])
    off = sym["st_value"] - sec["sh_addr"]
    body = sec.data()[off:off+sym["st_size"]]
    require(body and len(body) == sym["st_size"], "empty or truncated function")
    return sym, body


def identities(pkg, imp, live=False):
    m = load(imp)
    require(m["mode"] == "queue-service" and m["upstream"]["path"] == "drivers/net/virtio_net.c" and
            m["upstream"]["sha256"] == m["source"]["sha256"] == PIN and
            m["upstream"]["commit"] == "4700e3adf3b24b39dd4929e9c6d62811ba3ccea8",
            "queue import identity")
    if live:
        require(sha(Path(m["source"]["path"])) == PIN, "original translation unit bytes")
    for filename, report in m["objects"].items():
        require(Path(filename).name == filename and sha(imp.parent/filename) == report["sha256"],
                "whole object identity")
    objects = [(imp.parent/m[field]["filename"],m[field]["sha256"]) for field in ("driver_object","callback_object")]
    objects.append((pkg/"user.elf",m["service"]["sha256"]))
    require(m["callback_input"] == objects[0][0].name and sha(imp.parent/"service.map") == m["link_map_sha256"],
            "queue variant or link map identity")
    code, reloc, layouts = [], set(), {}
    for n,(path, digest) in enumerate(objects):
        require(sha(path) == digest, "import artifact hash mismatch")
        with path.open("rb") as f:
            elf = ELFFile(f)
            require(elf["e_machine"] == "EM_AARCH64" and elf.little_endian and elf.elfclass == 64,
                    "ELF architecture")
            sym, body = routine(elf,SYMBOLS[0])
            code.append(body)
            if n == 0:
                for sec in elf.iter_sections():
                    if isinstance(sec,RelocationSection) and sec["sh_info"] == sym["st_shndx"]:
                        reloc.update(r["r_offset"]//4 for r in sec.iter_relocations())
            if n == 2:
                require(elf["e_type"] == "ET_EXEC" and elf.get_section_by_name(".note.jnox.abi")
                        and not any(s["p_type"] in ("PT_DYNAMIC","PT_INTERP") for s in elf.iter_segments()),
                        "static native service")
                require(not any(elf.get_section_by_name(".symtab").get_symbol_by_name(k) for k in FORBIDDEN),
                        "unexpected full-driver entry")
                for name in SYMBOLS:
                    sym, raw = routine(elf,name)
                    va = sym["st_value"]
                    layouts[name] = dict(entry=va, size=len(raw), prefix=raw[:16].hex(),
                                         filter=f"0x{va:x}+0x{len(raw):x}", peer=None)
    require(code[0] == code[1] and len(code[0]) == len(code[2]) and
            hashlib.sha256(code[0]).hexdigest() == m["callback"]["original_section_sha256"] and
            all(code[0][i:i+4] == code[2][i:i+4] for i in range(0,len(code[0]),4) if i//4 not in reloc),
            "linked original validation code changed")
    with (pkg/"native_peer.elf").open("rb") as f:
        for seg in ELFFile(f).iter_segments():
            if seg["p_type"] != "PT_LOAD" or not seg["p_flags"] & 1:
                continue
            va = seg["p_vaddr"]
            for lay in layouts.values():
                off = lay["entry"]-va
                if (va & ~4095) <= lay["entry"] < ((va+seg["p_memsz"]+4095) & ~4095):
                    require(off >= 0, "ambiguous peer text prefix")
                    lay["peer"] = seg.data()[off:off+16].ljust(16,b"\0").hex()
    return dict(kernel=sha(pkg/"kernel.elf"), user=sha(pkg/"user.elf"), import_sha256=sha(imp),
                manifest=sha(pkg/"manifest.json"), original_source=PIN, layouts=layouts)


def decode(pkg, ram):
    m=load(pkg/"manifest.json")
    expected=dict(queue_size=32,indices={"rx":0,"tx":1},status_after_setup="FEATURES_OK",
                  buffers_required_for_driver_ok=True,driver_source="drivers/net/virtio_net.c",
                  syscalls={"setup":161,"query":162})
    require(m["linux_packet_queues"] == expected and m["linux_source_net"]["driver_slot"] == 0 and
            m["linux_source_net"]["unauthorized_slots"] == [1] and m["linux_source_net"]["mmio_base"] == NIC
            and (m["profile"],m["board"],m["policy"]) == ("native","virt","fifo") and len(m["slots"]) == 2,
            "queue grant manifest")
    kernel=pkg/"kernel.elf"
    raw=kernel.read_bytes()
    require(sha(kernel)==m["kernel_elf_sha256"], "booted kernel hash")
    pool,length=journal_location(kernel,"jnox_linux_net_queue_pool")
    require(length == 8192 and pool%4096 == 0 and 0x40000000 <= pool <= 0x48000000-length,"queue pool extent")
    data, private = [], []
    ring_pages={pool,pool+4096}
    for i,root in enumerate(linked_roots(kernel,2)):
        slot=m["slots"][i]
        require(slot["slot"] == i and slot["elf"] == ("user.elf","native_peer.elf")[i],"slot package")
        elf=pkg/slot["elf"]
        require(sha(elf) == slot["elf_sha256"] and elf.read_bytes() in raw,"EL0 boot image")
        va,size=journal_location(elf,PROBES[i][0])
        require(size==PROBES[i][1]*8 and 0x400000<=va<va+size<=0x800000 and va%8==0,"receipt extent")
        private.append({int(permissions(ram,root,p*4096,"user rw"),16)&PA_MASK
                        for p in range(va//4096,(va+size-1)//4096+1)})
        data.append(list(struct.unpack(f"<{size//8}Q",vread(ram,root,va,size))))
        permissions(ram,root,NIC&~4095,"kernel rw",device=True)
        for page in ring_pages:
            permissions(ram,root,page,"kernel rw")
        # Inspect every live stage-1 leaf, including aliases at other user VAs.
        tables={root & PA_MASK}
        for level in range(3):
            next_tables=set()
            for table in tables:
                for (desc,) in struct.iter_unpack("<Q",ram.read(table,4096)):
                    if not desc&1:
                        continue
                    require(desc&3==3, "unexpected block mapping in native address space")
                    if level < 2:
                        next_tables.add(desc&PA_MASK)
                    elif (desc&PA_MASK) in ring_pages:
                        require(not (desc & (1<<6)), "EL0 alias of kernel queue pool")
            tables=next_tables
    require(private[0].isdisjoint(private[1]) and not (private[0]|private[1])&ring_pages,
            "overlapping ring or receipt backing")
    require(all(ram.read(p,4096)==bytes(4096) for p in sorted(ring_pages)),"nonempty stopped split ring")
    return m,data,pool

def verify_events(raw, pool, mtu, driver):
    physical, transport = [], []
    for line in raw.splitlines():
        if not MMIO.search(line):
            continue
        mem = re.search(rb"memory_region_ops_(read|write) cpu (-?\d+) mr \S+ addr (0x[0-9a-f]+) value (0x[0-9a-f]+) size (\d+)",line,re.I)
        if mem:
            address=int(mem[3],16)
            if NIC <= address < NIC+512:
                require(mem[2]==b"0","non-CPU0 NIC access")
                physical.append((mem[1].decode(),address-NIC,int(mem[4],16),int(mem[5])))
            continue
        model=re.search(rb"virtio_mmio_(read|write_offset) virtio_mmio_(?:read|write) offset (0x[0-9a-f]+)(?: value (0x[0-9a-f]+))?$",line,re.I)
        require(model,"unknown selected model event")
        transport.append(("read" if model[1]==b"read" else "write",int(model[2],16),
                          int(model[3],16) if model[3] else None))
    require(transport==[(op,off,val if op=="write" else None) for op,off,val,width in physical],
            "physical/model MMIO differ")
    status=[v for op,r,v,w in physical if (op,r)==("write",112)]
    require(status==[0,1,3,11,0]*2,"unexpected device status lifecycle or DRIVER_OK")
    starts=[i for i,(op,r,v,w) in enumerate(physical) if (op,r,v)==("read",0,0x74726976)]
    require(len(starts)==2 and starts[0]==0,"missing distinct device claims")
    layouts=[]
    for lo,hi in zip(starts,starts[1:]+[len(physical)]):
        rows=physical[lo:hi]
        maxes={}
        programmed=[]
        expected_writes=[]
        def w(off,val):
            expected_writes.append((off,val))
        def reset():
            w(112,0)
            w(48,0)
            w(48,1)
        reset()
        w(112,1)
        w(112,3)
        selected=driver[6]
        w(36,0)
        w(32,selected & 0xffffffff)
        w(36,1)
        w(32,selected >> 32)
        w(112,11)
        for idx in range(2):
            w(48,idx)
        for idx in range(2):
            base=pool+idx*4096
            w(48,idx)
            w(56,32)
            for reg,offset in ((128,0),(144,512),(160,584)):
                addr=base+offset
                w(reg,addr&0xffffffff)
                w(reg+4,addr>>32)
                programmed.append(addr)
            w(68,1)
        # Kernel verifies both activated queues, then the two SVC162 observations.
        for idx in (0,1,0,1):
            w(48,idx)
        reset()
        require([(off,val) for op,off,val,width in rows if op=="write" and off!=20] == expected_writes,
                "queue programming, address, reset, notify or driver-status mismatch")
        bank=queue=None
        queue_reads=[]
        reset_checks=[]
        status_value=None
        feature_banks=set()
        mtu_reads=[]
        for op,off,val,width in rows:
            require(width == (2 if off==266 else 4),"register width mismatch")
            if op=="write":
                if off==48:
                    queue=val
                elif off==20:
                    require(val in (0,1),"invalid offer selector")
                    bank=val
                elif off==112:
                    status_value=val
                continue
            if off==16:
                require(bank in (0,1) and (val & 0xffffffff)==driver[4+bank],"hardware feature offer mismatch")
                feature_banks.add(bank)
            elif off==266:
                mtu_reads.append(val)
            elif off==112:
                require(status_value==val,"device status readback mismatch")
                if val==0:
                    reset_checks.append([])
            elif off in (52,68):
                require(queue in (0,1),"queue read without selector")
                if status_value==0:
                    require(off==68 and val==0 and reset_checks,"reset ready not cleared")
                    reset_checks[-1].append(queue)
                else:
                    require(status_value==11,"queue read outside negotiated ownership")
                    if off==52:
                        require(32 <= val <= 32768 and maxes.setdefault(queue,val)==val,"hardware capacity mismatch")
                    queue_reads.append((queue,off,val))
            else:
                require(off in (0,4,8,252) and (off==252 or val=={0:0x74726976,4:2,8:1}[off]),
                        "unexpected NIC register read")
        require(feature_banks=={0,1} and mtu_reads==[mtu],"callback hardware feature/MTU reads")
        require(reset_checks==[[0,1],[0,1]],"reset readback did not confirm both queues")
        expected=[]
        for ready in (0,1,1):
            for idx in (0,1):
                expected.extend([(idx,68,ready),(idx,52,maxes[idx])])
        require(queue_reads==expected,"preflight, activation or queried QueueReady/QueueNumMax mismatch")
        layouts.append(dict(rx_max=maxes[0],tx_max=maxes[1],addresses=programmed))
    return layouts


def verify_execution(raw, layouts):
    lines=raw.splitlines()
    timeline=[]
    for name, required_count in zip(SYMBOLS,(2,4)):
        lay=layouts[name]
        attempts, completed=callback_attempts(lines,lay["entry"])
        require(len(completed)==required_count, "wrong execution count for "+name)
        translations=[]
        for i,line in enumerate(lines):
            ins=INSN.match(line)
            if ins and int(ins[1],16)==lay["entry"]:
                b=bytearray()
                for text in lines[i:i+4]:
                    ins=INSN.match(text)
                    if not ins or int(ins[1],16)!=lay["entry"]+len(b):
                        break
                    b.extend(int(ins[2],16).to_bytes(4,"little"))
                require(b and lay["prefix"].startswith(b.hex()) and
                        (lay["peer"] is None or not lay["peer"].startswith(b.hex())),"translated entry identity")
                translations.append(i)
        require(len(translations)==1 and translations[0]<attempts[0][0] and
                all(fields==attempts[0][1] for _,fields in attempts) and
                not any(EXEC.match(s) or s.startswith(b"IN:") for s in lines[translations[0]+1:attempts[0][0]]),
                "callback execution not bound to translated block")
        timeline.extend((idx,name) for idx in completed)
    require([name for _,name in sorted(timeline)]==[SYMBOLS[i] for i in (0,1,1,1,0,1)],
            "original callback and bus operations reordered")


def verdict(pkg, imp, out):
    meta=identities(pkg,imp)
    launch=load(out/"launch.json")
    require(launch["identities"]==meta and launch["schema"]==1 and (out/"net.events").read_text()==TRACE,
            "launch/build evidence mismatch")
    mtu,mac=launch["mtu"],launch["mac"]
    require(launch["argv"]==argv(launch["argv"][0],Path(launch["capture_dir"]),Path(launch["kernel_path"]),
                                 mtu,mac,{"filter":",".join(meta["layouts"][n]["filter"] for n in SYMBOLS)}),
            "machine, device or trace filters differ")
    qmp,halt=load(out/"qmp.json"),load(out/"halted.json")
    require(qmp["status"]["running"] and [c["cpu-index"] for c in qmp["cpus"]]==[0] and qmp["stop_return"]=={}
            and not halt["running"] and halt["status"]=="paused","single-CPU halt evidence")
    ram=RAM(load(out/"ram.json"))
    m,(d,p),pool=decode(pkg,ram)
    require(ram.used==set(ram.records),"unconsumed retained RAM")
    uart=(out/"uart.log").read_bytes().splitlines()
    for prefix,slot in zip((b"NATIVE EL0 ELF ",b"NATIVE EL0 PEER ELF "),m["slots"]):
        require(prefix+slot["elf_sha256"].encode() in uart,"booted ELF receipt")
    require({x for x in uart if x.startswith(b"NATIVE EL0 EXIT PID ")} ==
            {b"NATIVE EL0 EXIT PID 0 0",b"NATIVE EL0 EXIT PID 1 0"} and not any(b"NATIVE EL0 FAULT" in x for x in uart),
            "native terminal receipts")
    require(d[:4]==[0x5651445256303031,100,0,0] and d[4]&40==40 and d[5]&1 and
            d[6:15]==[(1<<32)|40,0,mtu,1,0,0,NEG-22,NEG-22,0] and
            0<d[15]<d[28] and d[18:20]==[32,32] and d[22:28]==[3,0,0,11,0,NEG-116] and
            d[29]==NEG-116,"owner validation, queue helper, retirement or generation receipts")
    require(p[:6]==[0x5651504545303031,100,0,NEG-13,NEG-13,6] and
            all(a<b for a,b in zip(p[6:11],p[7:12])) and p[11]-p[6]>=100_000_000,"peer denial or progress")
    raw=(out/"callback.log").read_bytes()
    layouts=verify_events(raw,pool,mtu,d)
    for i,layout in enumerate(layouts):
        maxima=[layout["rx_max"],layout["tx_max"]]
        require(d[16:18] == [(n<<32)|(32<<16)|1 for n in maxima] if i==0 else
                d[30:32] == [(n<<32)|(32<<16)|1 for n in maxima],"live queue inspection mismatch")
        if i==0:
            require(d[20:22]==maxima,"Linux virtqueue maximum mismatch")
    verify_execution(raw,meta["layouts"])
    return dict(tokens=[d[15],d[28]],queue_epochs=layouts,driver_status=11,stale_errno=-116,
                original_validate_entries=2,bus_find_vqs_entries=4,peer_wakes=p[5],pool=hex(pool),
                descriptors_posted=0,stopped_pool_zero_bytes=8192,original_source=PIN)


def capture(pkg, imp, out, binary, timeout, mtu, mac):
    qemu = shutil.which(binary)
    require(qemu, "QEMU executable missing")
    meta = identities(pkg, imp, live=True)
    out.mkdir(parents=True, exist_ok=False)
    (out/"net.events").write_text(TRACE)
    callback = {"filter": ",".join(meta["layouts"][n]["filter"] for n in SYMBOLS)}
    cmd = argv(qemu, out, pkg/"kernel.elf", mtu, mac, callback)
    launch = dict(schema=1, callback=callback, mtu=mtu, mac=mac, argv=cmd, capture_dir=str(out), kernel_path=str(pkg/"kernel.elf"),
                  identities=meta, qemu_sha256=sha(Path(qemu)),
                  qemu_version=subprocess.check_output([qemu, "--version"], text=True).splitlines()[0])
    with (out/"stderr.log").open("xb") as err, subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, bufsize=0) as proc:
        save(out/"launch.json", dict(launch, pid=proc.pid))
        watchdog = threading.Timer(timeout + 5, proc.kill)
        watchdog.start()
        try:
            class Pipe:
                write = proc.stdin.write
                readline = proc.stdout.readline
            stream = Pipe()
            require("QMP" in json.loads(stream.readline()), "missing QMP greeting")
            request(stream, "qmp_capabilities")
            until = time.monotonic()+timeout
            while time.monotonic() < until and proc.poll() is None:
                uart = out/"uart.log"
                raw = uart.read_bytes() if uart.exists() else b""
                if b"NATIVE EL0 FAULT" in raw or len(set(re.findall(
                        rb"(?m)^NATIVE EL0 EXIT PID ([01]) -?\d+\r?$", raw))) == 2:
                    break
                time.sleep(.02)
            capture_halt(stream, out)
            dest = out/"sample.bin"
            def read(pa, size):
                request(stream, "pmemsave", val=pa, size=size, filename=str(dest))
                return dest.read_bytes()
            ram = RAM({}, read)
            try:
                decode(pkg, ram)
            finally:
                save(out/"ram.json", ram.records)
                dest.unlink(missing_ok=True)
            request(stream, "quit")
            require(proc.wait(timeout=5) == 0, "QEMU exit")
            save(out/"result.json", verdict(pkg, imp, out))
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            watchdog.cancel()
            watchdog.join()


def mutation_checks(pkg,imp,out):
    passed=[]
    with tempfile.TemporaryDirectory(prefix="jnox-queues-") as tmp:
        root=Path(tmp)
        for name in ("launch.json","net.events","uart.log","qmp.json","halted.json","ram.json","callback.log"):
            shutil.copyfile(out/name,root/name)
        def reject(label,name,replacement,reason):
            path=root/name
            old=path.read_bytes()
            try:
                path.write_bytes(replacement)
                try:
                    verdict(pkg,imp,root)
                except ValueError as e:
                    require(reason in str(e),label+" failed at unrelated check: "+str(e))
                else:
                    raise ValueError("accepted mutated "+label)
            finally:
                path.write_bytes(old)
            passed.append(label)
        original=load(root/"ram.json")
        _,_,pool=decode(pkg,RAM(original))
        bad=dict(original)
        bad[f"{pool:x}/4096"]="01"+bad[f"{pool:x}/4096"][2:]
        reject("nonempty RX descriptor","ram.json",json.dumps(bad).encode(),"nonempty stopped split ring")
        for name,magic,index,new,reason in (
                ("unauthorized setup",0x5651504545303031,3,1,"peer denial"),
                ("stale owner accepted",0x5651445256303031,29,0,"generation receipts"),
                ("ready reported closed",0x5651445256303031,16,0,"live queue inspection"),
                ("reused token",0x5651445256303031,28,1,"generation receipts")):
            bad=dict(original)
            prefix=struct.pack("<Q",magic).hex()
            key=next(k for k,v in bad.items() if v.startswith(prefix))
            raw=bytearray.fromhex(bad[key])
            if name=="reused token":
                new=struct.unpack_from("<Q",raw,15*8)[0]
            struct.pack_into("<Q",raw,index*8,new)
            bad[key]=raw.hex()
            reject(name,"ram.json",json.dumps(bad).encode(),reason)
        raw=(root/"callback.log").read_bytes()
        lines=raw.splitlines(keepends=True)
        for i,line in enumerate(lines):
            if re.search(rb"memory_region_ops_write .*addr 0x0*a003e80\b",line,re.I):
                require(b"virtio_mmio_write_offset" in lines[i+1],"expected adjoining descriptor model event")
                changed=list(lines)
                for pos in (i,i+1):
                    changed[pos]=re.sub(rb"value 0x[0-9a-f]+",b"value 0x40",changed[pos],flags=re.I)
                reject("wrong descriptor base","callback.log",b"".join(changed),"queue programming")
                break
        else:
            raise ValueError("missing descriptor write in mutation control")
        meta=identities(pkg,imp)
        _,entries=callback_attempts(lines,meta["layouts"][SYMBOLS[1]]["entry"])
        i=entries[-1]
        reject("missing bus invocation","callback.log",b"".join(lines[:i]+lines[i+1:]),"wrong execution count")
    return passed


if __name__=="__main__":
    cli=argparse.ArgumentParser(description=__doc__)
    for key in ("package","driver-manifest","out"):
        cli.add_argument("--"+key,type=Path,required=True)
    cli.add_argument("--mtu",type=int,default=None)
    cli.add_argument("--mac",type=str.lower,default=None)
    cli.add_argument("--qemu",default="qemu-system-aarch64")
    cli.add_argument("--timeout",type=float,default=25)
    cli.add_argument("--replay",action="store_true")
    cli.add_argument("--mutation-tests",action="store_true")
    args=cli.parse_args()
    pkg,imp,out=(p.resolve() for p in (args.package,args.driver_manifest,args.out))
    if not args.replay:
        mtu=args.mtu if args.mtu is not None else 1410
        mac=args.mac or "52:54:00:19:26:41"
        if not 68<=mtu<=65535 or not re.fullmatch(r"(?:[0-9a-f]{2}:){5}[0-9a-f]{2}",mac):
            cli.error("invalid MTU or MAC")
        capture(pkg,imp,out,args.qemu,args.timeout,mtu,mac)
    launch=load(out/"launch.json")
    require((args.mtu is None or args.mtu==launch["mtu"]) and (args.mac is None or args.mac==launch["mac"]),
            "requested device parameters differ")
    result=verdict(pkg,imp,out)
    if args.mutation_tests:
        result["rejected_mutations"]=mutation_checks(pkg,imp,out)
    print(json.dumps(result,indent=2))
