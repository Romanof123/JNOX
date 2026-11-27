#!/bin/sh
# Run under the shared serial build/guest lock. OUT must be a fresh directory.
set -eu
out=$(realpath -m "${1:?fresh output directory required}")
mkdir "$out"
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/../../.." && pwd)
cd "$repo"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$out/cargo"
sh sdk/native-aarch64/build-rust.sh --tmpfs "$here/rust-guest/Cargo.toml" "$out/caller.elf"
sh sdk/native-aarch64/build-rust.sh --features tmpfs services/socket/Cargo.toml "$out/broker.elf"
python3 tools/build_native_el0.py --board virt --policy fifo --elf "$out/caller.elf" --arg writer --priority 7 \
    --peer-elf "$out/caller.elf" --peer-arg reader --peer-priority 6 \
    --peer-elf "$out/caller.elf" --peer2-arg denied --peer-priority 5 \
    --peer-elf "$out/broker.elf" --peer3-arg 7 --peer-priority 8 \
    --display-service 0 --socket-service 3 --shared-fd --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3 \
    --tmpfs 0:rw --tmpfs 1:ro --out "$out/package"
python3 - "$out" <<'PY'
from pathlib import Path
import shutil
import sys
sys.path.insert(0, "examples/native-xwayland-shm")
from tmpfs_boot import capture

out = Path(sys.argv[1])
qemu = shutil.which("qemu-system-aarch64")
if not qemu:
    raise SystemExit("qemu-system-aarch64 is required")
roles = [("Rust writer", 90), ("Rust reader", 90), ("ungranted Rust client", 90), ("broker", 0)]
capture(qemu, out/"package/kernel.elf", out/"caller.elf", out/"broker.elf", out, "--tmpfs-roles", roles)
PY
