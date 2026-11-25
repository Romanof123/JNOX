#!/bin/sh
# Usage: build.sh examples/portability/telemetry_monitor.c output.elf
set -eu
local_socket=0
if [ "${1:-}" = "--local-socket" ]; then
    local_socket=1
    shift
fi
if [ "$#" -ne 2 ]; then
    echo "usage: $0 [--local-socket] APP.c OUTPUT.elf" >&2
    exit 2
fi
sdk=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
app=$1
out=$2
set --
if [ "$local_socket" = 1 ]; then
    set -- "$sdk/local_socket.c" -Wl,--wrap=read,--wrap=write,--wrap=close
fi
exec clang --target=aarch64-none-elf -fuse-ld=lld -std=c11 -O2 \
    -ffreestanding -nostdinc -nostdlib -static -fno-pic -fno-pie \
    -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
    -mgeneral-regs-only -mno-outline-atomics -I"$sdk/include" \
    -Wl,-T,"$sdk/app.ld",-z,max-page-size=4096,--build-id=none \
    "$sdk/start.S" "$sdk/io.c" "$app" "$@" -o "$out"
