#!/usr/bin/env python3
"""Retain a stock raspi4b cold boot and physical RAM for offline replay."""
import argparse
import json
import re
import shutil
import subprocess
import threading
import time
from pathlib import Path

from native_el0_capture import symbol_address
from ordinary_amc_capture import save, sha
from qemu_capture import request


def capture(a, observer=None):
    root = a.out.resolve()
    root.mkdir()
    package = a.package.resolve()
    manifest = json.loads((package / "manifest.json").read_bytes())
    shutil.copyfile(package / "manifest.json", root / "manifest.json")
    files = {s["elf"]: s["elf_sha256"] for s in manifest["slots"]}
    files.update({"kernel.elf": manifest["kernel_elf_sha256"],
                  "kernel8.img": manifest["kernel8_img_sha256"]})
    for name, digest in files.items():
        if Path(name).name != name:
            raise ValueError("invalid package filename")
        dest = root / name
        shutil.copyfile(package / name, dest)
        if sha(dest) != digest:
            raise ValueError(f"package digest mismatch for {name}")
        if name.endswith(".elf"):
            for tool, flags, suffix in [(a.nm, ["-nS"], ".nm"), (a.objdump, ["-d"], ".dis")]:
                with (root / (name + suffix)).open("xb") as out:
                    subprocess.run([tool, *flags, dest], stdout=out, check=True)
    symbols = (root / "kernel.elf.nm").read_text()
    base = symbol_address(symbols, "__native_payload_start")
    end = symbol_address(symbols, "__stack_top")
    if base != 0x02000000 or not base < end <= 0x04000000:
        raise ValueError("Pi kernel extent")
    qemu = Path(shutil.which(a.qemu) or a.qemu).resolve(strict=True)
    cmd = [str(qemu), "-M", "raspi4b", "-accel", "tcg", "-display", "none",
           "-serial", "file:" + str(root / "uart.bin"), "-qmp", "stdio",
           "-kernel", str(root / "kernel8.img")]
    if a.trace:
        cmd += ["-d", "int", "-D", str(root / "exceptions.log")]
    if observer is not None:
        cmd += observer.arguments(root)
    save(root / "launch.json", dict(argv=cmd, qemu_sha256=sha(qemu),
         version=subprocess.check_output([qemu, "--version"], text=True), timeout=120,
         ram_base=base, ram_end=end, trace=a.trace))
    reason = "timeout or guest stopped before terminal"
    with (root / "stderr").open("xb") as err:
        proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=err, bufsize=0)
        timer = threading.Timer(120, proc.kill)
        timer.start()
        try:
            class Pipe:
                write = proc.stdin.write
                readline = proc.stdout.readline
            if "QMP" not in json.loads(proc.stdout.readline()):
                raise ValueError("missing QMP greeting")
            request(Pipe, "qmp_capabilities")
            save(root / "cpus.json", request(Pipe, "query-cpus-fast"))
            while proc.poll() is None:
                uart = root / "uart.bin"
                raw = uart.read_bytes() if uart.exists() else b""
                end_marker = (rb"(?m)^NATIVE SMP END [0-9 ]+\r?\n" if manifest["policy"] != "fifo"
                              else rb"(?m)^NATIVE EL0 EXIT PID 1 -?\d+\r?\n")
                if manifest["policy"] in ("smp-global", "smp-clustered"):
                    end_marker = rb"(?m)^NATIVE MIG END [0-9 ]+\r?\n"
                if re.search(end_marker, raw):
                    request(Pipe, "stop")
                    request(Pipe, "pmemsave", val=base, size=end-base, filename=str(root / "ram.bin"))
                    if observer is not None:
                        observer.stopped(root)
                    reason = "terminal"
                    break
                time.sleep(0.025)
        except (ValueError, OSError, RuntimeError) as exc:
            reason = str(exc)
        finally:
            timer.cancel()
            proc.kill()
            proc.wait()
            timer.join()
    save(root / "session.json", dict(reason=reason, files={
        f.name: sha(f) for f in sorted(root.iterdir()) if f.is_file()}))
    print(sha(root / "session.json"))
    return int(reason != "terminal")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--trace", action="store_true")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--nm", default="aarch64-linux-gnu-nm")
    parser.add_argument("--objdump", default="aarch64-linux-gnu-objdump")
    raise SystemExit(capture(parser.parse_args()))
