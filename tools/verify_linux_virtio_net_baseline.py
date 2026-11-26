#!/usr/bin/env python3
"""Boot the stock Alpine/AArch64 virtio_net driver and measure an external echo over its NIC."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import secrets
import shutil
import socket
import stat
import struct
import subprocess
import tarfile
import threading
import time

from qemu_capture import request


KERNEL = "linux-virt-6.18.65-r0.apk"
BUSYBOX = "busybox-static-1.37.0-r30.apk"
PIN = {
    KERNEL: "859ad147c29bf3c6d84a3ff84280aa5c94d890dc37ada31b072d099a8437f8e0",
    BUSYBOX: "44c9abdfb970f398fa72c8382fe2d8808eea16beaf82daf6ac708b92f1b8659e",
}
RELEASE = "6.18.65-0-virt"
MODULES = [
    "kernel/net/core/failover.ko.gz",
    "kernel/drivers/net/net_failover.ko.gz",
    "kernel/drivers/virtio/virtio_mmio.ko.gz",
    "kernel/drivers/net/virtio_net.ko.gz",
]
CONFIG = "boot/config-" + RELEASE
ZBOOT = "boot/vmlinuz-virt"
PREFIX = "lib/modules/" + RELEASE + "/"
MAC = "52:54:00:12:34:56"

INIT_PREFIX = """#!/bin/sh
set -eu
bb=/bin/busybox
$bb mount -t devtmpfs devtmpfs /dev
$bb mount -t proc proc /proc
$bb mount -t sysfs sysfs /sys
echo "LD BOOT $($bb uname -m) $($bb uname -r)"
for name in failover net_failover virtio_mmio virtio_net; do
    p=/modules/$name.ko
    $bb insmod "$p"
    $bb grep -q "^$name " /proc/modules
    digest=$($bb sha256sum "$p")
    echo "LD LOADED $name ${digest%% *}"
done
$bb ip link set eth0 up
$bb ip addr add 10.0.2.15/24 dev eth0
$bb ip route add default via 10.0.2.2
driver=$($bb readlink -f /sys/class/net/eth0/device/driver)
module=$($bb readlink -f /sys/class/net/eth0/device/driver/module)
transport=$($bb readlink -f /sys/class/net/eth0/device/../driver)
echo "LD DEVICE $driver $module $transport $($bb cat /sys/class/net/eth0/address)"
fdt=$($bb sha256sum /sys/firmware/fdt)
echo "LD FDT ${fdt%% *}"
"""

INIT = INIT_PREFIX + """echo "LD READY"
while :; do
    $bb nc -l -p 8080 -e /serve || exit 9
done
"""

SERVE = """#!/bin/sh
set -eu
bb=/bin/busybox
IFS=' ' read -r method target proto
[ "$method" = GET ] || exit 2
IFS= read -r end
[ "$end" = "$(printf '\\r')" ] || exit 2
case "$target" in
    /echo/*)
        token=${target#/echo/}
        case "$token" in ''|*[!0-9a-f]*) exit 3;; esac
        [ "${#token}" -eq 64 ] || exit 4
        printf 'LD RECEIVED %s\\n' "$token" >/dev/console
        printf 'HTTP/1.0 200 OK\\r\\nContent-Type: text/plain\\r\\nContent-Length: 65\\r\\nConnection: close\\r\\n\\r\\n%s\\n' "$token"
        ;;
    /board.dtb)
        size=$($bb wc -c </sys/firmware/fdt)
        printf 'HTTP/1.0 200 OK\\r\\nContent-Type: application/octet-stream\\r\\nContent-Length: %s\\r\\nConnection: close\\r\\n\\r\\n' "$size"
        $bb cat /sys/firmware/fdt
        ;;
    *) exit 5;;
esac
"""


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def save(path, obj):
    path.write_text(json.dumps(obj, indent=2) + "\n")


def extract(path, names):
    require(sha(path.read_bytes()) == PIN[path.name], f"unexpected package bytes: {path.name}")
    data = {}
    with gzip.open(path) as stream, tarfile.open(fileobj=stream, mode="r|", ignore_zeros=True) as apk:
        for member in apk:
            if member.name in names:
                require(member.isfile() and member.name not in data, "unexpected archive member")
                data[member.name] = apk.extractfile(member).read()
    require(set(data) == set(names), f"missing archive members: {set(names) - data.keys()}")
    return data


def initrd(files):
    """Write a reproducible newc archive, including the initial console for PID 1."""
    buf = bytearray()

    def entry(name, mode, contents=b"", major=0, minor=0):
        name = name.encode() + b"\0"
        fields = (len(buf) + 1, mode, 0, 0, 1, 0, len(contents), 0, 0, major, minor, len(name), 0)
        buf.extend(b"070701" + "".join(f"{v:08x}" for v in fields).encode() + name)
        buf.extend(b"\0" * (-len(buf) % 4))
        buf.extend(contents)
        buf.extend(b"\0" * (-len(buf) % 4))

    for name in ("bin", "dev", "modules", "proc", "sys"):
        entry(name, stat.S_IFDIR | 0o755)
    entry("dev/console", stat.S_IFCHR | 0o600, major=5, minor=1)
    entry("bin/sh", stat.S_IFLNK | 0o777, b"busybox")
    for name, data in sorted(files.items()):
        entry(name, stat.S_IFREG | (0o644 if name.startswith("modules/") else 0o755), data)
    entry("TRAILER!!!", 0)
    return gzip.compress(bytes(buf), mtime=0)


def prepare(intake, out, *, init_script=INIT, service_files=None):
    apk = extract(intake / KERNEL, [CONFIG, ZBOOT, PREFIX + "modules.dep", *(PREFIX + n for n in MODULES)])
    busybox = extract(intake / BUSYBOX, ["bin/busybox.static"])["bin/busybox.static"]
    require(busybox[:4] == b"\x7fELF" and busybox[18:20] == b"\xb7\0", "unexpected BusyBox architecture")
    config = apk[CONFIG]
    for cfg in (b"CONFIG_VIRTIO_NET=m", b"CONFIG_VIRTIO_MMIO=m", b"CONFIG_SERIAL_AMBA_PL011=y"):
        require(cfg in config.splitlines(), "unexpected kernel configuration")
    (out / "config").write_bytes(config)
    zboot = apk[ZBOOT]
    off, size = struct.unpack_from("<II", zboot, 8)
    image = gzip.decompress(zboot[off:off + size])
    require(image[56:60] == b"ARM\x64", "expected raw Linux arm64 Image")
    (out / "Image").write_bytes(image)

    if service_files is None:
        service_files = {"serve": SERVE.encode()}
        (out / "serve.sh").write_text(SERVE)
    files = {"bin/busybox": busybox, "init": init_script.encode(), **service_files}
    operands = {}
    deps = dict(line.split(":", 1) for line in apk[PREFIX + "modules.dep"].decode().splitlines())
    available = set()
    for member in MODULES:
        require(set(deps[member].split()) <= available, "module dependency order differs")
        available.add(member)
        compressed = apk[PREFIX + member]
        raw = gzip.decompress(compressed)
        require(raw.startswith(b"\x7fELF") and b"vermagic=" + RELEASE.encode() in raw, "unexpected module ABI")
        name = Path(member).name.removesuffix(".ko.gz")
        files[f"modules/{name}.ko"] = raw
        operands[name] = {"archive_path": PREFIX + member, "compressed_sha256": sha(compressed),
                          "ko_sha256": sha(raw)}
    ramdisk = initrd(files)
    (out / "initrd.gz").write_bytes(ramdisk)
    (out / "init.sh").write_text(init_script)
    return {"packages": PIN, "release": RELEASE, "image_sha256": sha(image),
            "vmlinuz_sha256": sha(zboot), "config_sha256": sha(config), "initrd_sha256": sha(ramdisk),
            "busybox_sha256": sha(busybox), "modules": operands}


def http(port, path, out, name, deadline):
    req = f"GET {path} HTTP/1.0\r\n\r\n".encode()
    (out / f"{name}.request").write_bytes(req)
    while True:
        if time.monotonic() >= deadline:
            raise TimeoutError(f"no complete HTTP response for {name}")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=2) as conn:
                conn.settimeout(min(4, max(0.1, deadline - time.monotonic())))
                conn.sendall(req)
                response = conn.makefile("rb")
                status = response.readline(1024)
                if not status:
                    continue
                require(status == b"HTTP/1.0 200 OK\r\n", "unexpected HTTP response status")
                headers = b""
                while not headers.endswith(b"\r\n\r\n"):
                    line = response.readline(1024)
                    require(line and len(headers) < 4096, "incomplete HTTP headers")
                    headers += line
                length = re.search(rb"(?mi)^Content-Length: (\d+)\r$", headers)
                require(length and int(length[1]) <= 4 * 1024 * 1024, "unexpected HTTP length")
                body = response.read(int(length[1]))
                require(len(body) == int(length[1]), "short HTTP body")
                (out / f"{name}.response").write_bytes(status + headers + body)
                return body
        except (ConnectionRefusedError, ConnectionResetError, socket.timeout):
            time.sleep(0.1)


def guest_observations(operands, nonce, dtb):
    observations = [f"LD BOOT aarch64 {RELEASE}", f"LD FDT {sha(dtb)}", f"LD RECEIVED {nonce}",
                    f"LD DEVICE /sys/bus/virtio/drivers/virtio_net /sys/module/virtio_net "
                    f"/sys/bus/platform/drivers/virtio-mmio {MAC}"]
    for name, module in operands["modules"].items():
        observations.append(f"LD LOADED {name} {module['ko_sha256']}")
    return observations


def replay(out):
    """Recheck compact captures. Image and initrd.gz can be regenerated from the two APKs."""
    record = json.loads((out / "result.json").read_bytes())
    require(record["passed"] and record["operands"]["packages"] == PIN, "package identity or verdict differs")
    for name, digest in record["files"].items():
        if name in ("Image", "initrd.gz") and not (out / name).exists():
            continue
        require(Path(name).name == name and sha((out / name).read_bytes()) == digest, f"{name} differs")
    nonce = record["observed"]["nonce"]
    require(re.fullmatch("[0-9a-f]{64}", nonce), "invalid challenge")
    dtb = (out / "running.dtb").read_bytes()
    require(dtb.startswith(b"\xd0\x0d\xfe\xed") and b"virtio,mmio\0" in dtb and
            sha(dtb) == record["observed"]["dtb_sha256"], "unexpected actual DTB")
    for name, path, expected in (("challenge", "/echo/" + nonce, (nonce + "\n").encode()),
                                 ("dtb", "/board.dtb", dtb)):
        require((out / f"{name}.request").read_bytes() == f"GET {path} HTTP/1.0\r\n\r\n".encode(),
                f"{name} request changed")
        headers, sep, body = (out / f"{name}.response").read_bytes().partition(b"\r\n\r\n")
        require(sep and headers.startswith(b"HTTP/1.0 200 OK\r\n") and
                f"Content-Length: {len(expected)}".encode() in headers.split(b"\r\n") and body == expected,
                f"{name} response changed")
    operands = record["operands"]
    require(operands["modules"]["virtio_net"]["ko_sha256"] ==
            "3873c32ab124c0a83e7a7f04c27fde93b230c45236bb9fc0b9125abee19dca4f", "stock module differs")
    require(operands["image_sha256"] == "4eb5c87867473f20f5ce5084c23f3dff587c84327f3fbba0cfe3157133c20c29"
            and sha((out / "config").read_bytes()) == operands["config_sha256"], "kernel or config differs")
    expected = guest_observations(operands, nonce, dtb)
    lines = (out / "uart.log").read_text(errors="replace").splitlines()
    require(set(expected) == set(record["observed"]["guest_observations"]) and all(s in lines for s in expected),
            "missing original boot, module, binding, DTB or request record")
    print("PASS replay: stock module binding, transmitted challenge, exact reply and guest DTB")


def run(qemu, out, operands, timeout):
    kernel, ramdisk, uart = out / "Image", out / "initrd.gz", out / "uart.log"
    argv = [qemu, "-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-accel", "tcg",
            "-smp", "1", "-m", "384M", "-nodefaults", "-no-reboot", "-display", "none",
            "-monitor", "none", "-qmp", "stdio", "-serial", f"file:{uart}",
            "-kernel", str(kernel), "-initrd", str(ramdisk),
            "-append", "console=ttyAMA0 rdinit=/init panic=-1", "-netdev",
            "user,id=lan,restrict=on,ipv6=off,hostfwd=tcp:127.0.0.1:0-10.0.2.15:8080",
            "-device", f"virtio-net-device,netdev=lan,mac={MAC}"]
    save(out / "launch.json", {"argv": argv, "qemu_sha256": sha(Path(qemu).read_bytes()),
                               "qemu_version": subprocess.check_output([qemu, "--version"], text=True).splitlines()[0]})
    with (out / "qemu.stderr").open("wb") as err, subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err) as proc:
        guard = threading.Timer(timeout, proc.kill)
        guard.start()
        deadline = time.monotonic() + timeout
        try:
            greeting = json.loads(proc.stdout.readline())
            require("QMP" in greeting, "missing QMP greeting")
            class QMP:
                def write(self, data):
                    proc.stdin.write(data)
                    proc.stdin.flush()

                def readline(self):
                    return proc.stdout.readline()
            qmp = QMP()
            request(qmp, "qmp_capabilities")
            info = request(qmp, "human-monitor-command", **{"command-line": "info usernet"})
            (out / "usernet.txt").write_text(info)
            port = re.search(r"TCP\[HOST_FORWARD\]\s+\d+\s+127\.0\.0\.1\s+(\d+)\s+10\.0\.2\.15\s+8080\b", info)
            require(port, "could not identify loopback-only forwarding rule")
            port = int(port[1])
            while not uart.exists() or b"LD READY" not in uart.read_bytes():
                require(proc.poll() is None and time.monotonic() < deadline, "guest failed to reach network readiness")
                time.sleep(0.1)
            nonce = secrets.token_hex(32)
            body = http(port, "/echo/" + nonce, out, "challenge", deadline)
            require(body == (nonce + "\n").encode(), "challenge reply differs")
            dtb = http(port, "/board.dtb", out, "dtb", deadline)
            require(dtb[:4] == b"\xd0\x0d\xfe\xed" and b"virtio,mmio\0" in dtb, "unexpected guest device tree")
            (out / "running.dtb").write_bytes(dtb)
            lines = uart.read_text(errors="replace").splitlines()
            expected = guest_observations(operands, nonce, dtb)
            require(all(line in lines for line in expected), "missing guest module, binding, DTB or challenge observation")
            request(qmp, "quit")
            require(proc.wait(timeout=5) == 0, "QEMU exited abnormally")
            return {"nonce": nonce, "dtb_sha256": sha(dtb), "host_port": port, "guest_observations": expected}
        finally:
            guard.cancel()
            guard.join()
            if proc.poll() is None:
                proc.kill()
                proc.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--intake", type=Path, help="directory containing both pinned Alpine APKs")
    parser.add_argument("--out", type=Path, help="new evidence/build directory")
    parser.add_argument("--replay", type=Path, help="verify an existing compact capture without booting Linux")
    parser.add_argument("--timeout", type=int, default=90)
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    args = parser.parse_args()
    if args.replay:
        replay(args.replay)
        return
    if not args.intake or not args.out:
        parser.error("specify --intake and --out, or --replay")
    require(args.timeout > 0, "timeout must be positive")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    result = {"passed": False}
    try:
        result["operands"] = prepare(args.intake, out)
        qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
        result["observed"] = run(qemu, out, result["operands"], args.timeout)
        result["passed"] = True
    except Exception as exc:
        result["error"] = str(exc)
        raise
    finally:
        result["files"] = {p.name: sha(p.read_bytes()) for p in sorted(out.iterdir()) if p.is_file()}
        save(out / "result.json", result)
    print("PASS stock AArch64 Linux virtio_net module, HTTP challenge and live DTB")


if __name__ == "__main__":
    main()
