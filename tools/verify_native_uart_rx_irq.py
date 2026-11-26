#!/usr/bin/env python3
"""Cold-boot or replay the native PL011 RX interrupt and polled controls (Linux)."""
import argparse
import json
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace

from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import request
from verify_native_display import freeze, observed_exits, PPM, receive

IRQ = {"device": "pl011", "controller": "gicv2", "intid": 33, "capacity": 16, "stats_syscall": 75}
SHORT = bytes([0x00, 0xfe, 0x11, 0x80, 0x7f, 0x55, 0x0d, 0xa3])
BURST = bytes(range(0x61, 0x79))
INPUT = {"irq": [(1, SHORT), (2, BURST), (3, b"q")], "polled": [(4, b"p")], "headless": []}
EXITS = {"irq": [(0, 70), (1, 71)], "polled": [(0, 73), (1, 72)], "headless": [(None, 38)]}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def read_json(path):
    return json.loads(path.read_bytes())


def scanline(frame, stage):
    header = PPM.match(frame)
    if not header or header.groups() != (b"320", b"240"):
        return None
    pixels = frame[header.end():]
    require(len(pixels) == 320 * 240 * 3, "incomplete QMP scanout")
    if pixels[:3] != bytes([0x49, 0x52, stage]):
        return None
    words = [int.from_bytes(pixels[i:i+3], "big") for i in range(3, 3*320, 3)]
    if stage in (1, 4):
        require(not any(words), "unexpected initial telemetry")
        return {}
    irqs, count, drops, errors, depth, capacity, idle, elapsed = words[:8]
    expected = SHORT if stage == 2 else BURST[:16]
    require(0 < idle <= irqs and elapsed >= 100, "missing idle UART interrupt or timer wake")
    require((count, drops, errors, depth, capacity) ==
            (8 if stage == 2 else 32, 0 if stage == 2 else 8, 0, len(expected), 16),
            f"stage {stage}: incorrect hardware byte, ring or error counts {words[:8]}")
    require(words[8:8+depth] == list(expected) and not any(words[8+depth:]), "dequeued payload differs")
    return {"irqs": irqs, "valid_bytes": count, "drops": drops, "error_words": errors,
            "depth_before_read": depth, "capacity": capacity, "idle_irqs": idle,
            "elapsed_ms": elapsed, "payload_hex": expected.hex()}


def packet(root, mode):
    p = root / "package"
    m = read_json(p / "manifest.json")
    require((m["board"], m["policy"], m["app"]) == ("virt", "fifo", "process")
            and not m["tracked_changes"], "expected clean virt FIFO native package")
    require(m.get("uart_rx_irq") == (IRQ if mode == "irq" else None), "incorrect IRQ mode")
    owner = m.get("display_service")
    require((owner is None) if mode == "headless" else owner and owner["slot"] == 0, "incorrect display owner")
    slots = m["slots"]
    require(len(slots) == (1 if mode == "headless" else 2), "incorrect number of process slots")
    kernel = (p / "kernel.elf").read_bytes()
    require(sha(p / "kernel.elf") == m["kernel_elf_sha256"], "kernel differs from manifest")
    require(sha(p / "build.log") == m["build_log_sha256"], "kernel build log differs from manifest")
    for i, s in enumerate(slots):
        elf = p / ("native_peer.elf" if i else "user.elf")
        require((s["slot"], s["argv"], s["elf"]) ==
                (i, ["/bin/app", "denied"] if i else ["/bin/app"], elf.name), "incorrect executable/arguments")
        require(sha(elf) == s["elf_sha256"] == m["user_elf_sha256"] and elf.read_bytes() in kernel,
                "slot image differs from frozen kernel or manifest")
    return m


def inspect(root, mode):
    m = packet(root, mode)
    raw = (root / "uart.bin").read_bytes()
    prefix = b"JNOX M1 BOOT\nJNOX M1 READY\n"
    for i, slot in enumerate(m["slots"]):
        label = b"PEER ELF " if i else b"ELF "
        prefix += b"NATIVE EL0 " + label + slot["elf_sha256"].encode() + b"\n"
    require(raw.startswith(prefix) and observed_exits(raw) == EXITS[mode], "UART boot, image identity or exits differ")
    require((root / "input.bin").read_bytes() == b"".join(payload for _, payload in INPUT[mode]), "input differs")
    stages = {}
    for stage, _ in INPUT[mode]:
        values = scanline((root / f"stage-{stage}.ppm").read_bytes(), stage)
        require(values is not None, "missing stage marker")
        stages[str(stage)] = values
    if mode == "irq":
        require(stages["3"]["irqs"] > stages["2"]["irqs"], "no new UART IRQ during overflow test")
    return {"stages": stages, "exits": [list(pair) for pair in EXITS[mode]]}


def boot(out, mode, qemu, timeout):
    m = packet(out, mode)
    kernel = out / "package" / "kernel.elf"
    raw = bytearray()
    with tempfile.TemporaryDirectory(dir="/tmp") as tmp, (out/"uart.bin").open("xb", buffering=0) as uart, \
            (out/"input.bin").open("xb", buffering=0) as sent, (out/"stderr.txt").open("xb") as err:
        path = str(Path(tmp)/"pl011")
        args = [qemu, *FLAGS, "-S", "-qmp", "stdio", "-chardev",
                f"socket,id=pl011,path={path},server=on,wait=off", "-serial", "chardev:pl011"]
        if mode != "headless":
            args += ["-device", "ramfb,id=screen"]
        args += ["-kernel", str(kernel)]
        save(out/"launch.json", {"argv": args, "qemu_sha256": sha(Path(qemu)), "timeout_s": timeout,
                                 "kernel_sha256": m["kernel_elf_sha256"]})
        proc = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, bufsize=0)
        timer = threading.Timer(timeout, proc.kill)
        timer.start()
        try:
            with socket.socket(socket.AF_UNIX) as serial:
                greeting = json.loads(proc.stdout.readline())["QMP"]
                save(out/"qmp.json", greeting)
                pipe = SimpleNamespace(write=proc.stdin.write, readline=proc.stdout.readline)
                request(pipe, "qmp_capabilities")
                serial.connect(path)
                serial.setblocking(False)
                request(pipe, "cont")

                def tick():
                    receive(serial, uart, raw)
                    require(proc.poll() is None, "guest exited or timed out")
                    seen = observed_exits(raw)
                    require(seen == EXITS[mode][:len(seen)], f"unexpected guest exit {seen}")
                    return seen

                for stage, payload in INPUT[mode]:
                    latest = out/"last.ppm"
                    while True:
                        require(not tick(), f"guest exited before input at stage {stage}")
                        request(pipe, "screendump", filename=str(latest), device="screen", head=0)
                        if scanline(latest.read_bytes(), stage) is not None:
                            latest.rename(out/f"stage-{stage}.ppm")
                            break
                        time.sleep(0.015)
                    # The software FIFO deliberately remains unread during the burst.
                    # Pace chunks to keep host chardev buffering separate from ring loss.
                    for start in range(0, len(payload), 4):
                        chunk = payload[start:start+4]
                        serial.sendall(chunk)
                        sent.write(chunk)
                        time.sleep(0.005)
                while tick() != EXITS[mode]:
                    time.sleep(0.015)
                request(pipe, "stop")
                receive(serial, uart, raw)
                request(pipe, "quit")
                require(proc.wait(timeout=3) == 0, "QEMU quit failed")
        finally:
            timer.cancel()
            timer.join()
            if proc.poll() is None:
                proc.kill()
            proc.wait(timeout=3)
            proc.stdin.close()
            proc.stdout.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--package", type=Path)
    ap.add_argument("--polled-package", type=Path)
    ap.add_argument("--headless-package", type=Path)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--replay", type=Path, help="verify frozen packages, UART, input bytes and QMP scanouts offline")
    ap.add_argument("--qemu", default="qemu-system-aarch64")
    ap.add_argument("--timeout", type=int, default=25)
    a = ap.parse_args()
    if a.replay:
        modes = read_json(a.replay/"result.json")["modes"]
        require(list(modes) in (["irq", "polled"], ["irq", "polled", "headless"]), "missing or unexpected controls")
        for mode, recorded in modes.items():
            root = a.replay/mode
            require(inspect(root, mode) == recorded["observed"], f"{mode} observations changed")
            for name, digest in recorded["sha256"].items():
                require(sha(root/name) == digest, f"{mode}/{name} hash changed")
        print("PASS replay:", ", ".join(modes))
        return
    if not (a.package and a.polled_package and a.out and a.timeout > 0):
        ap.error("boot requires --package, --polled-package, --out and a positive timeout")
    out = a.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    qemu = str(Path(shutil.which(a.qemu) or a.qemu).resolve(strict=True))
    modes = {}
    baseline = None
    for mode, source in (("irq", a.package), ("polled", a.polled_package), ("headless", a.headless_package)):
        if not source:
            continue
        root = out/mode
        root.mkdir()
        m = freeze(source, root/"package", mode != "headless")
        shutil.copyfile(source/"build.log", root/"package"/"build.log")
        identity = (m["commit"], m["user_elf_sha256"])
        if baseline is not None:
            require(baseline == identity, "control differs from IRQ application or source commit")
        baseline = identity
        try:
            boot(root, mode, qemu, a.timeout)
            observed = inspect(root, mode)
        except Exception as exc:
            save(root/"failure.json", {"error": str(exc)})
            raise
        modes[mode] = {"observed": observed, "sha256": {
            p.relative_to(root).as_posix(): sha(p) for p in sorted(root.rglob("*")) if p.is_file()}}
    save(out/"result.json", {"modes": modes, "source_commit": baseline[0], "app_sha256": baseline[1],
                             "verifier_sha256": sha(Path(__file__))})
    print("PASS:", ", ".join(modes))


if __name__ == "__main__":
    try:
        main()
    except (OSError, KeyError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
