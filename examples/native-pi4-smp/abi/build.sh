#!/bin/sh
# Build ordinary SDK images with an additional separately assembled leaf.
set -eu
if [ "$#" -ne 1 ]; then
    echo "usage: sh $0 OUTPUT_DIRECTORY" >&2
    exit 2
fi
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
sdk="$here/../../../sdk/native-aarch64"
mkdir -p "$1"
leaf=$(mktemp "$1/.abi-work.XXXXXX.o")
trap 'rm -f "$leaf"' 0
clang --target=aarch64-none-elf -march=armv8-a+simd -c "$here/work.S" -o "$leaf"
for job in odd even; do
    case "$job" in odd) role=0 ;; even) role=1 ;; esac
    clang --target=aarch64-none-elf -fuse-ld=lld -std=c11 -O2 \
        -ffreestanding -nostdinc -nostdlib -static -fno-pic -fno-pie \
        -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
        -mgeneral-regs-only -mno-outline-atomics -I"$sdk/include" "-DROLE=$role" \
        -Wl,-T,"$sdk/app.ld",-z,max-page-size=4096,--build-id=none \
        "$sdk/start.S" "$sdk/io.c" "$here/client.c" "$leaf" -o "$1/$job.elf"
done
