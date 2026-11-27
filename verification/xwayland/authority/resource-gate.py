"""Exercise the real five-role resource provider and ordinary four-role control."""
import argparse
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import struct
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CASES = {
    "positive": ("srv", "cli", [44,40,44,44,40]),
    "crossed": ("cli", "srv", [44,41,44,44,41]),
    "server-only": ("srv", None, [44,40,44,44,42]),
    "client-only": (None, "cli", [44,42,44,44,40]),
    "absent": (None, None, [44,42,44,44,42]),
    "invalid": (0, 257, [44,43,44,44,43]),
    "reverse-invalid": (257, 0, [44,43,44,44,43]),
    "boundaries": (1, 256, [44,46,44,44,47]),
    "ordinary": ("srv", "cli", [45]*4),
}


def package(out, probe, five):
    cmd = ["python3", "tools/build_native_el0.py", "--board", "virt", "--policy", "fifo",
           "--display-service", "0", "--socket-service", "3", "--shared-fd"]
    if five:
        cmd.append("--display-five-active")
    for slot in ([0,1,2,4] if five else [0,1,2]):
        cmd += ["--ipc-route", f"{slot}:3"]
    cmd += ["--elf", probe, "--priority", "7", "--peer-elf", probe, "--peer-priority", "6",
            "--peer-arg", "S", "--peer-elf", probe, "--peer-priority", "5",
            "--peer-elf", probe, "--peer-priority", "4"]
    if five:
        cmd += ["--peer-elf", probe, "--peer4-arg", "C", "--peer-priority", "3"]
    subprocess.run([*cmd, "--out", out], cwd=ROOT, check=True)


def payload(spec):
    if isinstance(spec,int):
        return secrets.token_bytes(spec)
    # Big-endian FamilyLocal and four length-prefixed Xauthority fields.
    parts = (spec.encode(), b"0", b"MIT-MAGIC-COOKIE-1", secrets.token_bytes(16))
    return struct.pack(">H",256) + b"".join(struct.pack(">H",len(p))+p for p in parts)


def boot(qemu, kernel, ser, cli, wanted):
    cmd = [qemu,"-machine","virt,gic-version=2","-cpu","cortex-a72","-accel","tcg",
           "-smp","1","-m","128M","-nodefaults","-display","none","-monitor","none",
           "-no-reboot","-serial","stdio","-device","ramfb","-kernel",kernel]
    with tempfile.TemporaryDirectory() as temp:
        for role,spec in (("server",ser),("client",cli)):
            if spec is None:
                continue
            path=Path(temp)/role
            with os.fdopen(os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),"wb") as fp:
                fp.write(payload(spec))
            cmd += ["-fw_cfg",f"name=opt/jnox/xauth-{role},file={path}"]
        # Never persist the UART stream. Keep only the terminal role/exit tuples.
        found={}
        pattern=re.compile(rb"NATIVE EL0 EXIT PID (\d+) (\d+)")
        with subprocess.Popen(cmd,stdout=subprocess.PIPE,stdin=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL) as proc, selectors.DefaultSelector() as sel:
            try:
                sel.register(proc.stdout,selectors.EVENT_READ)
                partial=b""
                deadline=time.monotonic()+45
                while len(found)<len(wanted) and time.monotonic()<deadline:
                    if not sel.select(.5):
                        continue
                    data=os.read(proc.stdout.fileno(),65536)
                    if not data:
                        break
                    lines=(partial+data).split(b"\n")
                    partial=lines.pop()[-128:]
                    for line in lines:
                        match=pattern.fullmatch(line.rstrip(b"\r"))
                        if match:
                            pid,code=map(int,match.groups())
                            if pid in found:
                                raise ValueError("duplicate process exit")
                            found[pid]=code
            finally:
                proc.kill()
                proc.wait()
    return [found.get(i) for i in range(len(wanted))]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out",type=Path)
    parser.add_argument("--qemu",default="qemu-system-aarch64")
    args=parser.parse_args()
    out=args.out.resolve()
    out.mkdir()
    probe=out/"probe.elf"
    subprocess.run(["sh","sdk/native-aarch64/build.sh",HERE/"resource-probe.c",probe],cwd=ROOT,check=True)
    package(out/"five",probe,True)
    package(out/"four",probe,False)
    failures=[]
    for case,(srv,cli,wanted) in CASES.items():
        observed=boot(args.qemu,out/("four" if case=="ordinary" else "five")/"kernel.elf",srv,cli,wanted)
        result=dict(case=case,exits=observed,accepted=observed==wanted)
        (out/(case+".json")).write_text(json.dumps(result)+"\n")
        print(json.dumps(result),flush=True)
        if not result["accepted"]:
            failures.append(case)
    if failures:
        raise SystemExit("failed cases: "+", ".join(failures))


if __name__=="__main__":
    main()
