#!/bin/sh
# Capture one supplied ELF with GNU timeout, Linux QEMU and the GNU/LLVM tools.
# All launch artifacts, including an incomplete capture, stay in the new directory.
set -eu
if [ "$#" -ne 5 ]; then
    echo "usage: sh capture_partition_notice.sh P|Q short|long ELF EXPECTED_SHA256 NEW_DIRECTORY" >&2
    exit 1
fi
table=$1
mode=$2
elf=$3
expected=$4
out=$5
case "$table/$mode" in P/short|P/long|Q/short|Q/long) ;; *) exit 1 ;; esac
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -- "$out"
cp -- "$elf" "$out/kernel.elf"
cd -- "$out"
sha256sum kernel.elf > sha256
if [ "$(cut -d' ' -f1 sha256)" != "$expected" ]; then
    echo "supplied ELF differs from expected SHA-256" >&2
    exit 1
fi
if ! git -C "$repo" rev-parse HEAD 'HEAD^{tree}' > checker-revision 2>/dev/null; then
    # WSL can be reading a worktree whose .git points to a Windows gitdir.
    git.exe -C "$(wslpath -w "$repo")" rev-parse HEAD 'HEAD^{tree}' > checker-revision
fi
printf 'table=%s\nmode=%s\n' "$table" "$mode" > selector
qemu-system-aarch64 --version > qemu-version
if command -v llvm-objdump >/dev/null 2>&1; then
    llvm-objdump -d kernel.elf > disassembly
else
    aarch64-linux-gnu-objdump -d kernel.elf > disassembly
fi
set -- qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
    -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio \
    -d int -D exceptions.log -kernel kernel.elf
printf '%s\n' "$@" > qemu-argv

# Parked guests emit STOP and wait indefinitely. Keep the first UART and
# exception log on every exit. Limit files before launching the one guest.
status=0
(
    ulimit -f 1024
    exec timeout -k 2s 8s "$@"
) </dev/null > uart 2> qemu-stderr || status=$?
printf '%s\n' "$status" > host-status
if [ "$status" -ne 124 ]; then
    echo "guest exited before the scheduled host stop (status $status)" >&2
    exit 1
fi
sha256sum -c sha256 >&2
score=0
python3 "$repo/tools/ordinary_partition_account.py" uart --origin guest \
    --table "$table" --case "$mode" --elf kernel.elf \
    --elf-sha256 "$expected" --exceptions exceptions.log > score.json 2> score-stderr || score=$?
printf '%s\n' "$score" > score-status
exit "$score"
