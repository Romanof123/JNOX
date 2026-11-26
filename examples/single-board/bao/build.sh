#!/usr/bin/env bash
set -euo pipefail
gateway=0
if [[ ${1:-} == --gateway ]]; then
    gateway=1
    shift
fi
if [[ $# != 4 ]]; then
    echo "usage: bash build.sh [--gateway] BAO_SOURCE LINUX_BIN JNOX_ELF EMPTY_BUILD_DIR" >&2
    exit 2
fi
export LC_ALL=C
here=$(cd "$(dirname "$0")" && pwd)
src=$(realpath "$1")
linux=$(realpath "$2")
jnox=$(realpath "$3")
mkdir -p "$4"
out=$(realpath "$4")
if [[ -n $(ls -A "$out") || "$out" =~ [[:space:]] ]]; then
    echo "Bao's build requires an empty output directory with a whitespace-free path" >&2
    exit 2
fi
pin=0af4a1ab558ad60c9658af4f43756c4536dd3141
cross=${CROSS_COMPILE:-aarch64-linux-gnu-}
git clone --local --no-hardlinks --no-checkout "$src" "$out/src"
git -C "$out/src" checkout --quiet --detach "$pin"
cd "$out/src"
test "$(git rev-parse HEAD)" = "$pin"
plat=src/platform/qemu-aarch64-virt
python3 - "$plat" <<'PY'
from pathlib import Path
import sys

def change(path, old, new):
    text = path.read_text()
    if text.count(old) != 1:
        raise ValueError(f"unexpected pinned source in {path}: {old}")
    path.write_text(text.replace(old, new))

plat = Path(sys.argv[1])
change(plat / "virt_desc.c", ".cpu_num = 4,", ".cpu_num = 2,")
change(plat / "virt_desc.c", ".size = 0x100000000,", ".size = 0x20000000,")
change(plat / "platform.mk", "CPU:=cortex-a53", "CPU:=cortex-a72")
change(plat / "platform.mk", "GIC_VERSION:=GICV3", "GIC_VERSION:=GICV2")

# PSCI CPU_ON takes affinity fields. Bao's full MPIDR also contains RES1 bit 31.
change(Path("src/arch/armv8/armv8-a/cpu.c"),
       "psci_cpu_on(mpidr, load_addr, 0)", "psci_cpu_on(mpidr & MPIDR_AFF_MSK, load_addr, 0)")
# On this secure=off GICv2 board Bao enables Group 0 on the distributor
# and physical CPU interface. Assign SPIs to that group along with the PPIs.
change(Path("src/arch/armv8/gic.c"), "gicd->IGROUPR[i] = ~0U;", "gicd->IGROUPR[i] = 0U;")
PY
config=configs/domains
mkdir "$config"
if (( gateway )); then
    # Both Bao's host config generator and its target compiler read this file.
    { echo '#define JNOX_GATEWAY 1'; cat "$here/config.c"; } > "$config/config.c"
else
    cp "$here/config.c" "$config/config.c"
fi
cp "$linux" "$config/linux.bin"
entry=$("${cross}readelf" -h "$jnox" | awk '/Entry point address:/ {print $4}')
[[ "$entry" = 0x40200000 ]] || { echo "unexpected JNOX ELF entry $entry" >&2; exit 1; }
"${cross}objcopy" -O binary "$jnox" "$config/jnox.bin"
make -j1 PLATFORM=qemu-aarch64-virt CONFIG=domains CROSS_COMPILE="$cross"
linked=bin/qemu-aarch64-virt/domains
python3 "$here/verify_layout.py" "$linked" | tee "$out/layout.txt"
cp "$linked"/bao.{bin,elf} "$out/"
(cd "$out" && sha256sum bao.bin bao.elf src/configs/domains/{linux,jnox}.bin > SHA256SUMS)
if (( gateway )); then
    (cd "$out" && sha256sum src/configs/domains/config.c >> SHA256SUMS)
    printf '%s\n' 'profile=gateway JNOX_GATEWAY=1' \
        'shmem_id=0 machine_pa=0x5e000000 linux_ipa=0x0b000000 jnox_ipa=0x0b000000 size=0x1000 interrupts=0' \
        >> "$out/layout.txt"
fi
