#!/usr/bin/env bash
# Run only inside the assigned serialized guest slot, from a matching clean Linux checkout.
set -euo pipefail
archive=$(realpath "${1:?Wayland 1.23.1 archive}")
qa=$(realpath "${2:?frozen #444 observer directory}")
runtime=$(realpath "${3:?verified Picolibc graphics runtime}")
out=$(realpath -m "${4:?new output directory}")
lock=${5:?path to the common guest run lock}
exec 9>"$lock"
flock 9
repo=$(git rev-parse --show-toplevel)
cd "$repo"
test -z "$(git status --porcelain --untracked-files=no)"
mkdir "$out"
exec > >(tee "$out/gate.log") 2>&1
export LC_ALL=C PYTHONDONTWRITEBYTECODE=1 CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$out/target"
git show -s --format='%H %T' > "$out/source-identity.txt"
here="$repo/examples/native-xwayland-shm"
sdk="$repo/sdk/native-aarch64"
qemu=$(realpath "$(command -v qemu-system-aarch64)")
sha256sum "$qemu" > "$out/qemu.sha256"
"$qemu" --version
python3 "$here/check_dispatch.py"
mkdir "$out/observer"
cp "$qa"/{replay.py,capture.py,audit.py,contract.json} "$out/observer/"
cp tools/native_amc_roots.py "$out/observer/"
(cd "$out/observer" && sha256sum -c "$here/observer.sha256")
python3 "$here/extract_upstream_dup.py" "$archive" "$out"
printf '#include "upstream-dup.inc"\n#define JNOX_FRAME_DUP upstream_outgoing_fd\n#include "%s/main.c"\n' "$here" > "$out/guest.c"
sh "$sdk/build.sh" --local-socket "$out/guest.c" "$out/guest.elf"
sh "$sdk/build-rust.sh" services/socket/Cargo.toml "$out/broker.elf"
llvm-nm "$out/guest.elf" > "$out/guest.symbols"
grep -w wl_os_dupfd_cloexec "$out/guest.symbols"
pack=(python3 tools/build_native_el0.py --board virt --policy fifo)
"${pack[@]}" --elf "$out/guest.elf" --priority 7 --peer-elf "$out/guest.elf" --peer-arg d --peer-priority 5 \
    --peer-elf "$out/guest.elf" --peer2-arg a --peer-priority 4 --peer-elf "$out/broker.elf" --peer3-arg 7 --peer-priority 1 \
    --display-service 0 --socket-service 3 --shared-fd --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3 --out "$out/package"
python3 "$out/observer/capture.py" --package "$out/package" --out "$out/captures" --contract "$out/observer/contract.json" --qemu "$qemu"
python3 "$out/observer/audit.py" "$out/captures"
for run in cold-1 cold-2; do python3 "$here/check_dup.py" "$out/captures/$run/uart.bin" --mutations; done
python3 "$here/graphics_dup_gate.py" "$runtime" "$out" "$qemu"
sha256sum -c "$out/qemu.sha256"
