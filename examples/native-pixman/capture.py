#!/usr/bin/env python3
"""Capture a native virt guest until every expected process has exited."""
import argparse, json, os, re, selectors, subprocess, time
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("kernel", type=Path)
p.add_argument("log", type=Path)
p.add_argument("count", type=int)
p.add_argument("--console-grant", action="store_true")
p.add_argument("--symbol", action="append", help="capture execution in each named ELF symbol")
a = p.parse_args()
argv = ["qemu-system-aarch64","-machine","virt,gic-version=2","-cpu","cortex-a72",
        "-smp","1","-m","128M","-nic","none","-display","none","-monitor","none","-serial","stdio",
        "-kernel",str(a.kernel)]
nm = subprocess.check_output(["llvm-nm","-S",str(a.kernel.parent/"user.elf")],text=True)
want = {"main"} if a.console_grant else {"pixman_constructor", "pixman_image_composite32", "graphics_fp_yield", "abort"}
if a.symbol: want = set(a.symbol)
ranges = {}
for row in nm.splitlines():
    cols=row.split()
    if len(cols) == 4 and cols[3] in want: ranges[cols[3]]=[int(cols[0],16),max(4,int(cols[1],16))]
if set(ranges) != want: raise RuntimeError("incomplete trace filter")
trace = a.log.with_suffix(".trace")
argv.extend(["-d","in_asm,exec,nochain","-dfilter",",".join(f"0x{addr:x}+0x{size:x}" for addr,size in ranges.values()),"-D",str(trace)])
a.log.with_suffix(".symbols.json").write_text(json.dumps(ranges))
def exit_count(text):
    if a.count == 1 and re.search(rb"(?m)^NATIVE EL0 EXIT -?\d+\r?\n",text): return 1
    return len(set(re.findall(rb"(?m)^NATIVE EL0 EXIT PID (\d+) -?\d+\r?\n",text)))
with a.log.open("xb") as f:
    proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    try:
        sel = selectors.DefaultSelector()
        sel.register(proc.stdout, selectors.EVENT_READ)
        end = time.monotonic()+60
        text = b""
        while time.monotonic() < end:
            if not sel.select(max(0,end-time.monotonic())): break
            chunk=os.read(proc.stdout.fileno(),4096)
            if not chunk: break
            f.write(chunk); f.flush()
            text += chunk
            if b"NATIVE EL0 FAULT" in text: raise RuntimeError("native EL0 fault")
            if exit_count(text) == a.count:
                break
        else:
            raise RuntimeError("guest deadline")
        if exit_count(text) != a.count:
            raise RuntimeError("incomplete guest exits")
    finally:
        proc.terminate()
        try: proc.wait(timeout=4)
        except subprocess.TimeoutExpired: proc.kill(); proc.wait()
