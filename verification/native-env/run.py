#!/usr/bin/env python3
"""Build, package and boot the ordinary and Picolibc #462 fixtures on Linux."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SDK = ROOT / "sdk/native-aarch64"
sys.path.insert(0, str(ROOT/"tools"))
from verify_native_display import observed_exits

ENVS = (["WAYLAND_DISPLAY=wayland-a", "XDG_RUNTIME_DIR=/run/user/A", "EMPTY=", "OPTION=x=y"],
        ["XDG_RUNTIME_DIR=/run/user/B", "OPTION=p q=r", "WAYLAND_DISPLAY=wayland-b", "EMPTY="])


def run(*command):
    subprocess.run(list(map(str,command)),check=True,cwd=ROOT)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pico(runtime, out):
    prefix = runtime/"target"
    lib = prefix/"lib/libc.a"
    manifest = json.loads((runtime/"manifest.json").read_bytes())
    if (manifest["picolibc_sha256"] != "d1fc2a20f217472f8d9da084a519a25678e8d91852a41c7f3f1840d3508f7163" or
            sha(lib) != manifest["artifacts"]["target/lib/libc.a"]):
        raise ValueError("selected Picolibc runtime archive identity")
    elf = out/"picolibc.elf"
    # Compile both current-checkout CRT sources. An older runtime directory
    # supplies the pinned libc archive and target headers, never its CRT objects.
    run("clang","--target=aarch64-none-elf","-mcpu=cortex-a72","-fuse-ld=lld","-Os",
        "-mno-outline-atomics","-ffreestanding","-nostdlib","-nostdlibinc","-static","-fno-pic","-fno-pie",
        "-fno-stack-protector","-fno-unwind-tables","-fno-asynchronous-unwind-tables",
        "-ffunction-sections","-fdata-sections","-isystem",prefix/"include","-iquote",SDK/"include",
        "-DCHECK_PICOLIBC",SDK/"graphics/start.S",SDK/"graphics/runtime.c",HERE/"probe.c",lib,
        "-Wl,--gc-sections,-z,max-page-size=4096,--build-id=none","-Wl,-T,"+str(SDK/"graphics/app.ld"),
        "-Wl,-Map,"+str(out/"picolibc.map"),"-o",elf)
    return elf


def expected_hash(entries):
    value = 2166136261
    for entry in entries:
        for byte in entry.encode()+b"\0":
            value = ((value ^ byte)*16777619) & 0xffffffff
    return f"{value:08x}"


def verify_uart(uart, kind, envs):
    raw = uart.read_bytes()
    statuses = [73 if envs[i] else 83 for i in range(2)]
    exits = sorted(observed_exits(raw))
    if exits != [(0,statuses[0]),(1,statuses[1]+1)]:
        raise ValueError(f"unexpected or duplicate guest exits {exits!r}")
    expected = []
    for role, env in zip("AB",envs):
        suffix = f"{len(env)} {expected_hash(env)}"
        main = f"JNOX ENV M {role} {suffix}"
        expected.append(main)
        if kind == "picolibc":
            ctor = f"JNOX ENV C {role if env else '-'} {suffix}"
            expected.append(ctor)
            if raw.find(ctor.encode()) > raw.find(main.encode()):
                raise ValueError("constructor after main")
    seen = [line.decode("ascii") for line in raw.splitlines() if line.startswith(b"JNOX ENV ")]
    if Counter(seen) != Counter(expected):
        raise ValueError(f"environment byte receipts {seen!r}")
    syms = json.loads(uart.with_suffix(".symbols.json").read_bytes())
    lines = uart.with_suffix(".trace").read_bytes().splitlines()
    pcs = Counter()
    for i,line in enumerate(lines):
        match = re.match(rb"^Trace 0: 0x[0-9a-f]+ \[[0-9a-f]+/([0-9a-f]+)/",line,re.I)
        if match and (i+1 == len(lines) or not lines[i+1].startswith(b"Stopped execution of TB chain before")):
            pcs[int(match[1],16)] += 1
    counts = {name:pcs[addr] for name,(addr,_) in syms.items()}
    if counts["main"] != 2 or (kind == "picolibc" and (counts["env_constructor"] != 2 or counts["getenv"] < 2)):
        raise ValueError(f"unexpected executed entry counts {counts}")
    return {"exits":exits,"receipts":seen,"entry_counts":counts,"uart_sha256":sha(uart)}


def boot(elf, out, kind, case):
    work = out/(kind+"-"+case)
    values = {"distinct": ENVS, "empty": ([],[]), "mixed": ([],ENVS[1])}[case]
    names = [role if values[i] else "empty-"+role for i,role in enumerate("AB")]
    args = [sys.executable,ROOT/"tools/build_native_el0.py","--board","virt","--policy","fifo",
            "--out",work,"--elf",elf,"--arg",names[0],"--priority","4","--peer-elf",elf,
            "--peer-arg",names[1],"--peer-priority","5","--console-write","0","--console-write","1"]
    # Interleave the two slots to verify stable per-slot ordering.
    for n in range(max(map(len,values))):
        for slot,env in enumerate(values):
            if n < len(env):
                args += ["--env",f"{slot}:{env[n]}"]
    run(*args)
    if sha(work/"user.elf") != sha(elf) or sha(work/"native_peer.elf") != sha(elf):
        raise ValueError("both embedded inputs must match the application under test")
    uart = work/"uart.log"
    capture = [sys.executable,ROOT/"examples/native-pixman/capture.py",work/"kernel.elf",uart,"2","--symbol","main"]
    if kind == "picolibc":
        capture += ["--symbol","env_constructor","--symbol","getenv"]
    run(*capture)
    report = verify_uart(uart,kind,values)
    report.update(argv=names, environment=values, application_sha256=sha(elf), kernel_sha256=sha(work/"kernel.elf"))
    (work/"receipt.json").write_text(json.dumps(report,indent=2)+"\n")
    print(kind,case,report["exits"],report["entry_counts"],flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out",type=Path,help="new output directory")
    parser.add_argument("--runtime",type=Path,required=True,help="Picolibc runtime produced by sdk/native-aarch64/graphics/build.py")
    parser.add_argument("--empty",action="store_true",help="also boot two empty environments and a mixed empty/populated pair")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True,exist_ok=False)
    plain = out/"ordinary.elf"
    run("sh",SDK/"build.sh",HERE/"probe.c",plain)
    selected = pico(args.runtime.resolve(),out)
    for kind,elf in (("ordinary",plain),("picolibc",selected)):
        for case in (("distinct","empty","mixed") if args.empty else ("distinct",)):
            boot(elf,out,kind,case)


if __name__ == "__main__":
    main()
