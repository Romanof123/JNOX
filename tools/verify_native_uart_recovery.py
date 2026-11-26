#!/usr/bin/env python3
"""Observe one-shot JNOX UART driver recovery and normal exit on separate cold boots."""
import argparse
import json
from pathlib import Path
import secrets
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace

from native_amc_roots import linked_roots
from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import journal_location, request
from verify_native_display import PPM, receive, observed_exits
from verify_native_uart_rx_irq import IRQ, require, read_json


STANDBY = {"owner_slot": 0, "continuing_peer_slot": 1, "standby_slot": 2,
           "failure_exit": 86, "max_handoffs": 1}
MODES = ("failure", "normal")
USER_ELFS = ("user.elf", "native_peer.elf", "native_peer2.elf")
FILES = (*USER_ELFS, "kernel.elf", "manifest.json", "build.log")
ASID_MASK = 0x0000fffffffff000
IMSC = 0x09000038
GIC_ENABLE = 0x08000104  # Distributor ISENABLER1, INTID 33 = bit 1
STALE = b"old!"
MARKER = 0x50420000


def profile(pkg):
    m = read_json(pkg / "manifest.json")
    require((m["board"], m["policy"], m["app"]) == ("virt", "fifo", "process") and not m["tracked_changes"]
            and m["uart_rx_irq"] == IRQ and m["uart_rx_standby"] == STANDBY
            and m["display_service"]["slot"] == 0 and len(m["slots"]) == 3, "standby package mismatch")
    kernel = pkg / "kernel.elf"
    require(sha(kernel) == m["kernel_elf_sha256"] and sha(pkg / "build.log") == m["build_log_sha256"],
            "kernel or build log differs")
    linked = linked_roots(kernel, 3)
    kernel_bytes = kernel.read_bytes()
    for i, args in enumerate(([], ["peer"], ["standby"])):
        slot = m["slots"][i]
        elf = pkg / USER_ELFS[i]
        require((slot["slot"], slot["elf"], slot["argv"], slot["priority"], slot["expected_ttbr0_el1"]) ==
                (i, elf.name, ["/bin/app", *args], i+1, linked[i]), "slot configuration differs")
        require(sha(elf) == slot["elf_sha256"] == m["user_elf_sha256"] and elf.read_bytes() in kernel_bytes,
                "application identity differs")
    va, size = journal_location(pkg / USER_ELFS[0], "jnox_recovery_probe")
    require(0x400000 <= va < 0x800000 and size == 8 and va % 4 == 0 and va % 4096 <= 4088,
            "invalid private probe")
    return m, linked, va


def walk(root, va, read):
    table = root & ASID_MASK
    for shift in (30, 21, 12):
        address = table + ((va >> shift) & 511)*8
        require(0x40000000 <= address < 0x48000000, "page table outside guest RAM")
        entry = int.from_bytes(read(address, 8), "little")
        require(entry & 3 == 3, "missing private mapping")
        table = entry & ASID_MASK
    # AP[1] permits user access; AP[2] clear permits writes.
    require((entry >> 6) & 3 == 1 and 0x40000000 <= table < 0x48000000,
            "probe is not ordinary writable private RAM")
    return table + (va & 4095)


def read_addresses(walks, roots, va):
    def saved(address, count):
        raw = bytes.fromhex(walks[str(address)])
        require(len(raw) == count, "truncated table entry")
        return raw
    addresses = [walk(root, va, saved) for root in roots]
    require(len(set(addresses)) == 3, "aliased process probes")
    return addresses


def frame_data(buf, stage):
    header = PPM.match(buf)
    if not header or header.groups() != (b"320", b"240"):
        return None
    pix = buf[header.end():]
    require(len(pix) == 320*240*3, "partial QMP scanout")
    if pix[:3] != bytes((0x52, 0x43, stage)):
        return None
    return [int.from_bytes(pix[i:i+3], "big") for i in range(3, 3*320, 3)]


def prefix(mode):
    return (b"f" if mode == "failure" else b"n") + STALE


def boot(root, pkg, roots, va, mode, qemu, timeout):
    raw = bytearray()
    samples = {}
    nonce = b"\xe7" + secrets.token_bytes(7)
    with tempfile.TemporaryDirectory(dir="/tmp") as tmp, (root/"uart.bin").open("xb", buffering=0) as uart, \
            (root/"input.bin").open("xb", buffering=0) as sent, (root/"stderr.txt").open("xb") as stderr:
        temp = Path(tmp)
        sockpath = str(temp / "pl011")
        argv = [qemu, *FLAGS, "-S", "-qmp", "stdio", "-chardev",
                f"socket,id=pl011,path={sockpath},server=on,wait=off", "-serial", "chardev:pl011",
                "-device", "ramfb,id=screen", "-kernel", str(pkg/"kernel.elf")]
        save(root/"launch.json", {"argv": argv, "qemu_sha256": sha(Path(qemu)), "timeout_s": timeout})
        proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, bufsize=0)
        watchdog = threading.Timer(timeout, proc.kill)
        watchdog.start()
        try:
            with socket.socket(socket.AF_UNIX) as ser:
                save(root/"qmp.json", json.loads(proc.stdout.readline())["QMP"])
                pipe = SimpleNamespace(write=proc.stdin.write, readline=proc.stdout.readline)
                request(pipe, "qmp_capabilities")
                ser.connect(sockpath)
                ser.setblocking(False)
                request(pipe, "cont")

                def tick():
                    receive(ser, uart, raw)
                    require(proc.poll() is None, "guest timed out or exited")
                    return observed_exits(raw)

                def frame(stage):
                    p = temp/"latest.ppm"
                    while True:
                        require(not tick(), f"guest finished before screen stage {stage}")
                        request(pipe, "screendump", filename=str(p), device="screen", head=0)
                        data = p.read_bytes()
                        if frame_data(data, stage) is not None:
                            (root/f"stage-{stage}.ppm").write_bytes(data)
                            return
                        time.sleep(0.012)

                def mem(address, length):
                    p = temp/"read.bin"
                    request(pipe, "pmemsave", val=address, size=length, filename=str(p))
                    data = p.read_bytes()
                    require(len(data) == length, "incomplete physical read")
                    return data

                frame(1)
                tables = {}
                def capture_pte(address, count):
                    raw_pte = mem(address, count)
                    tables[str(address)] = raw_pte.hex()
                    return raw_pte
                for root_address in roots:
                    walk(root_address, va, capture_pte)
                addresses = read_addresses(tables, roots, va)
                save(root/"mappings.json", tables)

                def wait_sample(name, predicate):
                    while True:
                        tick()
                        values = [struct.unpack("<II", mem(pa, 8)) for pa in addresses]
                        gic, uart_mask = (int.from_bytes(mem(addr, 4), "little") for addr in (GIC_ENABLE, IMSC))
                        state = {"probes": values, "gic_enable1": gic, "uart_imsc": uart_mask}
                        if predicate(state):
                            samples[name] = state
                            return
                        time.sleep(0.01)

                def send(payload):
                    ser.sendall(payload)
                    sent.write(payload)

                wait_sample("primary", lambda s: sample_matches("primary", s, mode))
                send(prefix(mode))
                frame(2)
                send(b"+")
                if mode == "failure":
                    wait_sample("transfer", lambda s: sample_matches("transfer", s, mode))
                    frame(3)
                    wait_sample("replacement", lambda s: sample_matches("replacement", s, mode))
                    send(nonce)
                    frame(4)
                    send(b"q")
                wait_sample("revoked", lambda s: sample_matches("revoked", s, mode)
                            and s["probes"][1][1] > samples["primary"]["probes"][1][1])
                required = exit_list(mode)
                while tick() != required:
                    if observed_exits(raw):
                        require(observed_exits(raw) == required[:len(observed_exits(raw))], "wrong guest exits")
                    time.sleep(0.01)
                request(pipe, "stop")
                receive(ser, uart, raw)
                wait_sample("final", lambda s: sample_matches("final", s, mode))
                request(pipe, "quit")
                require(proc.wait(timeout=3) == 0, "QEMU quit failed")
        finally:
            save(root/"samples.json", samples)
            watchdog.cancel()
            watchdog.join()
            if proc.poll() is None:
                proc.kill()
            proc.wait(timeout=3)
            proc.stdin.close()
            proc.stdout.close()


def exit_list(mode):
    return [(0, 86), (1, 71), (2, 86)] if mode == "failure" else [(0, 0), (1, 71)]


def sample_matches(name, state, mode):
    a, p, b = state["probes"]
    stage = lambda slot: slot[0] - MARKER
    peer_tick = p[1]
    masks = state["uart_imsc"] & 0x50, state["gic_enable1"] & 2
    enabled = masks == (0x50, 2)
    masked = masks == (0, 0)
    spare = 22 if mode == "failure" else 0
    if name == "primary":
        return stage(a) == 1 and stage(p) == 10 and 0 < peer_tick < 50 and stage(b) == 0 and enabled
    if name == "transfer":
        return stage(a) == 2 and stage(b) == 20 and stage(p) == 10 and masked
    if name == "replacement":
        return stage(a) == 2 and stage(b) == 21 and stage(p) == 10 and enabled
    if name == "revoked":
        return stage(a) == 2 and stage(b) == spare and stage(p) == 10 and masked
    if name == "final":
        return stage(a) == 2 and stage(b) == spare and stage(p) == 11 and peer_tick == 50 and masked
    return False


def inspect(root, m, roots, va, mode):
    require(len(set(read_addresses(read_json(root/"mappings.json"), roots, va))) == 3, "probe mappings")
    sent = (root/"input.bin").read_bytes()
    require(sent[:6] == prefix(mode) + b"+", "unexpected primary serial input")
    nonce = sent[6:-1]
    require((len(nonce) == 8 and nonce[0] == 0xe7 and sent[-1:] == b"q") if mode == "failure"
            else len(sent) == 6, "unexpected replacement serial input")
    frames = {1: [], 2: [sent[0], 4, 5]}
    if mode == "failure":
        frames.update({3: [0, 16], 4: [8, *nonce]})
    for stage, expect in frames.items():
        words = frame_data((root/f"stage-{stage}.ppm").read_bytes(), stage)
        require(words is not None and words == expect + [0]*(319-len(expect)), f"wrong screen stage {stage}")
    raw = (root/"uart.bin").read_bytes()
    header = b"JNOX M1 BOOT\nJNOX M1 READY\n" + b"".join(
        b"NATIVE EL0 " + label + b" " + s["elf_sha256"].encode()+b"\n"
        for label, s in zip((b"ELF", b"PEER ELF", b"PEER2 ELF"), m["slots"]))
    require(raw.startswith(header) and observed_exits(raw) == exit_list(mode), "unexpected UART identities/exits")
    dismissal = b"NATIVE EL0 DISMISSED PID 2" in raw.splitlines()
    require(dismissal == (mode == "normal"), "standby dismissal mismatch")
    states = read_json(root/"samples.json")
    sequence = ["primary", "transfer", "replacement", "revoked", "final"] if mode == "failure" else [
        "primary", "revoked", "final"]
    require(list(states) == sequence and all(sample_matches(name, states[name], mode) for name in sequence),
            "incorrect device or private process state")
    ticks = [states[name]["probes"][1][1] for name in sequence]
    require(ticks == sorted(ticks) and ticks[0] < ticks[-2] < ticks[-1], "peer failed to progress across revocation")
    if mode == "failure":
        require(ticks[1] < ticks[2], "peer did not run during replacement startup")
    return {"exits": exit_list(mode), "peer_ticks": ticks, "standby_dismissed": dismissal,
            "fresh_nonce_hex": nonce.hex(), "private_probe_addresses": read_addresses(
                read_json(root/"mappings.json"), roots, va)}


def digests(directory):
    return {p.relative_to(directory).as_posix(): sha(p) for p in sorted(directory.rglob("*")) if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--replay", type=Path)
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timeout", type=int, default=20)
    args = parser.parse_args()
    if args.replay:
        root = args.replay
        result = read_json(root/"result.json")
        m, roots, va = profile(root/"package")
        require(result["source_commit"] == m["commit"] and result["app_sha256"] == m["user_elf_sha256"]
                and result["kernel_sha256"] == m["kernel_elf_sha256"], "package identity changed")
        for mode in MODES:
            observed = inspect(root/mode, m, roots, va, mode)
            require(json.loads(json.dumps(observed)) == result[mode]["observed"] and
                    digests(root/mode) == result[mode]["sha256"], "frozen observations changed")
        print("PASS replay: reported driver failure, standby exhausted, normal exit")
        return
    if not (args.package and args.out and args.timeout > 0):
        parser.error("specify --package and --out or --replay")
    root = args.out.resolve()
    root.mkdir(parents=True, exist_ok=False)
    pkg = root/"package"
    pkg.mkdir()
    for filename in FILES:
        shutil.copyfile(args.package/filename, pkg/filename)
    m, roots, va = profile(pkg)
    result = {"source_commit": m["commit"], "kernel_sha256": m["kernel_elf_sha256"],
              "app_sha256": m["user_elf_sha256"], "observer_sha256": sha(Path(__file__))}
    qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
    for mode in MODES:
        capture = root/mode
        capture.mkdir()
        try:
            boot(capture, pkg, roots, va, mode, qemu, args.timeout)
            result[mode] = {"observed": inspect(capture, m, roots, va, mode), "sha256": digests(capture)}
        except Exception as exc:
            save(capture/"failure.json", {"error": str(exc)})
            raise
    save(root/"result.json", result)
    print("PASS: reported driver failure, standby exhausted, normal exit")


if __name__ == "__main__":
    try:
        main()
    except (OSError, KeyError, ValueError, RuntimeError, subprocess.SubprocessError) as e:
        sys.exit(str(e))
