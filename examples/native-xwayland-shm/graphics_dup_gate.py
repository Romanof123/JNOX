"""Build an isolated Picolibc caller and collect its guest exit."""
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

runtime, out, qemu = map(Path,sys.argv[1:4])
mode = sys.argv[4:]
tmpfs_modes = {"--tmpfs-rw": "rw", "--tmpfs-ro": "ro", "--tmpfs-unset": "unset"}
if mode and (len(mode) != 1 or mode[0] not in {
        "--seals", "--clockres", "--interfaces", "--signals", "--signal-default",
        "--utc-unset", "--utc-seeded", *tmpfs_modes}):
    raise SystemExit("unknown public Picolibc guest selection")
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
objects = []
package_options = []
if mode:
    flags = ["-I" + str(prefix/"include/jnox-wayland"), *flags]
    if mode == ["--interfaces"]:
        verification = repo / "verification/xwayland"
        cases = [
            ("clock_resolution", os_port / "clockres_caller.c", 76),
            ("shared_seals", os_port / "seal_caller.c", 75),
            ("open_limit", os_port / "open_max_caller.c", 76),
            ("relative_sleep", verification / "sleep/guest.c", 77),
            ("anonymous_file", os_port / "memfd_io_caller.c", 76),
            ("credentials", verification / "credentials/guest.c", 84),
            ("replace_descriptor", os_port / "dup2_caller.c", 76),
        ]
        source = out / "interface-caller.c"
        declarations = []
        calls = []
        for index, (name, unit, success) in enumerate(cases):
            obj = out / (name + ".o")
            subprocess.run([*cc, *flags, "-iquote", sdk / "include", "-Dmain=probe_" + name,
                            "-c", unit, "-o", obj], check=True)
            objects.append(obj)
            declarations.append(f"int probe_{name}(void);")
            calls.append(f"    if (probe_{name}() != {success}) return {111 + index};")
        expected = 86
        package_options = ["--console-write", "0"]
        source.write_text("\n".join([*declarations, "int main(void) {", *calls,
                                    f"    return {expected};", "}"]) + "\n")
    elif mode in (["--utc-unset"], ["--utc-seeded"]):
        source = repo / "verification/xwayland/wall-clock/guest.c"
        expected = 87
        if mode == ["--utc-seeded"]:
            flags.append("-DJNOX_TEST_UTC_SEEDED")
            package_options = ["--boot-utc-seconds", "2208988800"]
    elif mode[0] in tmpfs_modes:
        selection = tmpfs_modes[mode[0]]
        source = repo / "verification/xwayland/tmpfs/guest.c"
        expected = 88
        grant = "ro" if selection == "ro" else "rw"
        package_options = ["--arg", selection, "--tmpfs", "0:" + grant]
        if selection != "unset":
            package_options += ["--boot-utc-seconds", "1700000000"]
    elif mode in (["--signals"], ["--signal-default"]):
        source = repo / "verification/xwayland/signals/guest.c"
        expected = 86
        if mode == ["--signal-default"]:
            expected = 143
            package_options = ["--arg", "default"]
    else:
        name, expected = ("clockres_caller.c", 76) if mode == ["--clockres"] else ("seal_caller.c", 75)
        source = os_port/name
else:
    source = out/"picolibc-caller.c"
    source.write_text('#define _POSIX_C_SOURCE 200809L\n#include "upstream-dup.inc"\n'
                      '#define JNOX_FRAME_DUP upstream_outgoing_fd\n'
                      f'#include "{os_port}/dup_caller.c"\n')
    expected = 74
subprocess.run([*cc,*flags,"-iquote",sdk/"include","-c",source,"-o",out/"picolibc-caller.o"],check=True)
elf = out/"picolibc-caller.elf"
subprocess.run([*cc,out/"picolibc-caller.o",*objects,prefix/"lib/libjnox-wayland-os.a",*ld,
                "-Wl,-Map,"+str(out/"picolibc-caller.map"),"-o",elf],check=True)
package = out/"picolibc-package"
subprocess.run([sys.executable,"tools/build_native_el0.py","--board","virt","--policy","fifo","--priority","7",
                "--elf",elf,"--peer-elf",out/"broker.elf","--peer-arg","1","--peer-priority","1",
                "--socket-service","1","--shared-fd","--ipc-route","0:1",*package_options,"--out",package],check=True)
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
                if status != expected:
                    raise ValueError(f"public Picolibc caller exited {status}")
                print(f"Public Picolibc caller exited {status}.",flush=True)
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
