#!/bin/sh
# The input ELF is the installed archive builder's roundtrip.elf.
set -eu
repo=$(cd "$(dirname "$0")/../../.." && pwd)
elf=$(realpath "$1")
out=$(realpath -m "$2")
shift 2
env_connect=
case $# in
    0) ;;
    1) test "$1" = --env-connect || exit 2; env_connect=--env-connect ;;
    *) exit 2 ;;
esac
mkdir "$out"
cd "$repo"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$out/target"
readelf -hW "$elf" | grep -q AArch64
llvm-nm --defined-only "$elf" >"$out/client.symbols"
for symbol in wl_display_connect_to_fd wl_display_roundtrip wl_connection_flush ffi_call; do
    grep -Eq " [TtWw] ${symbol}$" "$out/client.symbols"
done
sh sdk/native-aarch64/build-rust.sh --local-socket services/wayland/Cargo.toml "$out/compositor.elf"
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml "$out/broker.elf"
set --
if [ -n "$env_connect" ]; then
    for symbol in wl_display_connect wl_display_get_fd getenv; do
        grep -Eq " [TtWw] ${symbol}$" "$out/client.symbols"
    done
    set -- --env 1:WAYLAND_DISPLAY=/wayland-0
fi
python3 tools/build_native_el0.py "$@" --board virt --policy fifo --display-service 0 --socket-service 3 \
    --console-write 1 --shared-fd --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3 \
    --elf "$out/compositor.elf" --priority 7 --peer-elf "$elf" --peer-priority 5 \
    --peer-elf "$elf" --peer2-arg idle --peer-priority 4 --peer-elf "$out/broker.elf" \
    --peer3-arg 7 --peer-priority 1 --out "$out/package"
cmp "$elf" "$out/package/native_peer.elf"
(cd "$out/package" && sha256sum *.elf manifest.json) >"$out/SHA256SUMS"
python3 verification/xwayland/client/registry-boot.py "$out/package" "$out/observation" --profile output ${env_connect:+"$env_connect"}
