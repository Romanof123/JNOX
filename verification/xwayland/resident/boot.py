"""Observe the five-role X11 guest through QMP scanout, tablet input and terminal exits."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(REPO / "examples/native-x11"), str(REPO / "tools")]
from mosaic_fixture import GEOMETRY, expected_scene
from qemu_capture import request
from verify_native_display import PPM
from diagnostics import ROLES, console, executable_ranges

WIDTH, HEIGHT = 320, 240


def crop(ppm):
    head = PPM.match(ppm)
    if not head or tuple(map(int, head.groups())) != (WIDTH, HEIGHT):
        return None
    pixels = ppm[head.end():]
    if len(pixels) != WIDTH * HEIGHT * 3:
        raise ValueError("invalid RAMFB scanout length")
    # Rootful Xwayland is anchored at (16,24) by the compositor. Mosaic's
    # CreateWindow coordinates are relative to that X root.
    x, y, w, h = GEOMETRY
    x, y = x + 16, y + 24
    return b"".join(pixels[(row*WIDTH+x)*3:(row*WIDTH+x+w)*3] for row in range(y, y+h))


def motion(qmp, x, y):
    axes = (("x", (x*32767+318)//319), ("y", (y*32767+238)//239))
    request(qmp, "input-send-event", events=[{"type": "abs", "data": {"axis": axis, "value": value}}
                                             for axis, value in axes])


def button(qmp, down):
    request(qmp, "input-send-event", events=[{"type": "btn", "data": {"button": "left", "down": down}}])


def sample_stopped_pc(address, code_ranges):
    # Request exactly one architectural register on the one-vCPU guest.
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(2)
        sock.connect(str(address))
        with sock.makefile("rwb", buffering=0) as qmp:
            if "QMP" not in json.loads(qmp.readline()):
                return None
            request(qmp, "qmp_capabilities")
            request(qmp, "stop")
            if request(qmp, "query-status").get("status") != "paused":
                return None
            result = request(qmp, "human-monitor-command", **{"command-line": "p /x $pc"})
            match = re.fullmatch(r"\s*(?:\$[0-9]+\s*=\s*)?(0x[0-9a-fA-F]{1,16})\s*", result)
            if not match:
                return None
            pc = int(match.group(1), 16)
            candidates = [role for role, ranges in code_ranges.items()
                          if any(low <= pc < high for low, high in ranges)]
            if candidates and pc % 4 == 0:
                return {"pc": hex(pc), "candidate_images": candidates}
    return None


def observe(kernel, private, mode, timeout, receipt, identity, code_ranges, output_dir):
    auth = private.parent / "authority"
    address = private / "qmp"
    uart = private / "uart"
    frame = private / "frame.ppm"
    command = ["qemu-system-aarch64", "-machine", "virt,gic-version=2", "-cpu", "cortex-a72",
               "-accel", "tcg", "-smp", "1", "-m", "128M", "-nodefaults", "-no-reboot",
               "-display", "none", "-monitor", "none", "-serial", "stdio", "-S",
               "-device", "ramfb,id=screen", "-global", "virtio-mmio.force-legacy=false",
               "-device", "virtio-tablet-device", "-qmp", f"unix:{address},server=on,wait=off",
               "-fw_cfg", "name=opt/jnox/xauth-server,file=" + str(auth / "server.auth"),
               "-fw_cfg", "name=opt/jnox/xauth-client,file=" + str(auth / ("wrong.auth" if mode == "wrong" else "client.auth")),
               "-kernel", kernel]
    private.mkdir()
    receipt.update(case=mode, stage="starting", exits={}, frames=[], completed=False)
    with uart.open("xb") as output, (private/"stderr").open("xb") as err:
        proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=output, stderr=err)
        try:
            end = time.monotonic() + timeout
            def tick():
                if uart.stat().st_size > 1024*1024:
                    raise ValueError("guest console exceeded observation limit")
                exits = console(uart.read_bytes(), identity, receipt)
                diag = receipt["diagnostic"]
                if "kernel_fault" in diag:
                    raise ValueError("guest reported a native fault")
                messages = diag.get("server_messages", [])
                if "graphics-abort" in messages or ("fatal-server-error" in messages
                        and any(name != "fatal-server-error" and not name.startswith("xtrans-") for name in messages)):
                    raise ValueError("server reported fatal startup error")
                allowed = {0: 0, 1: 0, 2: 72, 3: 0, 4: 114 if mode == "wrong" else 0}
                if any(value != allowed[slot] for slot, value in exits.items()):
                    raise ValueError("unexpected guest exit")
                if proc.poll() is not None:
                    raise ValueError("QEMU exited before the observation completed")
                if time.monotonic() >= end:
                    raise TimeoutError("guest observation expired")
                return exits

            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(3)
                while True:
                    tick()
                    try:
                        sock.connect(str(address))
                        break
                    except (FileNotFoundError, ConnectionRefusedError):
                        time.sleep(.05)
                with sock.makefile("rwb", buffering=0) as qmp:
                    if "QMP" not in json.loads(qmp.readline()):
                        raise ValueError("invalid QMP greeting")
                    request(qmp, "qmp_capabilities")
                    request(qmp, "cont")
                    if mode == "wrong":
                        receipt["stage"] = "waiting for Mosaic SetupFailed"
                        while 4 not in tick():
                            time.sleep(.1)
                    else:
                        if not any(m["current"] and m["absolute"] for m in request(qmp, "query-mice")):
                            raise ValueError("virtio tablet unavailable")
                        motion(qmp, 2, 2)
                        for changed in (False, True):
                            label = "blue" if changed else "coral"
                            receipt["stage"] = "waiting for " + label + " Mosaic pixels"
                            expected = expected_scene(changed)
                            first = None
                            switch = time.monotonic() + 5
                            while True:
                                if 4 in tick():
                                    raise ValueError("Mosaic exited before its frame was observed")
                                try:
                                    request(qmp, "screendump", filename=str(frame), device="screen", head=0)
                                except RuntimeError:
                                    # The guest programs ramfb after reset; no surface exists yet.
                                    # tick() still enforces the original deadline and exit checks.
                                    first = None
                                    time.sleep(.1)
                                    continue
                                now = time.monotonic()
                                pixels = crop(frame.read_bytes())
                                if pixels == expected:
                                    if first is None:
                                        first = now
                                    if now-first >= .2:
                                        _, _, width, height = GEOMETRY
                                        capture = f"P6\n{width} {height}\n255\n".encode() + pixels
                                        name = f"mosaic-{label}.ppm"
                                        (output_dir / name).write_bytes(capture)
                                        receipt["frames"].append({"color": label, "origin": [48,56],
                                                                  "matched_pixels": width * height,
                                                                  "capture": name,
                                                                  "sha256": hashlib.sha256(capture).hexdigest()})
                                        break
                                else:
                                    first = None
                                    # A native surface may initially be in front. 'n' uses the
                                    # compositor's public focus switch, without an X11 click.
                                    if not changed and now >= switch:
                                        proc.stdin.write(b"n")
                                        proc.stdin.flush()
                                        switch = now + 5
                                time.sleep(.1)
                            motion(qmp, 70, 75)
                            button(qmp, True)
                            if not changed:
                                # Mosaic draws on ButtonPress. Hold position and button until its
                                # changed lower rows are on scanout, clear of the 64x64 cursor.
                                # This synchronizes release only. The next stage still requires
                                # two exact full crops with the pointer outside the window.
                                receipt["stage"] = "waiting for first ButtonPress redraw"
                                tail = GEOMETRY[2] * 3 * 64
                                blue_tail = expected_scene(True)[-tail:]
                                assert blue_tail != expected[-tail:]
                                while True:
                                    if 4 in tick():
                                        raise ValueError("Mosaic exited before its first button redraw")
                                    request(qmp, "screendump", filename=str(frame), device="screen", head=0)
                                    pressed = crop(frame.read_bytes())
                                    if pressed and pressed[-tail:] == blue_tail:
                                        break
                                    time.sleep(.1)
                                button(qmp, False)
                                motion(qmp, 2, 2)
                        receipt["stage"] = "waiting for Mosaic DestroyNotify and exit"
                        while 4 not in tick():
                            time.sleep(.1)
                        button(qmp, False)
                        receipt["stage"] = "waiting for Xwayland last-client termination"
                        while 1 not in tick():
                            time.sleep(.1)
                        receipt["stage"] = "closing the native session"
                        proc.stdin.write(b"q")
                        proc.stdin.flush()
                        while True:
                            tick()
                            if len(receipt["terminal_exits"]) == 5:
                                break
                            time.sleep(.1)
            receipt.update(stage="complete", completed=True)
        except (OSError, ValueError, RuntimeError, TimeoutError):
            if proc.poll() is None:
                try:
                    sample = sample_stopped_pc(address, code_ranges)
                    if sample:
                        receipt.setdefault("diagnostic", {})["stopped_vcpu"] = sample
                except (OSError, ValueError, RuntimeError):
                    pass
                # A fatal heading can arrive before its complete explanation.
                # Refresh after the stop has synchronized the guest output.
                if uart.stat().st_size <= 1024*1024:
                    console(uart.read_bytes(), identity, receipt)
            raise
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
            proc.stdin.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("workspace", type=Path, help="output of package.py")
    parser.add_argument("out", type=Path, help="fresh public-status directory")
    parser.add_argument("lock", type=Path)
    parser.add_argument("--timeout", type=int, default=120, help="host seconds per cold boot")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    os.umask(0o077)
    work = args.workspace.resolve()
    identity = json.loads((work / "workload.json").read_text())
    kernel = work / "package/kernel.elf"
    if hashlib.sha256(kernel.read_bytes()).hexdigest() != identity["kernel_sha256"]:
        parser.error("packaged kernel changed")
    manifest = json.loads((work / "package/manifest.json").read_text())
    code_ranges = {"kernel": executable_ranges(kernel)}
    if tuple(identity["role_order"]) != ROLES or len(manifest["slots"]) != len(ROLES):
        parser.error("unexpected package roles")
    for pid, (role, slot) in enumerate(zip(ROLES, manifest["slots"])):
        path = work / "package" / slot["elf"]
        if slot["slot"] != pid or hashlib.sha256(path.read_bytes()).hexdigest() != identity["elf_sha256"][role]:
            parser.error("packaged role changed")
        code_ranges[role] = executable_ranges(path)
    args.out.mkdir()
    good = True
    with args.lock.open("a") as lock, tempfile.TemporaryDirectory() as temp:
        fcntl.flock(lock, fcntl.LOCK_EX)
        private = Path(temp)
        subprocess.run([sys.executable, REPO / "examples/native-x11/local_profile.py", "prepare",
                        private / "authority", "--node", "guest-x11"], check=True)
        for mode in ("wrong", "matching"):
            receipt = {"kernel_sha256": identity["kernel_sha256"]}
            try:
                observe(kernel, private / mode, mode, args.timeout, receipt, identity, code_ranges, args.out)
            except (OSError, ValueError, RuntimeError, TimeoutError) as error:
                # Raw UART, QEMU logs and authority files stay inside the private
                # workspace. The public JSON retains allowlisted fields only.
                receipt["error_type"] = type(error).__name__
                good = False
            (args.out / (mode + ".json")).write_text(json.dumps(receipt, indent=2) + "\n")
            print(json.dumps(receipt), flush=True)
    return 0 if good else 1


if __name__ == "__main__":
    raise SystemExit(main())
