#!/bin/bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
lab=/home/ywcho/jnox406-lab
out="$lab/448-font-xkb-$1"
mkdir "$out"
exec > >(tee "$out/run.log") 2>&1
exec 9>"$lab/run.lock"
flock -x 9
export PATH="/home/ywcho/.cargo/bin:$PATH" CARGO_BUILD_JOBS=1
export JNOX_GRAPHICS_RUNTIME="$lab/448-graphics-93e41a9/attempts/attempt05/runtime"
set -x
pkg="$lab/448-graphics-93e41a9/package-checkout"
guest() {
    name=$1
    elf=$2
    shift 2
    sha256sum "$elf" > "$out/$name.elf.sha256"
    python3 "$pkg/tools/build_native_el0.py" --board virt --policy fifo --console-write 0 --elf "$elf" --priority 4 --out "$out/$name"
    cmp "$elf" "$out/$name/user.elf"
    python3 "$repo/examples/native-pixman/capture.py" "$out/$name/kernel.elf" "$out/$name.log" 1 "$@"
    python3 "$repo/verification/xwayland/check-dep-guest.py" "$out/$name.log" "$name"
}
if [[ ${2:-both} != font ]]; then
    guest xkb "$lab/448-xkb-first-02/build/probe-archive.elf" --symbol XkbRF_Create --symbol XkbRF_Free --symbol XkbModIndexText
fi
if [[ ${2:-both} != xkb ]]; then
    cp -a "$lab/448-xkb-first-02/protocols" "$out/prefix"
    tr -d '\r' < "$repo/verification/xwayland/graphics-cc.py" > "$out/cc"
    chmod +x "$out/cc"
    TARGET_CC="$out/cc" sh "$repo/verification/xwayland/font-static.sh" /mnt/d/codex/jnox448-font-cache "$out/font-build" "$out/prefix"
    guest font "$out/font-build/font-builtins-probe.elf" --symbol BuiltinFileOpen --symbol inflate
fi
