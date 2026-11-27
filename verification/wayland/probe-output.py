#!/usr/bin/env python3
"""Bounded raw protocol checks for output binding, object ownership and membership."""
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
    return struct.pack("<" + "I"*len(values), *(v & 0xffffffff for v in values))


def string(text):
    data = text.encode() + b"\0"
    return words(len(data)) + data + bytes(-len(data) % 4)


class Peer:
    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.settimeout(3)
        self.sock.connect(str(path))
        self.send(1, 1, words(2))
        self.globals = {}
        for obj, op, body in self.sync():
            if (obj, op) == (2, 0):
                name, n = struct.unpack_from("<II", body)
                interface = body[8:8+n-1].decode()
                assert body[8+n-1] == 0 and interface not in self.globals
                version, = struct.unpack("<I",body[8 + ((n+3)//4)*4:])
                self.globals[interface] = name, version
        assert self.globals["wl_output"][1] == 3

    def bind(self, interface, version, obj):
        name, _ = self.globals[interface]
        self.send(2, 0, words(name) + string(interface) + words(version, obj))

    def send(self, obj, op, body=b"", fd=None):
        msg = words(obj,(len(body)+8)<<16 | op) + body
        anc = [] if fd is None else [(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array("i",[fd]))]
        n = self.sock.sendmsg([msg], anc)
        self.sock.sendall(msg[n:])

    def take(self, n):
        buf = bytearray()
        while len(buf) < n:
            block = self.sock.recv(n-len(buf))
            if not block:
                raise EOFError("peer disconnected")
            buf.extend(block)
        return bytes(buf)

    def sync(self, error=None):
        if error is None:
            self.send(1, 0, words(63))
        events = []
        for _ in range(96):
            obj, header = struct.unpack("<II",self.take(8))
            size, op = divmod(header,65536)
            assert 8 <= size <= 4096 and size % 4 == 0
            body = self.take(size-8)
            if (obj, op) == (1, 0):
                assert error is not None and struct.unpack_from("<II", body) == error
                return events
            if (obj,op,body) == (1,1,words(63)):
                assert error is None and events.pop()[:2] == (63,0)
                return events
            events.append((obj,op,body))
        raise ValueError("sync reply exceeds event bound")


def description(obj, version):
    geometry = words(0,0,0,0,0) + string("JNOX") + string("RAM framebuffer") + words(0)
    events = [(obj,0,geometry),(obj,1,words(3,320,240,0))]
    if version >= 2:
        events.extend([(obj,3,words(1)),(obj,2,b"")])
    return events


def exercise(path):
    a, b = Peer(path), Peer(path)
    try:
        for ver, obj in ((1,20),(2,21)):
            a.bind("wl_output",ver,obj)
            assert a.sync() == description(obj,ver)
        b.bind("wl_output",3,25)
        assert b.sync() == description(25,3)
        a.bind("wl_compositor",4,3)
        a.bind("wl_shm",1,4)
        a.bind("xdg_wm_base",1,5)
        a.send(3,0,words(8))  # surface, xdg_surface, toplevel
        a.send(5,2,words(9,8))
        a.send(9,1,words(10))
        a.send(10,11,words(21))
        a.send(8,6)
        initial = a.sync()
        serial, = [struct.unpack("<I",data) for obj,op,data in initial if (obj,op) == (9,0)][0]
        a.send(9,4,words(serial))
        with tempfile.TemporaryFile() as f:
            f.truncate(4096)
            a.send(4,0,words(11,4096),f.fileno())
        a.send(11,0,words(12,0,8,4,32,1))
        def attach(dx, dy):
            a.send(8,1,words(12,dx,dy))
            a.send(8,6)
            events = a.sync()
            return [(op,struct.unpack("<I",msg)[0]) for obj,op,msg in events if obj == 8]
        assert attach(0,0) == [(0,20),(0,21)]
        a.bind("wl_output",3,22)
        assert a.sync() == description(22,3) + [(8,0,words(22))]
        a.send(22,0)
        assert a.sync() == [(1,1,words(22))]
        a.send(10,11,words(0))
        changes = a.sync()
        next_serial, = [struct.unpack("<I",msg) for obj,op,msg in changes if (obj,op) == (9,0)][0]
        a.send(9,4,words(next_serial))
        assert not attach(0,0)
        assert attach(400,0) == [(1,20),(1,21)]
        assert attach(-400,0) == [(0,20),(0,21)]
        assert not b.sync()
        a.send(8,1,words(0,0,0))
        a.send(8,6)
        assert a.sync() == [(8,1,words(20)),(8,1,words(21))]
        a.bind("wl_output",3,22)  # ID reusable; currently unmapped
        assert a.sync() == description(22,3)
        a.send(10,11,words(25))  # only B owns this ID
        a.sync(error=(1,0))
        b.send(25,0)
        assert b.sync() == [(1,1,words(25))]
    finally:
        a.sock.close()
        b.sock.close()
    for case in ("version","old-release","wrong-type","released"):
        c = Peer(path)
        try:
            if case == "version":
                c.bind("wl_output",4,7)
            elif case == "old-release":
                c.bind("wl_output",2,7)
                c.send(7,0)
            else:
                c.bind("wl_compositor",4,3)
                c.bind("xdg_wm_base",1,5)
                c.send(3,0,words(8))
                c.send(5,2,words(9,8))
                c.send(9,1,words(10))
                target = 3
                if case == "released":
                    c.bind("wl_output",3,7)
                    c.send(7,0)
                    target = 7
                c.send(10,11,words(target))
            c.sync(error=(1,1 if case == "old-release" else 0))
        finally:
            c.sock.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("server",type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        path = root/"display"
        env = dict(os.environ,JNOX_WAYLAND_FRAMEBUFFER=str(root/"fb"))
        with subprocess.Popen([args.server.resolve(),path],stdin=subprocess.PIPE,env=env) as server:
            try:
                deadline = time.monotonic()+5
                while not path.is_socket():
                    if server.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("host compositor did not start")
                    time.sleep(.01)
                exercise(path)
                server.communicate(b"q",timeout=8)
                assert server.returncode == 0
            finally:
                if server.poll() is None:
                    server.kill()
                    server.wait()
    print("Verified output v1/v2/v3 bindings, clipping, membership, release, fullscreen object ownership and peer isolation.")


if __name__ == "__main__":
    main()
