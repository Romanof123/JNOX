"""Source-built preflight. Imported by native_extension_equivalence.py source-built."""
from collections import Counter
import argparse
import io
import json
from pathlib import Path
import re
import struct
import tarfile

from native_extension_equivalence import APPS, ARCHIVE, Image, MAX, RECEIPT, decoded, require, sha

ENTRY = "jnox_extension_evaluator_enter"
ENTRY_SEC = ".jnox_extension_entry"
SELECTOR = ".jnox_extension_selector"
CELLS = "jnox_native_extension_stop_cells"
LOOP = "jnox_native_extension_stop_loop"
LABELS = ("ordinary", "dormant", "active")
FIRST = "359d4cc8667ffaf66f13ddcc1c24376bb32718ea4f9335391e4ace8eec0e837f"


def check_entry(im, position, label):
    section = im.sections[ENTRY_SEC]
    addr = section["sh_addr"]
    require(section["sh_flags"] == 6 and section["sh_size"] == 64 and
            section["sh_addralign"] == 64 and addr % 64 == 0 and
            im.names.get(ENTRY) == {addr}, "fixed RX source entry")
    words = struct.unpack("<16I", im.bytes(addr, 64))
    require(words[:2] == (0xd50342ff, 0xd5033fdf), "entry must unmask IRQ and ISB")
    if label == "ordinary":
        require(words[2:] == (0xd65f03c0,) + (0,)*13 and SELECTOR not in im.sections and
                not any(n.startswith("jnox_native_extension_stop_") for n in im.names),
                "ordinary entry must return with no selector or loop")
    else:
        section = im.sections[SELECTOR]
        table = section["sh_addr"]
        require(section["sh_flags"] == 2 and section["sh_size"] == 16 and table % 8 == 0 and
                section["sh_addralign"] == 8 and im.names.get(CELLS) == {table} and
                im.names.get(LOOP) == {addr+36}, "fixture selector and loop")
        require(any(flags==4 and va==pa==table and mem==len(data)==16
                    for flags,va,pa,mem,align,data in im.loads), "isolated immutable selector mapping")
        require(decoded(words[2],addr+8) == (0x10000010,table,False) and
                words[3:5] == (0xa9404610,0xca000210) and
                decoded(words[5],addr+20) == (0xb4000010,addr+36,False) and
                words[6] == 0xca000231 and decoded(words[7],addr+28) == (0xb4000011,addr+36,False) and
                words[8:] == (0xd65f03c0,0x14000000)+(0,)*6, "fixed x0 selector semantics")
        expected = (MAX, MAX) if label=="dormant" else (1,3) if position=="a" else (9,MAX)
        require(im.bytes(table,16) == struct.pack("<QQ",*expected), "selector contents")
    pc, length, = [(pc,n) for pc,n,name in im.ranges if name=="native_extension_evaluate"][0]
    calls=[at for at in range(pc,pc+length,4) if decoded(im.word(at),at)==(0x94000000,addr,False)]
    require(len(calls)==1, "single evaluator BL to fixed entry")
    return addr,calls[0]


def layout(im, fixture, mask_entry):
    # Never mask other bytes in the RX segment, even unattributed alignment gaps.
    entry=im.sections[ENTRY_SEC]["sh_addr"]
    selector=im.sections[SELECTOR]["sh_addr"] if fixture else None
    loads=[]
    for flags,va,pa,mem,align,blob in im.loads:
        require(va==pa and mem>=len(blob) and flags in (4,5,6), "LOAD permissions or address")
        if fixture and va==selector and flags==4 and mem==len(blob)==16:
            continue
        if va<=entry<va+len(blob):
            offset=entry-va
            require(flags==5 and offset+64<=len(blob), "entry mapping")
            if mask_entry:
                blob=blob[:offset]+bytes(64)+blob[offset+64:]
        loads.append((flags,va,pa,mem,align,len(blob),sha(blob)))
    sections=sorted((name,s["sh_addr"],s["sh_size"],s["sh_flags"],s["sh_type"],s["sh_addralign"])
                    for name,s in im.sections.items() if name!=SELECTOR)
    syms=Counter((a,n,name) for a,n,name in im.ranges if name not in (CELLS,LOOP))
    return im.elf["e_entry"],loads,sections,syms


def read_json(path):
    return json.loads(path.read_bytes())


def package(directory,position,label,reference,apps):
    data=(directory/"kernel.elf").read_bytes()
    im=Image(data)
    manifest=read_json(directory/"manifest.json")
    require(manifest["kernel_elf_sha256"]==sha(data) and sha((directory/"build.log").read_bytes())==manifest["build_log_sha256"]
            and manifest["board"]=="virt" and manifest["policy"]=="extension" and len(manifest["slots"])==3,
            "position kernel/build/target binding")
    rule=(directory/"extension_program.txt").read_bytes()
    require(sha(rule)==manifest["extension_program"]["sha256"]==reference["extension_program"]["sha256"]
            and manifest["extension_program"]["operations"]==reference["extension_program"]["operations"], "rule binding")
    for i,(slot,name) in enumerate(zip(manifest["slots"],APPS)):
        expected=apps[i if position=="a" else 2-i]
        require((directory/name).read_bytes()==expected and slot["elf_sha256"]==sha(expected) and
                (slot["slot"],slot["cpu"],slot["generation"],slot["role"])==(i,0,(7,11,17)[i],"delegated" if i else "reserved")
                and slot["argv"]==["/bin/app"]+(["four-batches"] if position=="b" and not i else [])
                and sum(blob.count(expected) for *_,blob in im.loads)==1, "slot inputs, role and embedded bytes")
    if label=="ordinary":
        require("extension_stop_bench" not in manifest, "ordinary manifest marked as bench")
    else:
        cells=[] if label=="dormant" else [1,3] if position=="a" else [9]
        require(manifest["extension_stop_bench"]==dict(position=position+"-"+label,cells=cells),"fixture identity")
    return im,sha(data)


def bound_blob(link,digest):
    require(isinstance(digest,str) and re.fullmatch("[a-f0-9]{64}",digest),"invalid input digest")
    raw=(link/"blobs"/digest).read_bytes()
    require(sha(raw)==digest,"archived input differs from digest")
    return raw


def main(argv):
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("position",choices=("a","b"))
    for name in ("cohort","standalone","out"):
        p.add_argument(name,type=Path)
    a=p.parse_args(argv)
    a.out.mkdir(parents=True,exist_ok=False)
    report=dict(position=a.position,verdict="UNVERIFIED",scope="linked preflight only")
    try:
        old=ARCHIVE.read_bytes()
        require(sha(old)==RECEIPT,"historical accepted archive")
        with tarfile.open(fileobj=io.BytesIO(old)) as t:
            apps=[t.extractfile("./accepted/"+n).read() for n in APPS]
            reference=json.load(t.extractfile("./accepted/manifest.json"))
        first=ARCHIVE.with_name("native-extension-stop-first-preflight.tar.gz").read_bytes()
        require(sha(first)==FIRST,"historical rejected preflight")
        with tarfile.open(fileobj=io.BytesIO(first)) as t:
            proof=t.extractfile("./proof.txt").read()
            (a.out/"first-preflight-proof.txt").write_bytes(proof)
        report.update(rejected_preflight_archive=FIRST,accepted_archive=RECEIPT,prior_proof_sha256=sha(proof),
                      history="Archived-source A reproduced historical loads. B had reordered app arguments and its own archived-source control, no historical launch. Eight panic location lines moved by 26. First fixture timer instructions changed. No binary or timing equivalence transfers.")
        images={}
        outputs={}
        locations=[]
        require(a.standalone.resolve() not in [(a.cohort/l).resolve() for l in LABELS],"standalone ordinary must be independently supplied")
        for label in LABELS:
            images[label],outputs[label]=package(a.cohort/label,a.position,label,reference,apps)
            locations.append(check_entry(images[label],a.position,label))
        standalone,standalone_sha=package(a.standalone,a.position,"ordinary",reference,apps)
        require(layout(standalone,False,False)==layout(images["ordinary"],False,False),"independent ordinary reproducibility")
        require(len(set(locations))==1,"entry or evaluator call site moved")
        require(layout(images["ordinary"],False,True)==layout(images["dormant"],True,True),
                "trusted LOAD geometry, contents, symbols or section protections differ")
        require(layout(images["dormant"],True,False)==layout(images["active"],True,False),"active/dormant shared image differs")
        require(read_json(a.cohort/"spec.json")["position"]==a.position,"position binding")
        require(images["dormant"].sections[SELECTOR]["sh_addr"]==images["active"].sections[SELECTOR]["sh_addr"],"moved selector")
        report.update(kernels=outputs,standalone=standalone_sha,entry=locations[0][0],call=locations[0][1])
        report.update(provenance=check_inputs(a.cohort,outputs,images))
        report["verdict"]="PASS"
    except Exception as exc:
        report["reason"]=str(exc)
        raise
    finally:
        (a.out/"equivalence.json").write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")

def check_inputs(root, outputs, images):
    import posixpath
    import shlex
    link=root/"link"
    receipt=read_json(root/"cohort.json")
    spec=read_json(root/"spec.json")
    record=read_json(link/"inputs.json")
    require(receipt["spec_sha256"]==sha((root/"spec.json").read_bytes()) and
            receipt["inputs_sha256"]==sha((link/"inputs.json").read_bytes()), "cohort receipt")
    source=Path(__file__).resolve().parents[1]
    for key,name in (("entry_source_sha256","kernel/src/native_extension_entry.S"),
                     ("wrapper_sha256","tools/native_extension_cohort_link.py"),
                     ("driver_sha256","tools/native_extension_cohort.py")):
        require(spec[key]==sha((source/name).read_bytes()),"cohort source "+name)
    flags=record["expanded_flags"]
    ei,oi=record["entry_index"],record["output_index"]
    require(0<oi<len(flags) and flags[oi-1]=="-o" and flags.count("-o")==1 and ei!=oi,"link output index")
    cwd=read_json(link/"invocation.json")["cwd"]
    require(read_json(link/"invocation.json")["argv"]==record["raw_flags"],"original invocation")
    def absolute(arg):
        return posixpath.normpath(posixpath.join(cwd,arg))
    responses={r["path"]:bound_blob(link,r["sha256"]) for r in record["responses"]}
    def expand(args,depth=0):
        require(depth<8,"response nesting")
        for arg in args:
            if arg.startswith("@"):
                yield from expand(shlex.split(responses[absolute(arg[1:])].decode()),depth+1)
            else:
                yield arg
    require(list(expand(record["raw_flags"]))==flags,"archived response/argv expansion")
    trusted={}
    indices=[]
    for r in record["inputs"]:
        i=r["index"]
        require(0<=i<len(flags) and flags[i]==r["argument"] and i not in indices and
                r["kind"] in ("object","library","entry","script"),"ordered input identity")
        indices.append(i)
        arg=flags[i]
        path=arg[2:] if r["kind"]=="script" and arg.startswith("-T") else arg
        require(absolute(path)==r["path"],"archived input path")
        data=bound_blob(link,r["sha256"])
        if r["kind"]=="script":
            require(data==(source/"kernel/native-extension.ld").read_bytes(),"production linker script")
        elif r["kind"]=="library":
            require(data.startswith(b"!<arch>\n"),"non-archived or thin library")
        else:
            elf=ImageObject(data)
            require(elf["e_type"]=="ET_REL","trusted relocatable")
        require((r["kind"]=="entry")== (i==ei),"fixed entry input index")
        trusted[r["path"]]=r["sha256"]
    require(indices==sorted(indices) and ei in indices and any(r["kind"]=="object" for r in record["inputs"])
            and sum(r["kind"]=="script" for r in record["inputs"])==1,"missing ordered inputs")
    for i,arg in enumerate(flags):
        if i==oi or i in indices or (i and flags[i-1]=="-L") or arg.startswith("-L"):
            continue
        options = {"-o","-flavor","-T","--as-needed","--no-as-needed","-Bstatic","-Bdynamic",
                   "--gc-sections","--eh-frame-hdr","--strip-debug","--no-eh-frame-hdr","--no-undefined",
                   "--start-group","--end-group","--whole-archive","--no-whole-archive","--push-state",
                   "--pop-state","-static","-O1","-O2","--build-id=none","--no-rosegment","--no-relax",
                   "--discard-all","-z","gnu","noexecstack","relro","now"}
        require(arg in options or arg.startswith("--compress-debug-sections="),"unaccounted linker input "+arg)
    require(record["tools"]==spec["tools"],"tool pins")
    maps={}
    trusted_maps=[]
    for label in LABELS:
        meta=read_json(link/(label+".json"))
        require(receipt["links"][label]==sha((link/(label+".json")).read_bytes()) and
                receipt["manifests"][label]==sha((root/label/"manifest.json").read_bytes()) and
                meta["exit"]==0 and meta["output_sha256"]==outputs[label],"link/manifest receipt")
        argv=meta["argv"]
        require(argv[0]==record["tools"]["rust-lld"]["path"] and
                argv[-1]=="-Map="+spec["root"]+"/link/"+label+".map","linker/map path")
        actual=argv[1:-1]
        expected=list(flags)
        if label!="ordinary":
            expected[ei]=spec["root"]+"/link/"+label+".o"
            expected[oi]=spec["root"]+"/link/"+label+".elf"
        require(actual==expected,"trusted link order/options changed")
        entry_blob=bound_blob(link,meta["entry_sha256"])
        if label=="ordinary":
            require(meta["entry_sha256"]==trusted[absolute(flags[ei])],"ordinary entry object")
        obj=ImageObject(entry_blob)
        sec=obj.get_section_by_name(ENTRY_SEC)
        linked=images[label].sections[ENTRY_SEC].data()
        require(sec is not None and len(sec.data())==64 and sec.data()[:8]==linked[:8] and
                sec.data()[12:]==linked[12:],"entry object binding")
        if label=="ordinary":
            require(sec.data()==linked and obj.get_section_by_name(SELECTOR) is None,"ordinary entry object bytes")
        else:
            require(obj.get_section_by_name(SELECTOR).data()==images[label].sections[SELECTOR].data()
                    and (int.from_bytes(sec.data()[8:12],"little") & 0x9f00001f)==0x10000010,"entry selector relocation")
        rawmap=(link/(label+".map")).read_bytes()
        require(sha(rawmap)==meta["map_sha256"],"map digest")
        maps[label]=sha(rawmap)
        common=[]
        seen=set()
        # LLD maps list output sections and input sections with their VMA,
        # LMA, size and alignment; archive members remain distinguishable.
        for line in rawmap.decode().splitlines():
            m=re.fullmatch(r"\s*([0-9a-fA-F]+)\s+([0-9a-fA-F]+)\s+([0-9a-fA-F]+)\s+(\d+)\s+(.*?)\s*",line)
            if not m:
                continue
            va,pa,size=(int(m[i],16) for i in (1,2,3))
            name=m[5]
            if name in images[label].sections:
                s=images[label].sections[name]
                require((va,pa,size)==(s["sh_addr"],s["sh_addr"],s["sh_size"]),"map/ELF "+name)
                seen.add(name)
            if ":(" not in name or not any(s["sh_addr"] <= va < s["sh_addr"] + s["sh_size"]
                                              for s in images[label].sections.values()):
                continue
            origin,section=name.rsplit(":(",1)
            if origin=="<internal>":
                common.append((va,pa,size,name))
                continue
            path,_,member=origin.partition("(")
            if absolute(path)==absolute(actual[ei]):
                require(section.rstrip(")") in (ENTRY_SEC,SELECTOR,".text") or va==0,
                        "unexpected entry object contribution")
                continue
            require(absolute(path) in trusted,"map object absent from retained inputs: "+path)
            common.append((va,pa,size,trusted[absolute(path)],member,section))
        require(set(images[label].sections)<=seen and common,"incomplete linker map")
        trusted_maps.append(common)
    require(trusted_maps[0]==trusted_maps[1]==trusted_maps[2],"trusted input contribution moved")
    return dict(inputs_sha256=receipt["inputs_sha256"],maps=maps,common_input_sections=len(trusted_maps[0]),
                cohort_sha256=sha((root/"cohort.json").read_bytes()),tools=record["tools"])


def ImageObject(data):
    from elftools.elf.elffile import ELFFile
    obj=ELFFile(io.BytesIO(data))
    require(obj.elfclass==64 and obj.little_endian and obj["e_machine"]=="EM_AARCH64","AArch64 object")
    return obj
