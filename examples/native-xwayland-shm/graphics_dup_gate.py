"""Build isolated Picolibc public-fcntl caller and collect its genuine guest exit."""
import ast
import configparser
import hashlib
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

runtime, out, qemu = map(Path,sys.argv[1:])
repo = Path.cwd()
sdk = repo/"sdk/native-aarch64"
os_port = sdk/"graphics/os"
prefix = out/"graphics-target"
shutil.copytree(runtime/"target", prefix)
manifest = json.loads((runtime/"manifest.json").read_bytes())
for name in ("libc.a", "libm.a"):
    if hashlib.sha256((prefix/"lib"/name).read_bytes()).hexdigest() != manifest["artifacts"]["target/lib/"+name]:
        raise ValueError("Picolibc graphics archive mismatch: "+name)
subprocess.run([sys.executable,os_port/"install.py","--prefix",prefix],check=True)
cross = configparser.ConfigParser(interpolation=None)
cross.read(runtime/"jnox.ini")
cc = ast.literal_eval(cross["binaries"]["c"])
flags = ast.literal_eval(cross["built-in options"]["c_args"])
ld = ast.literal_eval(cross["built-in options"]["c_link_args"])
source = out/"picolibc-caller.c"
source.write_text('#define _POSIX_C_SOURCE 200809L\n#include "upstream-dup.inc"\n'
                  '#define JNOX_FRAME_DUP upstream_outgoing_fd\n'
                  f'#include "{os_port}/dup_caller.c"\n')
subprocess.run([*cc,*flags,"-iquote",sdk/"include","-c",source,"-o",out/"picolibc-caller.o"],check=True)
elf = out/"picolibc-caller.elf"
subprocess.run([*cc,out/"picolibc-caller.o",prefix/"lib/libjnox-wayland-os.a",*ld,
                "-Wl,-Map,"+str(out/"picolibc-caller.map"),"-o",elf],check=True)
package = out/"picolibc-package"
subprocess.run([sys.executable,"tools/build_native_el0.py","--board","virt","--policy","fifo","--priority","7",
                "--elf",elf,"--peer-elf",out/"broker.elf","--peer-arg","1","--peer-priority","1",
                "--socket-service","1","--shared-fd","--ipc-route","0:1","--out",package],check=True)
uart = out/"picolibc-uart.bin"
cmd = list(map(str,[qemu,"-machine","virt,gic-version=2","-cpu","cortex-a72","-smp","1","-accel","tcg",
                    "-m","128M","-nodefaults","-display","none","-monitor","none","-no-reboot",
                    "-serial","file:"+str(uart),"-kernel",package/"kernel.elf"]))
(out/"picolibc-launch.json").write_text(json.dumps(cmd,indent=2))
with (out/"picolibc-stderr").open("xb") as err:
    proc = subprocess.Popen(cmd,stdout=subprocess.DEVNULL,stderr=err)
    try:
        until = time.monotonic()+60
        while time.monotonic()<until:
            data = uart.read_bytes() if uart.exists() else b""
            match = re.search(rb"(?m)^NATIVE EL0 EXIT PID 0 (-?\d+)\r?\n",data)
            if match:
                status = int(match[1])
                if status != 74:
                    raise ValueError(f"public Picolibc caller exited {status}")
                print("Public Picolibc fcntl caller exited 74.",flush=True)
                break
            if proc.poll() is not None:
                raise ValueError(f"QEMU exited {proc.returncode}")
            time.sleep(0.1)
        else:
            raise TimeoutError("public caller exit missing")
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
