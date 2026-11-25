#!/usr/bin/env python3
"""Offline #349 ELF preflight. Requires pyelftools and GNU AArch64 binutils."""
import argparse
import hashlib
import io
import json
import struct
import subprocess
import tarfile
from pathlib import Path

from elftools.elf.elffile import ELFFile

ARCHIVE = Path(__file__).resolve().parents[1] / "docs/evidence/native-extension-transaction-first.tar.gz"
RECEIPT = "29c60cf742e78c1fd3a38d1aa9332e5fadd07f39614c7d4434b3a2225f655cc8"
APPS = ("user.elf", "native_peer.elf", "native_peer2.elf")
PREFIX = "jnox_native_extension_stop_"
MAX = (1 << 64) - 1


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def signed(value, bits):
    return (value ^ (1 << (bits - 1))) - (1 << (bits - 1))


class Image:
    def __init__(self, data):
        self.elf = ELFFile(io.BytesIO(data))
        require(self.elf.elfclass == 64 and self.elf.little_endian and
                self.elf["e_machine"] == "EM_AARCH64", "expected little-endian AArch64")
        self.loads = [(s["p_flags"], s["p_vaddr"], s["p_paddr"], s["p_memsz"],
                       s["p_align"], s.data()) for s in self.elf.iter_segments() if s["p_type"] == "PT_LOAD"]
        self.sections = {s.name: s for s in self.elf.iter_sections() if s["sh_flags"] & 2}
        self.names = {}
        self.ranges = []
        for s in self.elf.get_section_by_name(".symtab").iter_symbols():
            if not isinstance(s["st_shndx"], int) or not s.name or s.name.startswith("$"):
                continue
            addr, size = s["st_value"], s["st_size"]
            self.names.setdefault(s.name, set()).add(addr)
            self.ranges.append((addr, size, s.name))

    def address(self, suffix):
        entries = self.names.get(PREFIX + suffix, set())
        require(len(entries) == 1, f"missing or ambiguous hook symbol: {suffix}")
        return next(iter(entries))

    def bytes(self, addr, length):
        for _, va, _, _, _, blob in self.loads:
            if va <= addr and addr + length <= va + len(blob):
                return blob[addr-va:addr-va+length]
        raise ValueError(f"address outside file-backed LOAD: {addr:#x}")

    def word(self, addr):
        return int.from_bytes(self.bytes(addr, 4), "little")


def decoded(w, pc):
    # Keep every non-immediate bit, including branch condition, width and registers.
    if w & 0x7c000000 == 0x14000000:
        bits, shift = 26, 0
    elif w & 0xff000010 == 0x54000000 or w & 0x7e000000 == 0x34000000 or w & 0x3b000000 == 0x18000000:
        bits, shift = 19, 5
    elif w & 0x7e000000 == 0x36000000:
        bits, shift = 14, 5
    elif w & 0x1f000000 == 0x10000000:
        delta = signed(((w >> 5 & 0x7ffff) << 2) | (w >> 29 & 3), 21)
        page = bool(w >> 31)
        return w & ~0x60ffffe0, ((pc & ~4095) + delta*4096 if page else pc+delta), page
    else:
        return None
    mask = (1 << bits) - 1
    return w & ~(mask << shift), pc + signed((w >> shift) & mask, bits)*4, False


def inspect_hook(im):
    start, loop, end, table = map(im.address, ("hook", "loop", "resume", "cells"))
    require(loop == start+24 and end == loop+4, "unexpected hook extent")
    w = [im.word(start+i*4) for i in range(7)]
    source = (w[2] >> 16) & 31
    require(im.word(start-4) == 0xd5033fdf and im.word(start-8) == 0xd50342ff,
            "hook must immediately follow DAIFClr #2; ISB")
    require(decoded(w[0], start) == (0x10000010, table, False) and w[1] == 0xa9404610 and
            source not in (16,17,31) and w[2] == (0xca000210 | source << 16) and
            decoded(w[3], start+12) == (0xb4000010, loop, False) and
            w[4] == (0xca000231 | source << 16) and
            decoded(w[5], start+20) == (0xb5000011, end, False) and w[6] == 0x14000000,
            "unexpected selector, scratch registers, control flow or stop-loop instruction")
    require(any(name == "native_extension_evaluate" and va <= start < end < va+size
                for va,size,name in im.ranges), "hook outside production evaluator")
    require(any(flags & 4 and not flags & 2 and va <= table and table+16 <= va+len(blob)
                for flags,va,_,_,_,blob in im.loads), "selector LOAD must be readable and immutable")
    require(any(not section["sh_flags"] & 5 and section["sh_addr"] <= table and
                table+16 <= section["sh_addr"]+section["sh_size"] for section in im.sections.values()),
            "selector is not in an allocated data section distinct from instructions")
    return start, end, table, source


def anchors(im, addr):
    return {(name,addr-va) for va,size,name in im.ranges
            if len(im.names[name]) == 1 and (addr == va or va < addr < va+size)}


def corresponding(a,b,old,new,hook):
    # Translate offsets in the evaluator around the single inserted instruction block.
    left = anchors(a, old)
    right = anchors(b, new)
    h,e = hook
    return sorted(left & {(name, off-(e-h) if name=="native_extension_evaluate" and new>=e else off)
                          for name,off in right if not name.startswith(PREFIX)})


def compare_code(a,b,old,new,length,hook,report):
    off = 0
    while off < length:
        p, q = old+off, new+off
        x, y = a.word(p), b.word(q)
        dx,dy = decoded(x,p),decoded(y,q)
        off += 4
        if not dx or not dy:
            require(x == y, f"unexplained instruction at {p:#x}/{q:#x}")
            continue
        require(dx[0] == dy[0], f"changed opcode/register/condition at {p:#x}/{q:#x}")
        t,u = dx[1],dy[1]
        if dx[2]:
            # Decode a contiguous ADRP + ADD/unsigned scalar load/store if present.
            # Other materializations cannot be justified from a page alone.
            v = a.word(p+4) if off < length else 0
            z = b.word(q+4) if off < length else 0
            base = x & 31
            if (v >> 5 & 31) == base and (z >> 5 & 31) == base and v & ~0x3ffc00 == z & ~0x3ffc00:
                scale = None
                if v & 0xffc00000 == 0x91000000:
                    scale = 0
                elif v & 0x3f000000 == 0x39000000:
                    scale = v >> 30
                if scale is not None:
                    t += ((v >> 10) & 4095) << scale
                    u += ((z >> 10) & 4095) << scale
                    off += 4
                elif x != y or t != u:
                    raise ValueError(f"unresolved ADRP at {p:#x}")
            elif x != y or t != u:
                raise ValueError(f"unresolved ADRP at {p:#x}")
        if t==u and x==y and p==q:
            # Absolute MMIO outside this image or a fixed page base can remain literal.
            if not any(va<=t<va+ms for _,va,_,ms,_,_ in a.loads+b.loads):
                continue
            if dx[2] and off == p-old+4:
                continue
        target = corresponding(a,b,t,u,hook)
        require(target, f"unresolved relocation {p:#x}->{t:#x}, {q:#x}->{u:#x}")
        if x==y and p==q and t==u:
            continue
        report.append(dict(old=p,new=q,old_word=x,new_word=y,old_target=t,new_target=u,
                           symbol_offset=target[0]))


def data_pointers(a,b,left,right,va,vb,hook,report):
    require(len(left)==len(right),"allocated data length differs")
    i=0
    while i<len(left):
        if left[i]==right[i]:
            i+=1
            continue
        pos=i-((va+i) % 8)
        require(pos>=0 and (vb+pos)%8==0 and pos+8<=len(left),"unexplained unaligned data change")
        t=int.from_bytes(left[pos:pos+8],"little")
        u=int.from_bytes(right[pos:pos+8],"little")
        target=corresponding(a,b,t,u,hook)
        require(target,f"unexplained data word at {va+pos:#x}/{vb+pos:#x}")
        report.append(dict(pointer_at=va+pos,new_at=vb+pos,old_target=t,new_target=u,symbol_offset=target[0]))
        i=pos+8


def check_loads(im):
    for flags,va,pa,ms,align,blob in im.loads:
        require(va==pa and flags in (5,6) and ms>=len(blob),"unexpected physical LOAD mapping")
        spans=sorted((s["sh_addr"]-va,s["sh_addr"]-va+s["sh_size"]) for s in im.sections.values()
                     if s["sh_type"]!="SHT_NOBITS" and va<=s["sh_addr"]<va+len(blob) and s["sh_size"])
        end=0
        for lo,hi in spans:
            require(end<=lo<=hi<=len(blob) and not any(blob[end:lo]),"unexplained LOAD gap")
            end=hi
        require(not any(blob[end:]),"unexplained LOAD tail")
    for s in im.sections.values():
        require(any(va<=s["sh_addr"] and s["sh_addr"]+s["sh_size"]<=va+ms and
                    bool(s["sh_flags"]&1)==bool(flags&2)
                    for flags,va,_,ms,_,_ in im.loads),"section mapping/permissions")


def compare_ordinary(a,b,report):
    h,e,table,reg = inspect_hook(b)
    require(not any(n.startswith(PREFIX) for n in a.names), "ordinary ELF contains bench hook")
    require(a.sections.keys()==b.sections.keys(), "allocated sections differ")
    require(a.elf["e_entry"]==b.elf["e_entry"] and [(l[0],l[4]) for l in a.loads]==[(l[0],l[4]) for l in b.loads],
            "changed entry or LOAD permissions/alignment")
    for name,s in a.sections.items():
        d = b.sections[name]
        require((s["sh_flags"],s["sh_type"])==(d["sh_flags"],d["sh_type"]), f"changed flags/type: {name}")
        x,y = s["sh_addr"],d["sh_addr"]
        n,m = s["sh_size"],d["sh_size"]
        if s["sh_flags"] & 4:
            pre = h-y if y <= h < y+m else n
            extra = e-h if pre != n else 0
            require(n%4==0 and m==n+extra, f"unexplained executable size: {name}")
            compare_code(a,b,x,y,pre,(h,e),report)
            compare_code(a,b,x+pre,y+pre+extra,n-pre,(h,e),report)
        elif s["sh_type"]=="SHT_NOBITS":
            require(n==m, f"changed zero-fill size: {name}")
        else:
            raw = d.data()
            if y <= table and table+16 <= y+m:
                i=table-y
                data_pointers(a,b,s.data()[:i],raw[:i],x,y,(h,e),report)
                data_pointers(a,b,s.data()[i:],raw[i+16:],x+i,y+i+16,(h,e),report)
            else:
                data_pointers(a,b,s.data(),raw,x,y,(h,e),report)
    return dict(start=h,loop=e-4,resume=e,selector=table,cell_register=reg)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("position",choices=("a","b"))
    for name in ("ordinary","dormant","active","out"):
        parser.add_argument(name,type=Path)
    parser.add_argument("--objdump",default="aarch64-linux-gnu-objdump")
    parser.add_argument("--readelf",default="aarch64-linux-gnu-readelf")
    args=parser.parse_args()
    args.out.mkdir(parents=True,exist_ok=False)
    report=dict(position=args.position,verdict="UNVERIFIED",packages={},relocations=[])
    try:
        historical=ARCHIVE.read_bytes()
        require(sha(historical)==RECEIPT,"archive identity")
        with tarfile.open(fileobj=io.BytesIO(historical)) as archive:
            def archived(name):
                return archive.extractfile("./accepted/"+name).read()
            old=archived("kernel.elf")
            reference=json.loads(archived("manifest.json"))
            apps=[archived(name) for name in APPS]
        report.update(archive_sha256=RECEIPT,historical_kernel=sha(old))
        dirs=[args.ordinary,args.dormant,args.active]
        require(len({p.resolve() for p in dirs})==3,"package paths must be distinct")
        images=[]
        for label,p in zip(("ordinary","dormant","active"),dirs):
            raw=(p/"kernel.elf").read_bytes()
            im=Image(raw)
            check_loads(im)
            images.append(im)
            manifest=json.loads((p/"manifest.json").read_bytes())
            require(sha((p/"build.log").read_bytes())==manifest["build_log_sha256"],"unbound build log")
            require(sha(raw)==manifest["kernel_elf_sha256"] and len(manifest["slots"])==3 and
                    manifest["policy"]=="extension","manifest/kernel binding")
            ops=manifest["extension_program"]
            require(ops["operations"]==reference["extension_program"]["operations"] and
                    sha((p/"extension_program.txt").read_bytes())==ops["sha256"]==reference["extension_program"]["sha256"],"rule mismatch")
            hashes=[]
            for i,(slot,name) in enumerate(zip(manifest["slots"],APPS)):
                app=(p/name).read_bytes()
                expected=apps[i if args.position=="a" else 2-i]
                argv=["/bin/app"]+(["four-batches"] if args.position=="b" and i==0 else [])
                require(app==expected and sha(app)==slot["elf_sha256"] and slot["slot"]==i and
                        slot["generation"]==(7,11,17)[i] and slot["cpu"]==0 and slot["role"]==("reserved" if i==0 else "delegated") and slot["argv"]==argv and sum(blob.count(app) for *_,blob in im.loads)==1,
                        f"{label}: slot {i} identity/argv/embedded image")
                hashes.append(sha(app))
            if label=="ordinary":
                require("extension_stop_bench" not in manifest,"ordinary marked as bench")
            else:
                cells=([] if label=="dormant" else [1,3] if args.position=="a" else [9])
                require(manifest["extension_stop_bench"]=={"position": args.position+"-"+label,"cells":cells},"incorrect bench label")
            info=dict(directory=str(p.resolve()),kernel=sha(raw),apps=hashes,manifest=sha((p/"manifest.json").read_bytes()))
            report["packages"][label]=info
            for command,flags,suffix in [(args.objdump,["-drw"],"asm"),(args.readelf,["-W","-a"],"map")]:
                listing=subprocess.check_output([command,*flags,str(p/"kernel.elf")])
                (args.out/(label+"."+suffix)).write_bytes(listing)
                info[suffix]=sha(listing)
        ordinary,dormant,active=images
        require(len({v["kernel"] for v in report["packages"].values()})==3,"identical input supplied")
        if args.position=="a":
            require(Image(old).loads==ordinary.loads,"operational rebuild differs from archived A")
        else:
            historical_image=Image(old)
            report["historical_scope"]="B reorders archived A apps and adds four-batches argv; check all executable sections against A."
            historical_code={k for k,v in historical_image.sections.items() if v["sh_flags"]&4}
            require(historical_code=={k for k,v in ordinary.sections.items() if v["sh_flags"]&4},"B executable sections differ")
            for name in sorted(historical_code):
                prior=historical_image.sections[name]
                now=ordinary.sections[name]
                require(prior["sh_size"]==now["sh_size"],"B executable size differs from historical A; reconciliation required")
                compare_code(historical_image,ordinary,prior["sh_addr"],now["sh_addr"],now["sh_size"],(0,0),report["relocations"])
            report["reordered_B_loads"]=[sha(l[-1]) for l in ordinary.loads]
        hook=inspect_hook(dormant)
        require(hook==inspect_hook(active),"pair hook addresses or operands differ")
        start,end,table,_=hook
        active_cells=(1,3) if args.position=="a" else (9,MAX)
        require(dormant.bytes(table,16)==struct.pack("<QQ",MAX,MAX) and
                active.bytes(table,16)==struct.pack("<QQ",*active_cells),"selector mismatch")
        require(len(dormant.loads)==len(active.loads),"different pair LOAD count")
        for l,r in zip(dormant.loads,active.loads):
            require(l[:-1]==r[:-1] and len(l[-1])==len(r[-1]),"different pair LOAD geometry")
            blob=r[-1]
            i=table-r[1]
            if 0<=i<=len(blob)-16:
                blob=blob[:i]+dormant.bytes(table,16)+blob[i+16:]
            require(l[-1]==blob,"pair differs outside selector")
        report["hook"]=compare_ordinary(ordinary,dormant,report["relocations"])
        report["verdict"]="PASS"
    except Exception as exc:
        report["reason"]=str(exc)
        raise
    finally:
        (args.out/"equivalence.json").write_text(json.dumps(report,indent=2)+"\n")


if __name__=="__main__":
    main()
