#!/usr/bin/env python3
"""Observe the four-process native window pair on one cold QEMU virt boot (Linux)."""
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
from types import SimpleNamespace
import zlib

from native_amc_roots import linked_roots
from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import request
from verify_native_display import FONT, HEIGHT, WIDTH, observed_exits, receive, stable_frame


ROOT = Path(__file__).resolve().parents[1]
SLOTS = (
    ("user.elf", [], 7, "user_elf_sha256"),
    ("native_peer.elf", ["a"], 5, "peer_elf_sha256"),
    ("native_peer2.elf", ["b"], 4, "peer2_elf_sha256"),
    ("native_peer3.elf", ["7"], 1, "peer3_elf_sha256"),
)
EXITS = [(0, 70), (1, 71), (2, 72), (3, 0)]
# Each input follows two matching external frames of the *preceding* state.
SCENES = (
    ("a-focused", "A", False, False, b"x"),
    ("a-ink", "A", True, False, b"n"),
    ("b-focused", "B", True, False, b"x"),
    ("b-ink", "B", True, True, b"q"),
)
GLYPHS = {**FONT, "C": "0e11101010110e", "K": "11121418141211", "W": "1111111515150a"}
PULSE = (0x0010, 0x0030, 0x0028, 0x0028, 0x0068, 0x0448, 0x0a44, 0xf244, 0x0182, 0x0102, 0x0101, 0)
TILES = (0x7e7e, 0x4242, 0x5a5a, 0x5a5a, 0x4242, 0x7e7e, 0, 0x3c3c, 0x2424, 0x2424, 0x3c3c, 0)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def expected_frame(focus, a_ink, b_ink):
    """Paint independent host fixtures in stacking order, without reading guest data."""
    canvas = bytearray(bytes.fromhex("101923") * WIDTH * HEIGHT)

    def rect(x, y, w, h, color):
        strip = bytes.fromhex(color) * w
        for row in range(y, y + h):
            start = 3 * (row * WIDTH + x)
            canvas[start:start + 3*w] = strip

    def mask(x, y, rows, width, scale, color):
        for dy, bits in enumerate(rows):
            for dx in range(width):
                if bits & (1 << (width - 1 - dx)):
                    rect(x + dx*scale, y + dy*scale, scale, scale, color)

    def label(x, y, title, color):
        for char in title:
            if char != " ":
                mask(x, y, bytes.fromhex(GLYPHS[char]), 5, 1, color)
            x += 6

    label(24, 12, "LOCAL WINDOWS", "acbdc5")
    label(24, 220, "N FOCUS   X INK   Q QUIT", "acbdc5")
    windows = {
        "A": (24, 32, 160, 120, "398657" if a_ink else "276575",
              "A  PULSE", 16, PULSE, "d2ece8"),
        "B": (112, 84, 176, 120, "a25639" if b_ink else "704567",
              "B  TILES", 72, TILES, "f1d5bc"),
    }
    for name in sorted(windows, key=lambda role: role == focus):
        x, y, w, h, body, title, motif_x, motif, ink = windows[name]
        border = "e0eee9" if name == focus else "536572"
        rect(x, y, w, h, border)
        rect(x + 2, y + 2, w - 4, h - 4, body)
        rect(x + 2, y + 2, w - 4, 22, "233541")
        label(x + 10, y + 8, title, "e0eee9")
        rect(x + w - 16, y + 9, 6, 6, border)
        mask(x + motif_x, y + 36, motif, 16, 4, ink)
    return bytes(canvas)


def check_png(path, expected):
    """Check the lossless RGB8, unfiltered PNG produced from the QMP PPM."""
    with path.open("rb") as image:
        require(image.read(8) == b"\x89PNG\r\n\x1a\n", f"{path.name}: bad PNG signature")
        compressed = bytearray()
        for kind in (b"IHDR", b"IDAT", b"IEND"):
            length, tag = struct.unpack(">I4s", image.read(8))
            data = image.read(length)
            require(tag == kind and struct.unpack(">I", image.read(4))[0] == zlib.crc32(tag + data),
                    f"{path.name}: invalid {kind!r} chunk")
            if tag == b"IHDR":
                require(struct.unpack(">IIBBBBB", data) == (WIDTH, HEIGHT, 8, 2, 0, 0, 0),
                        f"{path.name}: unexpected PNG dimensions or format")
            elif tag == b"IDAT":
                compressed.extend(data)
            else:
                require(not data and not image.read(1), f"{path.name}: trailing PNG data")
    stride = WIDTH * 3
    rows = b"".join(b"\0" + expected[i:i + stride] for i in range(0, len(expected), stride))
    require(zlib.decompress(compressed) == rows, f"{path.name}: decoded PNG pixels differ")


def source_record(manifest):
    revision = manifest["commit"]
    require(re.fullmatch("[a-f0-9]{40}", revision) and not manifest["tracked_changes"],
            "package must identify a clean source commit")
    paths = ("examples/native-window-pair", "services/socket", "sdk/native-aarch64",
             "arch", "boards", "kernel", "tools/build_native_el0.py", "tools/build_kernel.py")
    # The commit anchors kernel and SDK sources as well as the prelinked Rust applications.
    # Hash the files in this checkout; the packager separately hashes its input ELFs.
    subprocess.run(["git", "diff", "--quiet", revision, "--", *paths], cwd=ROOT, check=True)
    files = subprocess.check_output(["git", "ls-tree", "-r", "-z", "--name-only", revision, "--", *paths],
                                    cwd=ROOT).decode().split("\0")
    sources = {p: sha(ROOT / p) for p in files if p and not p.endswith(".md")}
    for p in ("examples/native-window-pair/Cargo.toml", "examples/native-window-pair/src/main.rs",
              "examples/native-window-pair/src/client.rs", "examples/native-window-pair/src/protocol.rs",
              "examples/native-window-pair/src/scene.rs",
              "services/socket/Cargo.toml", "services/socket/src/main.rs", "services/socket/src/local.rs"):
        require(p in sources, f"missing role source in package revision: {p}")
    return {"commit": revision, "source_sha256": sources}


def freeze_package(package, dest):
    dest.mkdir()
    for name in ("manifest.json", "build.log", "kernel.elf", *(s[0] for s in SLOTS)):
        shutil.copyfile(package / name, dest / name)
    manifest = json.loads((dest / "manifest.json").read_bytes())
    require((manifest["format"], manifest["profile"], manifest["app"], manifest["board"], manifest["policy"])
            == (1, "native", "process", "virt", "fifo"), "expected ordinary native virt FIFO process package")
    require(manifest["display_service"] == {
        "slot": 0, "display": "ramfb", "input": "pl011", "width": WIDTH, "height": HEIGHT,
        "stride": WIDTH * 4, "format": "XRGB8888", "syscalls": {"init": 72, "write_row": 73, "poll_key": 74}},
        "expected only slot 0 to own RAMFB and PL011")
    require(manifest["socket_service"] == {"slot": 3, "authorized_clients": [0, 1, 2]}
            and manifest["ipc_routes"] == [{"caller": pid, "server": 3} for pid in range(3)],
            "expected local socket service and three explicit inbound routes")
    for key in ("net_service", "pubsub_service", "pipes", "cap_grants", "loan_route", "notify_routes",
                "mailbox_routes", "cap_routes", "waitset", "thread_capacity"):
        require(not manifest.get(key), f"unexpected package facility {key}")
    kernel = dest / "kernel.elf"
    require(sha(kernel) == manifest["kernel_elf_sha256"] and sha(dest / "build.log") == manifest["build_log_sha256"],
            "kernel or build log differs from manifest")
    require(len(manifest["slots"]) == len(SLOTS), "expected four process slots")
    roots = linked_roots(kernel, len(SLOTS))
    kernel_bytes = kernel.read_bytes()
    for pid, (slot, (name, args, priority, hash_key)) in enumerate(zip(manifest["slots"], SLOTS)):
        require((slot["slot"], slot["elf"], slot["argv"], slot["priority"], slot["expected_ttbr0_el1"])
                == (pid, name, ["/bin/app", *args], priority, roots[pid]), f"slot {pid}: unexpected role or MMU root")
        data = (dest / name).read_bytes()
        digest = sha256(data).hexdigest()
        original = slot["input"]
        require(original["kind"] == "elf" and
                digest == slot["elf_sha256"] == manifest[hash_key] == original["sha256"]
                == manifest["source_sha256"][original["path"]], f"slot {pid}: ELF hash mismatch")
        require(data.startswith(b"\x7fELF\x02\x01") and data[18:20] == b"\xb7\x00" and data in kernel_bytes,
                f"slot {pid}: missing embedded AArch64 ELF in frozen kernel")
    require(manifest["user_elf_sha256"] == manifest["peer_elf_sha256"] == manifest["peer2_elf_sha256"]
            and manifest["peer3_elf_sha256"] != manifest["user_elf_sha256"], "unexpected client or broker identity")
    return manifest


def boot_prefix(manifest):
    lines = [b"JNOX M1 BOOT", b"JNOX M1 READY"]
    for slot, label in zip(manifest["slots"], (b"ELF", b"PEER ELF", b"PEER2 ELF", b"PEER3 ELF")):
        lines.append(b"NATIVE EL0 " + label + b" " + slot["elf_sha256"].encode("ascii"))
    return b"\n".join(lines) + b"\n"


def check_terminal(raw, prefix):
    require(raw.startswith(prefix), "missing board boot, ready, or four pinned UART ELF identities")
    lines = raw[len(prefix):].splitlines(keepends=True)
    footer = [f"NATIVE EL0 EXIT PID {pid} {code}\n".encode() for pid, code in EXITS]
    require(len(lines) >= 6 and lines[-4:] == footer, "missing or unexpected terminal exit records")
    rows = lines[:-5]
    dropped = re.fullmatch(rb"NATIVE FIXED DROPPED (\d+)\n", lines[-5])
    require(dropped and all(re.fullmatch(rb"NATIVE FIXED EVENT(?: \d+){24}\n", row) for row in rows),
            "malformed native four-slot scheduler report")
    return {"fixed_events": len(rows), "dropped_fixed_events": int(dropped[1]), "native_exits": observed_exits(raw)}


def run(qemu, out, manifest, frozen_hashes, timeout, result):
    raw = bytearray()
    package = out / "package"
    kernel = package / "kernel.elf"
    prefix = boot_prefix(manifest)
    with tempfile.TemporaryDirectory(dir="/tmp") as temp, (out / "uart.bin").open("xb", buffering=0) as uart, \
            (out / "qemu.stderr").open("xb") as stderr, (out / "input.bin").open("xb", buffering=0) as sent:
        path = str(Path(temp) / "pl011")
        argv = [qemu, *FLAGS, "-S", "-qmp", "stdio", "-chardev",
                f"socket,id=pl011,path={path},server=on,wait=off", "-serial", "chardev:pl011",
                "-device", "ramfb,id=screen", "-kernel", str(kernel)]
        save(out / "launch.json", {"argv": argv, "timeout_seconds": timeout, "package_sha256": frozen_hashes,
                                  "qemu_sha256": sha(Path(qemu)), "start_unix_ns": time.time_ns()})
        proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, bufsize=0)
        started = time.monotonic()
        # Includes QMP greeting and blocking screendump replies.
        timer = threading.Timer(timeout, proc.kill)
        timer.start()
        stage = "QMP greeting"
        try:
            with socket.socket(socket.AF_UNIX) as serial:
                result["qmp_version"] = json.loads(proc.stdout.readline())["QMP"]["version"]
                pipe = SimpleNamespace(write=proc.stdin.write, readline=proc.stdout.readline)
                request(pipe, "qmp_capabilities")
                serial.connect(path)
                serial.setblocking(False)
                request(pipe, "cont")
                quit_sent = False

                def tick():
                    receive(serial, uart, raw)
                    if proc.poll() is not None or time.monotonic() - started >= timeout:
                        raise RuntimeError("QEMU exited or exceeded timeout")
                    exits = observed_exits(raw)
                    require(exits == EXITS[:len(exits)], f"unexpected guest exits {exits}")
                    require(quit_sent or not exits, "guest exited before operator quit")
                    return exits

                for name, focus, a_ink, b_ink, key in SCENES:
                    stage = f"wait for {name} scanout"
                    expected = expected_frame(focus, a_ink, b_ink)
                    frame = stable_frame(pipe, out, name, expected, tick)
                    stage = f"verify {name} image and UART before sending {key!r}"
                    check_png(out / (name + ".png"), expected)
                    tick()
                    require(raw.startswith(prefix), "boot UART differs from frozen package")
                    frame.update(matched_pixels=WIDTH*HEIGHT, focus=focus, a_ink=a_ink, b_ink=b_ink,
                                 captured_seconds=time.monotonic() - started)
                    result["frames"][name] = frame
                    serial.sendall(key)
                    sent.write(key)
                    result["input"].append({"byte": key.decode(), "after_frame": name, "seconds": time.monotonic() - started})
                    quit_sent = key == b"q"
                stage = "wait for client receipts and all four terminal exits"
                while tick() != EXITS:
                    time.sleep(0.01)
                request(pipe, "stop")
                receive(serial, uart, raw)
                result.update(check_terminal(raw, prefix))
                request(pipe, "quit")
                require(proc.wait(timeout=3) == 0, f"QEMU final status {proc.returncode}")
                require(all(sha(package / name) == digest for name, digest in frozen_hashes.items()),
                        "frozen package changed during capture")
        except Exception as exc:
            raise RuntimeError(f"{stage}: {exc}") from exc
        finally:
            timer.cancel()
            timer.join()
            if proc.poll() is None:
                proc.kill()
            proc.wait(timeout=3)
            proc.stdin.close()
            proc.stdout.close()
            result.update(qemu_status=proc.returncode, elapsed_seconds=time.monotonic() - started)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path, help="four-role native package from a clean source commit")
    parser.add_argument("--out", required=True, type=Path, help="new evidence directory")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--timeout", type=int, default=45, help="wall-clock seconds for the entire boot")
    args = parser.parse_args()
    if args.timeout < 1:
        parser.error("timeout must be positive")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    result = {"pass": False, "frames": {}, "input": []}
    try:
        package = out / "package"
        manifest = freeze_package(args.package, package)
        sources = source_record(manifest)
        tools = ("verify_native_window_pair.py", "verify_native_display.py", "native_amc_roots.py",
                 "ordinary_amc_capture.py", "qemu_capture.py")
        sources["observer_sha256"] = {name: sha(ROOT / "tools" / name) for name in tools}
        save(out / "sources.json", sources)
        qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
        frozen_hashes = {p.name: sha(p) for p in sorted(package.iterdir())}
        run(qemu, out, manifest, frozen_hashes, args.timeout, result)
        result["pass"] = True
    except (KeyError, OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        result["error"] = str(exc)
        print(exc, file=sys.stderr)
    finally:
        result["hashes"] = {p.name: sha(p) for p in sorted(out.iterdir()) if p.is_file()}
        save(out / "result.json", result)
    print(f"{'PASS' if result['pass'] else 'FAIL'} {out}")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
