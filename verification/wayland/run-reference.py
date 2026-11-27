#!/usr/bin/env python3
"""Run the stock host client against a supplied JNOX compositor executable."""
import argparse
import array
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked_libraries(pid, root, out):
    text = Path(f"/proc/{pid}/maps").read_text()
    out.joinpath("client.maps").write_text(text)
    paths = {Path(fields[5]) for line in text.splitlines()
             if len(fields := line.split(maxsplit=5)) == 6 and fields[5].startswith("/")}
    libs = {}
    manifest = json.loads(Path(__file__).with_name("upstream.json").read_text())
    for name in ("libwayland-client", "libxkbcommon"):
        expected = next(path for path in manifest["files"] if Path(path).name.startswith(name + ".so."))
        actual = [p for p in paths if p.name.startswith(name + ".so.")]
        if len(actual) != 1 or not actual[0].samefile(root / expected):
            raise ValueError(f"{name}: unexpected mapped library {actual}")
        libs[name] = digest(actual[0].read_bytes())
        if libs[name] != manifest["files"][expected]:
            raise ValueError(f"{actual[0]}: mapped library digest mismatch")
    return libs


def locate(data):
    width, height = 320, 240
    if len(data) != width * height * 4:
        return None
    words = array.array("I", data)
    if sys.byteorder != "little":
        words.byteswap()
    rgb = [word & 0xffffff for word in words]
    start = 0x5100a3
    candidates = []
    for pos, word in enumerate(rgb):
        if word != start:
            continue
        y, x = divmod(pos, width)
        if x + 20 > width or y + 16 > height:
            continue
        if all(rgb[pos + j * width + i] == start + (j << 16) + (i << 8)
               for j in range(16) for i in range(20)):
            candidates.append((x, y))
    if len(candidates) == 1:
        return candidates[0]
    return None


def capture_frame(path, out):
    until = time.monotonic() + 5
    while time.monotonic() < until:
        data = path.read_bytes()
        origin = locate(data)
        if origin is not None:
            (out / "mapped.raw").write_bytes(data)
            return {"sha256": digest(data), "origin": origin, "matching_pixels": 320}
        time.sleep(0.01)
    raise ValueError("320 client pixels did not appear together within the host framebuffer")


def stop(proc):
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


def run(args, out, sock):
    frame = out / "framebuffer.raw"
    env = dict(os.environ, JNOX_WAYLAND_FRAMEBUFFER=str(frame))
    with (out / "server.log").open("wb") as server_log:
        server = subprocess.Popen([args.server, sock], env=env, stdin=subprocess.PIPE,
                                  stdout=server_log, stderr=subprocess.STDOUT)
        try:
            until = time.monotonic() + 10
            while not sock.is_socket():
                if server.poll() is not None or time.monotonic() >= until:
                    raise ValueError("compositor did not create the requested socket")
                time.sleep(0.01)

            libs = args.sysroot / "usr/lib/x86_64-linux-gnu"
            env = dict(os.environ, WAYLAND_DEBUG="client", LD_LIBRARY_PATH=str(libs))
            with (out / "client.wire.log").open("wb") as wire:
                command = [args.client, "--socket", sock, "--hold-frame"]
                if args.geometry:
                    command.append("--geometry")
                client = subprocess.Popen(command,
                                          stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=wire, env=env)
                try:
                    if not select.select([client.stdout], [], [], 20)[0] or client.stdout.readline() != b"FRAME\n":
                        raise ValueError("client failed to reach the mapped frame")
                    result = {"libraries": checked_libraries(client.pid, args.sysroot, out),
                              "frame": capture_frame(frame, out)}
                    expected = (13, 22) if args.geometry else (16, 24)
                    if result["frame"]["origin"] != expected:
                        raise ValueError(f"buffer origin {result['frame']['origin']} differs from {expected}")
                    result["window_geometry"] = [3, 2, 14, 12] if args.geometry else None
                    output, _ = client.communicate(b"\n", timeout=10)
                    (out / "client.log").write_bytes(b"FRAME\n" + output)
                    if client.returncode != 0 or not output.startswith(b"PASS configure_serial="):
                        raise ValueError(f"client exit {client.returncode}: {output!r}")
                finally:
                    stop(client)
            if server.poll() is None:
                server.communicate(b"q", timeout=10)
            if server.returncode != 0:
                raise ValueError(f"compositor exit {server.returncode}")
        finally:
            stop(server)
    result.update(server_sha256=digest(args.server.read_bytes()), client_sha256=digest(args.client.read_bytes()),
                  wire_sha256=digest((out / "client.wire.log").read_bytes()),
                  client_result=output.decode("ascii").strip(), backend="linux-host")
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--geometry", action="store_true",
                        help="set a smaller, offset xdg window geometry and verify the full buffer placement")
    for name in ("server", "client", "sysroot", "output"):
        parser.add_argument(name, type=lambda p: Path(p).resolve())
    args = parser.parse_args()
    args.output.mkdir()
    with tempfile.TemporaryDirectory(prefix="jnox-wl-", dir="/tmp") as directory:
        run(args, args.output, Path(directory) / "display")
    print(args.output / "result.json")


if __name__ == "__main__":
    main()
