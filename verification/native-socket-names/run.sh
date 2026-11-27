#!/bin/sh
# Linux host with the ordinary native packaging and QEMU tools.
set -eu
repo=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
out=$(realpath -m "${1:?provide a new results directory}")
mkdir "$out"
cd "$repo"
sdk=sdk/native-aarch64
probe=verification/native-socket-names/probe.c
rustc --edition=2021 --test services/socket/src/local.rs -o "$out/local-tests"
"$out/local-tests" > "$out/local-tests.log"
sh "$sdk/build-rust.sh" services/socket/Cargo.toml "$out/broker.elf"
sh "$sdk/build.sh" --local-socket "$probe" "$out/sdk.elf"
cases=sdk
if [ -n "${JNOX_GRAPHICS_RUNTIME:-}" ]; then
    # The overlay installer writes only into this disposable prefix.
    runtime=$(realpath "$JNOX_GRAPHICS_RUNTIME")
    prefix="$out/picolibc-prefix"
    mkdir -p "$prefix/lib"
    cp -R "$runtime/target/include" "$prefix/include"
    ln -s "$runtime/target/lib/libc.a" "$prefix/lib/libc.a"
    python3 "$sdk/graphics/os/install.py" --prefix "$prefix"
    cc=verification/xwayland/graphics-cc.py
    python3 "$cc" -I"$prefix/include/jnox-wayland" -c "$probe" -o "$out/picolibc.o"
    python3 "$cc" "$out/picolibc.o" "$prefix/lib/libjnox-wayland-os.a" -o "$out/picolibc.elf"
    cases="sdk picolibc"
fi
for variant in $cases; do
    pkg="$out/$variant"
    python3 tools/build_native_el0.py --board virt --policy fifo --elf "$out/$variant.elf" --priority 4 \
        --peer-elf "$out/broker.elf" --peer-arg 1 --peer-priority 1 --socket-service 1 --ipc-route 0:1 --out "$pkg"
    cmp "$pkg/user.elf" "$out/$variant.elf"
    cmp "$pkg/native_peer.elf" "$out/broker.elf"
    python3 examples/native-pixman/capture.py "$pkg/kernel.elf" "$pkg/uart.log" 2 \
        --symbol getsockname --symbol getpeername --symbol listen --symbol accept
    PYTHONPATH=tools python3 - "$pkg/uart.log" <<'PY'
import sys
from pathlib import Path
from verify_native_display import observed_exits
path = Path(sys.argv[1])
exits = sorted(observed_exits(path.read_bytes()))
if exits != [(0, 77), (1, 0)]:
    raise SystemExit(f"{path}: unexpected or duplicate process exits {exits}")
print(path.parent.name, exits)
PY
done
