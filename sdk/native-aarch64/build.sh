#!/bin/sh
# Usage: build.sh examples/portability/telemetry_monitor.c output.elf
set -eu
if [ "$#" -ne 2 ]; then
    echo "usage: $0 APP.c OUTPUT.elf" >&2
    exit 2
fi
sdk=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec clang --target=aarch64-none-elf -fuse-ld=lld -std=c11 -O2 \
    -ffreestanding -nostdinc -nostdlib -static -fno-pic -fno-pie \
    -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
    -mgeneral-regs-only -mno-outline-atomics -I"$sdk/include" \
    -Wl,-T,"$sdk/app.ld",-z,max-page-size=4096,--build-id=none \
    "$sdk/start.S" "$sdk/io.c" "$1" -o "$2"
