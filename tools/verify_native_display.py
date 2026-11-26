#!/usr/bin/env python3
"""Boot the native display demo and check RAMFB scanout and PL011 input (Linux)."""

import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import zlib

from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import request


WIDTH, HEIGHT = 320, 240
STABLE_SECONDS = 0.2
EXIT = re.compile(rb"(?m)^NATIVE EL0 EXIT (?:PID (\d+) )?(-?\d+)\r?\n")
PPM = re.compile(rb"P6\s+(\d+)\s+(\d+)\s+255\r?\n")

# The display contract uses 5x7 glyphs. Each value is a horizontal row, with
# the leftmost pixel at bit 4. These host expectations use no guest code/data.
FONT = {
    "0": "0e11131519110e", "1": "040c040404040e", "2": "0e11010204081f",
    "A": "0e11111f111111", "B": "1e11111e11111e", "D": "1e11111111111e",
    "E": "1f10101e10101f", "F": "1f10101e101010", "I": "0e04040404040e",
    "J": "0702020202120c", "L": "1010101010101f", "N": "11191915131311",
    "O": "0e11111111110e", "P": "1e11111e101010", "Q": "0e11111115120d",
    "R": "1e11111e141211", "S": "0f10100e01011e", "T": "1f040404040404",
    "U": "1111111111110e", "X": "11110a040a1111", "Y": "11110a04040404",
    "-": "0000001f000000",
}


def expected_rgb(after):
    rgb = bytearray(bytes.fromhex("101923") * WIDTH * HEIGHT)

    def rect(left, top, width, height, color):
        strip = bytes.fromhex(color) * width
        for y in range(top, top + height):
            start = 3 * (y * WIDTH + left)
            rgb[start:start + len(strip)] = strip

    def label(x, y, words, scale, color):
        for char in words:
            if char != " ":
                for row, mask in enumerate(bytes.fromhex(FONT[char])):
                    for col in range(5):
                        if mask & (16 >> col):
                            rect(x + col * scale, y + row * scale, scale, scale, color)
            x += 6 * scale

    accent = "9fe6ab" if after else "6ecce0"
    rect(16, 44, 288, 146, "23433d" if after else "203747")
    rect(16, 44, 6, 146, accent)
    rect(16, 204, 288, 2, "526471")
    label(16, 14, "J-NOX DISPLAY", 2, "e8eff1")
    label(42, 62, "02" if after else "01", 8, "e8eff1")
    label(42, 139, "AFTER" if after else "BEFORE", 3, accent)
    label(16, 216, "N NEXT   Q QUIT", 2, "acbdc5")
    return bytes(rgb)


def save_png(path, rgb):
    # Lossless view of the bytes actually returned by QMP.
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    rows = b"".join(b"\0" + rgb[start:start + 3 * WIDTH] for start in range(0, len(rgb), 3 * WIDTH))
    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def freeze(package, dest, enabled):
    dest.mkdir()
    shutil.copyfile(package / "manifest.json", dest / "manifest.json")
    manifest = json.loads((dest / "manifest.json").read_bytes())
    if (manifest["board"], manifest["app"]) != ("virt", "process"):
        raise ValueError("expected ordinary native virt processes")
    config = manifest.get("display_service")
    if enabled:
        if (not config or config["slot"] != 0 or manifest["policy"] != "fifo"
                or (config["width"], config["height"], config["format"]) != (WIDTH, HEIGHT, "XRGB8888")):
            raise ValueError("expected display-enabled FIFO package with slot 0 owning 320x240 XRGB8888")
    elif config is not None:
        raise ValueError("expected a package built without the display service")
    names = ["user.elf", "native_peer.elf"] if enabled else ["user.elf"]
    if len(manifest["slots"]) != len(names):
        raise ValueError("unexpected native process count")
    for pid, (name, slot) in enumerate(zip(names, manifest["slots"])):
        argv = ["/bin/app", "denied"] if pid else ["/bin/app"]
        if (slot["slot"], slot["argv"], slot["elf"]) != (pid, argv, name):
            raise ValueError(f"unexpected app or arguments in slot {pid}")
        shutil.copyfile(package / name, dest / name)
        if sha(dest / name) != slot["elf_sha256"] or slot["elf_sha256"] != manifest["user_elf_sha256"]:
            raise ValueError(f"slot {pid} differs from the pinned demo ELF")
    kernel = dest / "kernel.elf"
    shutil.copyfile(package / kernel.name, kernel)
    if sha(kernel) != manifest["kernel_elf_sha256"]:
        raise ValueError("kernel differs from the package manifest")
    kernel.chmod(0o444)
    return manifest


def receive(sock, log, raw):
    while True:
        try:
            block = sock.recv(65536)
        except BlockingIOError:
            return
        if not block:
            return
        log.write(block)
        raw.extend(block)
        if len(raw) > 1024 * 1024:
            raise ValueError("UART exceeded 1 MiB")


def observed_exits(raw):
    if b"NATIVE EL0 FAULT " in raw:
        raise ValueError("kernel reported a native EL0 fault; see uart.bin")
    return [(int(pid) if pid else None, int(status)) for pid, status in EXIT.findall(raw)]


def stable_frame(pipe, root, name, expected, tick):
    latest = root / "last-scanout.ppm"
    first = None
    while True:
        tick()
        request(pipe, "screendump", filename=str(latest), device="screen", head=0)
        data = latest.read_bytes()
        header = PPM.match(data)
        pixels = data[header.end():] if header else b""
        if header and tuple(map(int, header.groups())) == (WIDTH, HEIGHT) and pixels == expected:
            moment = time.monotonic()
            if first is None:
                (root / (name + ".ppm")).write_bytes(data)
                first = moment
            elif moment - first >= STABLE_SECONDS:
                latest.rename(root / (name + "-repeat.ppm"))
                save_png(root / (name + ".png"), pixels)
                return {"rgb_sha256": sha256(pixels).hexdigest(), "stable_seconds": moment - first}
        else:
            first = None
        time.sleep(0.04)


def boot(qemu, kernel, pinned_hash, root, mode, timeout):
    root.mkdir()
    expected = [(0, 0 if mode == "interaction" else 19), (1, 61)] if mode != "headless" else [(None, 38)]
    raw = bytearray()
    record = {"mode": mode, "kernel_sha256": pinned_hash, "frames": {}, "input": [], "pass": False}
    with tempfile.TemporaryDirectory(dir="/tmp") as temp, (root / "uart.bin").open("xb", buffering=0) as uart, (root / "stderr.txt").open("xb") as err:
        path = str(Path(temp) / "uart")
        cmd = [qemu, *FLAGS, "-S", "-qmp", "stdio", "-chardev",
               f"socket,id=pl011,path={path},server=on,wait=off", "-serial", "chardev:pl011"]
        if mode != "absent":
            cmd += ["-device", "ramfb,id=screen"]
        cmd += ["-kernel", str(kernel)]
        if sha(kernel) != pinned_hash:
            raise ValueError("frozen kernel changed before boot")
        save(root / "launch.json", {"argv": cmd, "kernel_sha256": pinned_hash, "timeout_seconds": timeout})
        proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, bufsize=0)
        started = time.monotonic()
        timer = threading.Timer(timeout, proc.kill)  # Includes QMP greeting and blocked QMP I/O.
        stage = "QMP greeting"
        timer.start()
        try:
            with socket.socket(socket.AF_UNIX) as serial:
                record["qmp_version"] = json.loads(proc.stdout.readline())["QMP"]["version"]

                class Pipe:
                    write = proc.stdin.write
                    readline = proc.stdout.readline

                pipe = Pipe()
                request(pipe, "qmp_capabilities")
                stage = "connect PL011"
                serial.connect(path)
                serial.setblocking(False)
                request(pipe, "cont")

                def tick():
                    receive(serial, uart, raw)
                    if time.monotonic() - started >= timeout or proc.poll() is not None:
                        raise RuntimeError(f"guest terminated or timed out during {stage}")
                    exits = observed_exits(raw)
                    if exits != expected[:len(exits)]:
                        raise ValueError(f"unexpected native exits {exits}, expected {expected}")
                    return exits

                if mode == "interaction":
                    for label, after, key in (("before", False, b"n"), ("after", True, b"q")):
                        stage = "wait for stable " + label + " scanout"
                        record["frames"][label] = stable_frame(pipe, root, label, expected_rgb(after), tick)
                        if tick():
                            raise ValueError("display process exited before input")
                        serial.sendall(key)
                        record["input"].append({"byte": key.decode(), "seconds": time.monotonic() - started})
                stage = "wait for native exits"
                while tick() != expected:
                    time.sleep(0.01)
                request(pipe, "stop")
                receive(serial, uart, raw)
                record["native_exits"] = observed_exits(raw)
                if record["native_exits"] != expected:
                    raise ValueError("unexpected trailing native exit")
                request(pipe, "quit")
                if proc.wait(timeout=2) != 0:
                    raise RuntimeError(f"QEMU exit {proc.returncode}")
                if sha(kernel) != pinned_hash:
                    raise ValueError("frozen kernel changed during boot")
                record["pass"] = True
        except Exception as exc:
            record["error"] = f"{stage}: {exc}"
            raise RuntimeError(f"{root.name}: {record['error']} (see {root})") from exc
        finally:
            timer.cancel()
            timer.join()
            if proc.poll() is None:
                proc.kill()
            proc.wait(timeout=3)
            proc.stdin.close()
            proc.stdout.close()
            record["qemu_status"] = proc.returncode
            record["elapsed_seconds"] = time.monotonic() - started
            record["hashes"] = {p.name: sha(p) for p in sorted(root.iterdir()) if p.is_file()}
            save(root / "result.json", record)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True, help="display package with the denied peer in slot 1")
    parser.add_argument("--headless-package", type=Path, help="single-process package of the same demo without --display-service")
    parser.add_argument("--out", type=Path, required=True, help="new capture directory")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timeout", type=int, default=30, help="wall-clock seconds per guest")
    args = parser.parse_args()
    if args.timeout < 1:
        parser.error("timeout must be positive")
    qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
    out = args.out.resolve()
    out.mkdir(parents=True)
    package = out / "display-package"
    display = freeze(args.package, package, True)
    if args.headless_package:
        headless = freeze(args.headless_package, out / "headless-package", False)
        if headless["user_elf_sha256"] != display["user_elf_sha256"]:
            raise ValueError("headless control must use the same demo ELF")
    save(out / "observer.json", {"qemu": qemu, "qemu_sha256": sha(Path(qemu)),
                                "verifier_sha256": sha(Path(__file__))})
    for mode in ("interaction", "absent"):
        boot(qemu, package / "kernel.elf", display["kernel_elf_sha256"], out / mode, mode, args.timeout)
    if args.headless_package:
        boot(qemu, out / "headless-package/kernel.elf", headless["kernel_elf_sha256"],
             out / "headless", "headless", args.timeout)
    print(f"PASS {out}")


if __name__ == "__main__":
    try:
        main()
    except (KeyError, OSError, ValueError, RuntimeError) as error:
        sys.exit(str(error))
