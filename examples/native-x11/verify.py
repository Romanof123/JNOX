#!/usr/bin/env python3
"""Boot the ordinary JNOX client against private Xvfb and observe its pixels and input."""
import argparse
from contextlib import contextmanager, closing, ExitStack
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import select
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import zlib

from Xlib import X, Xatom, display
from Xlib.ext import xtest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from qemu_capture import request
from verify_native_display import stable_frame

TITLE = "JNOX X11 Mosaic"
GEOMETRY = (32, 32, 240, 160)
LOCAL_ORIGIN = (40, 40)
AUTH_NAME = b"MIT-MAGIC-COOKIE-1"
EXIT = rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?\n"


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def auth_record(family, host, number, cookie):
    fields = (host, number, AUTH_NAME, cookie)
    return struct.pack(">H", family) + b"".join(struct.pack(">H", len(f)) + f for f in fields)


@contextmanager
def process(argv, log, **kwargs):
    with log.open("xb") as stream:
        child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=stream, stderr=stream, **kwargs)
        try:
            yield child
        finally:
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()


def start_display(stack, tmp, out, cookie):
    authority = tmp / "server.auth"
    authority.write_bytes(auth_record(65535, b"", b"", cookie))
    read_fd, write_fd = os.pipe()
    try:
        try:
            stack.enter_context(process([
                "Xvfb", "-displayfd", str(write_fd), "-nolisten", "tcp", "-noreset",
                "-screen", "0", "320x240x24", "-auth", str(authority), "+extension", "XTEST",
            ], out / "xvfb.log", pass_fds=(write_fd,)))
        finally:
            os.close(write_fd)
        require(select.select([read_fd], [], [], 10)[0], "Xvfb startup timed out")
        number = os.read(read_fd, 64).strip()
        require(number.isdigit(), "Xvfb did not provide a display number; see xvfb.log")
    finally:
        os.close(read_fd)
    unix_path = f"/tmp/.X11-unix/X{int(number)}"
    # The observer uses a separate local X connection and the guest goes through the relay.
    client_auth = tmp / "observer.auth"
    client_auth.write_bytes(auth_record(256, socket.gethostname().encode(), number, cookie))
    os.environ["XAUTHORITY"] = str(client_auth)
    observer = stack.enter_context(closing(display.Display(":" + number.decode())))
    require(observer.has_extension("XTEST"), "XTEST unavailable")
    return observer, unix_path


def check_auth(unix_path):
    # Check the real server's setup response on a connection with an invalid cookie.
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as check:
        check.settimeout(3)
        check.connect(unix_path)
        auth = AUTH_NAME + b"\0" * (-len(AUTH_NAME) % 4)
        check.sendall(struct.pack("<BBHHHHH", ord("l"), 0, 11, 0, len(AUTH_NAME), 16, 0)
                      + auth + bytes(16))
        require(check.recv(1) == b"\0", "Xvfb failed to reject invalid authorization")


class GetImageObserver:
    """Record only GetImage metadata from the guest's little-endian X11 request stream."""
    def __init__(self, requests):
        self.requests = requests
        self.pending = bytearray()
        self.setup = True

    def feed(self, data):
        self.pending.extend(data)
        if self.setup:
            if len(self.pending) < 12:
                return
            require(self.pending[0] == ord("l"), "unexpected X11 request byte order")
            name, secret = struct.unpack_from("<HH", self.pending, 6)
            size = 12 + ((name + 3) & ~3) + ((secret + 3) & ~3)
            if len(self.pending) < size:
                return
            del self.pending[:size]
            self.setup = False
        while len(self.pending) >= 4:
            size = struct.unpack_from("<H", self.pending, 2)[0] * 4
            require(size >= 4, "unexpected extended X11 request")
            if len(self.pending) < size:
                return
            if self.pending[0] == 73:
                require(size == 20, "invalid GetImage request size")
                drawable, x, y, w, h, mask = struct.unpack_from("<IhhHHI", self.pending, 4)
                self.requests.append(dict(format=self.pending[1], drawable=drawable, x=x, y=y,
                                          width=w, height=h, plane_mask=mask))
            del self.pending[:size]


@contextmanager
def relay(listener, unix_path, stats, scanout):
    done = threading.Event()
    observer = GetImageObserver(stats.setdefault("get_image", [])) if scanout else None

    def forward():
        try:
            while not done.is_set():
                if select.select([listener], [], [], 0.1)[0]:
                    break
            else:
                return
            guest, address = listener.accept()
            with guest, socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
                server.connect(unix_path)
                guest.settimeout(3)
                server.settimeout(3)
                stats["source"] = list(address)
                destinations = {guest: (server, "to_server"), server: (guest, "to_guest")}
                while destinations and not done.is_set():
                    for src in select.select(list(destinations), [], [], 0.1)[0]:
                        dst, key = destinations[src]
                        data = src.recv(32768)
                        if data:
                            if src is guest and observer is not None:
                                observer.feed(data)
                            dst.sendall(data)
                            stats[key] += len(data)
                        else:
                            dst.shutdown(socket.SHUT_WR)
                            del destinations[src]
        except (OSError, RuntimeError) as exc:
            stats["error"] = str(exc)

    worker = threading.Thread(target=forward)
    worker.start()
    try:
        yield
    finally:
        done.set()
        worker.join()


def package(client, broker, tmp, out, port, cookie, scanout):
    build = tmp / "package"
    args = [sys.executable, str(ROOT / "tools/build_native_el0.py"), "--out", str(build),
            "--policy", "fifo", "--elf", str(client), "--priority", "4",
            "--arg", str(port), "--arg", cookie.hex(),
            "--peer-elf", str(broker), "--peer-priority", "7", "--peer-arg", "1",
            "--ipc-route", "0:1", "--socket-service", "1", "--net-service", "1"]
    if scanout:
        args += ["--arg", "scanout", "--display-service", "0"]
    result = subprocess.run(args, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    log = result.stdout
    if result.returncode and (build / "build.log").exists():
        log += (build / "build.log").read_bytes()
    for sensitive in (cookie.hex().encode().hex().encode(), cookie.hex().encode()):
        log = log.replace(sensitive, b"[ephemeral cookie]")
    (out / "package.log").write_bytes(log)
    require(result.returncode == 0, "guest packaging failed; see package.log")
    manifest = json.loads((build / "manifest.json").read_text())
    kernel = build / "kernel.elf"
    hashes = dict(kernel=sha(kernel), client=sha(client), broker=sha(broker))
    require(hashes["kernel"] == manifest["kernel_elf_sha256"] and
            [s["elf_sha256"] for s in manifest["slots"]] == [hashes["client"], hashes["broker"]],
            "ELF identity mismatch")
    return kernel, dict(commit=manifest["commit"], tracked_changes=manifest["tracked_changes"],
                        sha256=hashes, socket_service=manifest["socket_service"],
                        net_service=manifest["net_service"], display_service=manifest.get("display_service"),
                        policy="fifo")


def qemu_args(program, kernel, out, qmp_path):
    args = [program, "-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
            "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none", "-monitor", "none",
            "-no-reboot", "-kernel", str(kernel), "-serial", "file:" + str(out / "uart"),
            "-netdev", "user,id=lan,net=10.0.2.0/24,host=10.0.2.2",
            "-global", "virtio-mmio.force-legacy=false",
            "-device", "virtio-net-device,netdev=lan,mac=52:54:00:12:34:56"]
    if qmp_path is not None:
        args += ["-device", "ramfb,id=screen", "-qmp", f"unix:{qmp_path},server=on,wait=off"]
    return args


def open_qmp(stack, qemu, path):
    sock = stack.enter_context(socket.socket(socket.AF_UNIX, socket.SOCK_STREAM))
    sock.settimeout(5)
    deadline = time.monotonic() + 10
    while True:
        try:
            sock.connect(str(path))
            break
        except (FileNotFoundError, ConnectionRefusedError):
            require(qemu.poll() is None and time.monotonic() < deadline,
                    "QMP unavailable; see qemu.log")
            time.sleep(0.03)
    pipe = stack.enter_context(sock.makefile("rwb", buffering=0))
    version = json.loads(pipe.readline())["QMP"]["version"]
    request(pipe, "qmp_capabilities")
    return pipe, version


def observed_window(root):
    for child in root.query_tree().children:
        name = child.get_full_property(Xatom.WM_NAME, Xatom.STRING)
        if name and name.value == TITLE.encode() and child.get_attributes().map_state == X.IsViewable:
            return child
    return None


def capture(observer):
    screen = observer.screen()
    width, height = screen.width_in_pixels, screen.height_in_pixels
    image = screen.root.get_image(0, 0, width, height, X.ZPixmap, 0xffffffff)
    visual = next(v for depth in screen.allowed_depths for v in depth.visuals
                  if v.visual_id == screen.root_visual)
    require((width, height, image.depth, visual.red_mask, visual.green_mask, visual.blue_mask)
            == (320, 240, 24, 0xff0000, 0xff00, 0xff), "unexpected Xvfb screen or visual")
    require(len(image.data) == width * height * 4, "unexpected Xvfb pixel format")
    rgb = bytearray(width * height * 3)
    channels = (2, 1, 0) if observer.display.info.image_byte_order == X.LSBFirst else (1, 2, 3)
    for channel, offset in enumerate(channels):
        rgb[channel::3] = image.data[offset::4]
    return rgb


def png(path, rgb):
    def chunk(kind, data):
        return (struct.pack(">I", len(data)) + kind + data
                + struct.pack(">I", zlib.crc32(kind + data)))
    stride = 320 * 3
    scanlines = b"".join(b"\0" + rgb[row:row+stride] for row in range(0, len(rgb), stride))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 320, 240, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(scanlines)) + chunk(b"IEND", b""))


def expected_scene(changed):
    _, _, width, height = GEOMETRY
    rgb = bytearray(bytes.fromhex("182840") * width * height)
    for x, y, w, h, color in ((16, 16, 64, 128, "20b8a0"), (96, 16, 128, 48, "f4b644"),
                              (96, 80, 128, 64, "498cf0" if changed else "e85c50")):
        for row in range(y, y+h):
            start = (row * width + x) * 3
            rgb[start:start+w*3] = bytes.fromhex(color) * w
    return rgb


def matched_frame(observer, changed):
    rgb = capture(observer)
    x, y, w, h = GEOMETRY
    start = (y * 320 + x) * 3
    crop = b"".join(rgb[i:i+w*3] for i in range(start, start + h*320*3, 320*3))
    return rgb if crop == expected_scene(changed) else None


def click(observer, window):
    root = observer.screen().root
    xtest.fake_input(observer, X.MotionNotify, root=root, x=32+128, y=32+110)
    observer.sync()
    require(root.query_pointer().child == window, "XTEST pointer missed the guest window")
    xtest.fake_input(observer, X.ButtonPress, detail=1)
    xtest.fake_input(observer, X.ButtonRelease, detail=1)
    observer.sync()


def local_canvas(remote):
    x, y, w, h = GEOMETRY
    left, top = LOCAL_ORIGIN
    canvas = bytearray(320 * 240 * 3)
    for row in range(h):
        src = 3 * ((y + row) * 320 + x)
        dst = 3 * ((top + row) * 320 + left)
        canvas[dst:dst+w*3] = remote[src:src+w*3]
    return canvas


def observe(observer, qemu, out, status, timeout, qmp):
    root = observer.screen().root
    deadline = time.monotonic() + timeout

    def tick(label, allow_exit=False):
        uart = (out / "uart").read_bytes() if (out / "uart").exists() else b""
        exits = [[int(pid), int(code)] for pid, code in re.findall(EXIT, uart)]
        status["guest_exits"] = exits
        require(b"NATIVE EL0 FAULT" not in uart, "guest fault; see uart")
        require(all(code == 0 for _, code in exits), f"guest exit {exits}; see uart")
        require(allow_exit or not exits, f"guest exited before {label}")
        require("error" not in status["relay"], f"TCP relay failed: {status['relay']}")
        require(qemu.poll() is None, "QEMU stopped; see qemu.log")
        require(time.monotonic() < deadline, f"timeout waiting for {label}; see uart and xvfb.log")

    def wait(label, get, allow_exit=False):
        while True:
            tick(label, allow_exit)
            value = get()
            if value:
                return value
            time.sleep(0.04)

    window = wait("mapped JNOX window", lambda: observed_window(root))
    geom = window.get_geometry()
    geometry = (geom.x, geom.y, geom.width, geom.height)
    status.update(title=TITLE, window_id=window.id, geometry=geometry, border_px=geom.border_width,
                  input_method="XTEST button 1 press/release", input_relative_xy=[128, 110])
    require(geometry == GEOMETRY and geom.border_width == 0, f"unexpected geometry: {geometry}")
    status["frames"] = {}
    previous_requests = 0
    previous_bytes = 0
    for label, changed in (("before", False), ("after", True)):
        rgb = wait(label + " framebuffer", lambda: matched_frame(observer, changed))
        path = out / (label + ".png")
        png(path, rgb)
        samples = {}
        for x, y in ((32, 32), (120, 32), (88, 72), (128, 110)):
            offset = ((geometry[1]+y) * 320 + geometry[0]+x) * 3
            samples[f"{x},{y}"] = "#" + rgb[offset:offset+3].hex()
        status["frames"][label] = dict(png_sha256=sha(path), samples=samples, matched_pixels=240*160)
        if qmp is not None:
            local_label = "local-" + label
            frame = stable_frame(qmp, out, local_label, local_canvas(rgb), lambda: tick(local_label))
            wire = status["relay"]
            strips = [dict(format=2, drawable=window.id, x=0, y=y, width=240, height=32,
                           plane_mask=0xffffffff) for y in range(0, 160, 32)]
            require(wire["get_image"][previous_requests:] == strips, "incomplete guest GetImage strips")
            require(wire["to_guest"] - previous_bytes >= len(strips)*32 + 240*160*4,
                    "missing guest image transfer")
            previous_requests, previous_bytes = len(wire["get_image"]), wire["to_guest"]
            frame.update(png_sha256=sha(out / (local_label + ".png")), origin=LOCAL_ORIGIN,
                         matched_pixels=320*240, server_bytes_cumulative=previous_bytes)
            status["frames"][local_label] = frame
        click(observer, window)
    wait("destroyed JNOX window", lambda: window not in root.query_tree().children, True)
    wait("clean exits of client and broker", lambda: len(status["guest_exits"]) >= 2, True)
    require(sorted(status["guest_exits"]) == [[0, 0], [1, 0]], "unexpected process exits")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("client", type=Path)
    parser.add_argument("broker", type=Path)
    parser.add_argument("out", type=Path, help="fresh evidence directory")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--local-scanout", action="store_true", help="check guest GetImage and RAMFB presentation")
    parser.add_argument("--timeout", type=float, help="post-boot wall-clock seconds (90 remote, 300 with scanout)")
    args = parser.parse_args()
    timeout = args.timeout if args.timeout is not None else (300 if args.local_scanout else 90)
    if timeout <= 0:
        parser.error("timeout must be positive")
    out = args.out.resolve()
    out.mkdir(mode=0o700)
    status = dict(result="failed", local_scanout=args.local_scanout, timeout_seconds=timeout,
                  relay=dict(to_server=0, to_guest=0))
    try:
        with tempfile.TemporaryDirectory(prefix="jnox-x11-") as directory, ExitStack() as stack:
            tmp = Path(directory)
            cookie = secrets.token_bytes(16)
            listener = stack.enter_context(socket.socket(socket.AF_INET, socket.SOCK_STREAM))
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
            kernel, status["package"] = package(args.client.resolve(), args.broker.resolve(),
                                                tmp, out, port, cookie, args.local_scanout)
            observer, unix_path = start_display(stack, tmp, out, cookie)
            check_auth(unix_path)
            status["invalid_cookie_rejected"] = True
            listener.listen(1)
            status["relay"]["listen"] = ["127.0.0.1", port]
            stack.enter_context(relay(listener, unix_path, status["relay"], args.local_scanout))
            qmp_path = tmp / "qmp.sock" if args.local_scanout else None
            argv = qemu_args(args.qemu, kernel, out, qmp_path)
            status["qemu_argv"] = argv
            qemu = stack.enter_context(process(argv, out / "qemu.log"))
            qmp = None
            if qmp_path is not None:
                qmp, status["qmp_version"] = open_qmp(stack, qemu, qmp_path)
            observe(observer, qemu, out, status, timeout, qmp)
        require(all(status["relay"][key] > 0 for key in ("to_server", "to_guest")),
                "missing bidirectional guest TCP traffic")
        status["result"] = "passed"
    except (OSError, RuntimeError, ValueError) as exc:
        status["error"] = str(exc)
        print(exc, file=sys.stderr)
    finally:
        (out / "status.json").write_text(json.dumps(status, indent=2) + "\n")
    print(f"{status['result']}: {out / 'status.json'}")
    return int(status["result"] != "passed")


if __name__ == "__main__":
    sys.exit(main())
