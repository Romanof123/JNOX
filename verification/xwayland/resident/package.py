"""Package the original rootful Xwayland, Mosaic, Tiles and their native services."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from runtime_image import from_receipt


REPO = Path(__file__).resolve().parents[3]
SERVER_ARGS = (":0", "-auth", "/run/Xauthority", "-geometry", "320x240",
               "-shm", "-fp", "built-ins", "-terminate")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package(server_dir, out, epoch):
    out.mkdir()
    server = out / "xwayland.elf"
    image = from_receipt(server_dir, server)
    (out / "server-image.json").write_text(json.dumps(image, indent=2) + "\n")
    env = dict(os.environ, CARGO_BUILD_JOBS="1", CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(out / "target"))

    def run(*cmd):
        subprocess.run(cmd, check=True, cwd=REPO, env=env)

    comp, tiles, broker, mosaic = [out / (name + ".elf") for name in ("compositor", "tiles", "broker", "mosaic")]
    run("sh", "sdk/native-aarch64/build-rust.sh", "--local-socket", "services/wayland/Cargo.toml", comp)
    run("sh", "sdk/native-aarch64/build.sh", "--local-socket", "examples/native-wayland/client.c", tiles)
    run("sh", "sdk/native-aarch64/build-rust.sh", "--features", "display-five-active,tmpfs", "services/socket/Cargo.toml", broker)
    run("sh", "sdk/native-aarch64/build.sh", "--local-socket", "examples/native-x11/client.c", mosaic)
    dest = out / "package"
    run(sys.executable, "tools/build_native_el0.py", "--board", "virt", "--policy", "fifo", "--display-five-active",
        "--nodename", "guest-x11", "--console-write", "1", "--display-service", "0", "--socket-service", "3",
        "--shared-fd", "--pointer-tablet", "--tmpfs", "1:rw", "--tmpfs-x11-dir", "1", "--boot-utc-seconds", str(epoch),
        "--ipc-route", "0:3", "--ipc-route", "1:3", "--ipc-route", "2:3", "--ipc-route", "4:3",
        "--elf", comp, "--priority", "7", "--peer-elf", server, "--peer-priority", "5",
        *("--peer-arg=" + arg for arg in SERVER_ARGS), "--env", "1:WAYLAND_DISPLAY=/wayland-0",
        "--peer-elf", tiles, "--peer2-arg", "pb", "--peer-priority", "4",
        "--peer-elf", broker, "--peer-priority", "1", "--peer-elf", mosaic, "--peer-priority", "5",
        "--peer4-arg", "unix", "--out", dest)
    manifest = json.loads((dest / "manifest.json").read_text())
    roles = ("compositor", "xwayland", "tiles", "broker", "mosaic")
    sources = (comp, server, tiles, broker, mosaic)
    slots = manifest["slots"]
    if len(slots) != len(roles):
        raise ValueError("expected five packaged roles")
    for slot, elf in zip(slots, sources):
        if sha(elf) != slot["elf_sha256"] or sha(dest / slot["elf"]) != slot["elf_sha256"]:
            raise ValueError("packaged ELF identity changed")
    result = {"role_order": roles, "elf_sha256": dict(zip(roles, map(sha, sources))), "server_image": image,
              "kernel_sha256": sha(dest / "kernel.elf"), "server_argv": list(SERVER_ARGS)}
    (out / "workload.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("server", type=Path, help="xkb-build.py output containing Xwayland.sha256")
    p.add_argument("out", type=Path, help="fresh package workspace")
    p.add_argument("lock", type=Path, help="shared serial build/guest lock")
    p.add_argument("--boot-utc-seconds", type=int, required=True, help="declared reproducible guest time seed")
    args = p.parse_args()
    os.umask(0o077)
    with args.lock.open("a") as shared:
        fcntl.flock(shared, fcntl.LOCK_EX)
        package(args.server.resolve(), args.out.resolve(), args.boot_utc_seconds)
