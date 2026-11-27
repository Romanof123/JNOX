#!/bin/sh
# Build the five-process AArch64 guest for CHG-343. Run from any directory.
set -eu
if [ "$#" -ne 1 ]; then
    echo "usage: sh build-tablet-recovery.sh NEW_OUTPUT_DIRECTORY" >&2
    exit 2
fi
mkdir -- "$1"
out=$(CDPATH= cd -- "$1" && pwd)
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$root"
export CARGO_BUILD_JOBS=1
sdk=sdk/native-aarch64
git rev-parse HEAD > "$out/source-commit"
sh "$sdk/build-rust.sh" --local-socket --features restart-probe services/wayland/Cargo.toml "$out/compositor.elf"
sh "$sdk/build-rust.sh" services/socket/Cargo.toml "$out/socket.elf"
sh "$sdk/build.sh" --local-socket examples/native-wayland-restart/client.c "$out/client.elf"
sh "$sdk/build.sh" examples/native-wayland-restart/control.c "$out/control.elf"

python3 tools/build_native_el0.py --board virt --policy fifo \
    --elf "$out/compositor.elf" --arg restart-primary --priority 7 \
    --peer-elf "$out/client.elf" --peer-arg reconnect-pointer --peer-priority 5 \
    --peer-elf "$out/compositor.elf" --peer2-arg restart-spare --peer-priority 6 \
    --peer-elf "$out/socket.elf" --peer3-arg 7 --peer-priority 1 \
    --peer-elf "$out/control.elf" --peer-priority 8 \
    --display-service 0 --display-standby 2 --socket-service 3 --pointer-tablet \
    --shared-fd --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3 --out "$out/package"
cd "$out"
sha256sum compositor.elf socket.elf client.elf control.elf package/kernel.elf package/manifest.json > SHA256SUMS
