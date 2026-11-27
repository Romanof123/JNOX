#!/usr/bin/env python3
"""Exercise the host compositor with two independent Wayland wire clients and test packets matching guest syscall 197's event format."""
import argparse
import array
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time


def words(*values):
    return struct.pack("<" + "I" * len(values), *values)


def string(value):
    blob = value.encode() + b"\0"
    return words(len(blob)) + blob + bytes(-len(blob) % 4)


class Client:
    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(5)
        self.sock.connect(str(path))

    def send(self, obj, op, data=b"", fd=None):
        message = words(obj, ((8 + len(data)) << 16) | op) + data
        ancillary = [] if fd is None else [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", [fd]))]
        count = self.sock.sendmsg([message], ancillary)
        self.sock.sendall(message[count:])

    def take(self, n):
        blob = bytearray()
        while len(blob) < n:
            chunk = self.sock.recv(n - len(blob))
            if not chunk:
                raise EOFError("compositor disconnected")
            blob.extend(chunk)
        return bytes(blob)

    def expect(self, obj, op):
        while True:
            source, size_op = struct.unpack("<II", self.take(8))
            size, code = divmod(size_op, 65536)
            data = self.take(size - 8)
            if source == 1 and code == 0:
                raise ValueError(f"wl_display.error {data!r}")
            if source == obj and code == op:
                return struct.unpack("<" + "I" * (len(data) // 4), data)
            if source == 7:
                raise ValueError(f"unexpected wl_pointer event {code} {data.hex()}")

    def bind(self, pointer):
        self.send(1, 1, words(2))
        for name, interface, version, obj in [(1, "wl_compositor", 4, 3), (2, "wl_shm", 1, 4),
                                               (3, "xdg_wm_base", 1, 5), (4, "wl_seat", 3, 6)]:
            self.send(2, 0, words(name) + string(interface) + words(version, obj))
        assert self.expect(6, 0) == (3 if pointer else 2,)
        if pointer:
            self.send(6, 0, words(7))

    def map(self, app):
        self.send(3, 0, words(8))
        self.send(5, 2, words(9, 8))
        self.send(9, 1, words(10))
        self.send(10, 3, string(app))
        self.send(8, 6)
        serial, = self.expect(9, 0)
        self.send(9, 4, words(serial))
        with tempfile.TemporaryFile() as backing:
            backing.truncate(128 * 96 * 4)
            self.send(4, 0, words(11, 128 * 96 * 4), backing.fileno())
        self.send(11, 0, words(12, 0, 128, 96, 512, 1))
        self.send(8, 1, words(12, 0, 0))
        self.send(8, 6)
        self.expect(12, 0)


def exercise(binary, devices):
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        display, device = root / "display", root / "tablet"
        env = dict(os.environ)
        env.pop("JNOX_WAYLAND_TEST_POINTER", None)
        if devices:
            env["JNOX_WAYLAND_TEST_POINTER"] = str(device)
        with subprocess.Popen([binary, display], stdin=subprocess.PIPE, env=env) as server:
            try:
                deadline = time.monotonic() + 5
                while not display.exists():
                    if server.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("host compositor did not start")
                    time.sleep(.01)
                a = Client(display)
                a.bind(devices)
                if not devices:
                    a.sock.close()
                    return
                a.map("org.jnox.Pulse")
                b = Client(display)
                b.bind(True)
                b.map("org.jnox.Tiles")
                with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as tablet:
                    def packet(*events):
                        for kind, code, value in (*events, (0, 0, 0)):
                            tablet.sendto(struct.pack("<Q", kind << 48 | code << 32 | value), str(device))
                    packet((3, 0, 120), (3, 1, 88))
                    enter = a.expect(7, 0)
                    assert enter[1:] == (8, 104 * 256, 64 * 256)
                    packet((3, 0, 121), (1, 272, 1))
                    assert a.expect(7, 2)[1:] == (105 * 256, 64 * 256)
                    press = a.expect(7, 3)
                    assert press[2:] == (272, 1) and press[0] > enter[0]
                    packet((3, 0, 190))
                    assert a.expect(7, 2)[1:] == (174 * 256, 64 * 256)
                    packet((1, 272, 0))
                    release = a.expect(7, 3)
                    assert release[2:] == (272, 0)
                    leave = a.expect(7, 1)
                    enter_b = b.expect(7, 0)
                    assert press[0] < release[0] < leave[0] < enter_b[0]
                    assert enter_b[1:] == (8, 78 * 256, 8 * 256)
                    b.send(7, 1)
                    assert b.expect(1, 1) == (7,)
                    tablet.sendto(b"broken", str(device))
                    assert a.expect(6, 0) == b.expect(6, 0) == (2,)
                a.sock.close()
                b.sock.close()
            finally:
                server.terminate()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compositor", type=Path)
    args = parser.parse_args()
    for present in (False, True):
        exercise(args.compositor.resolve(), present)
    print("Host pointer routing, implicit grab, release and capability checks passed.")

