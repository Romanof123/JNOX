#!/bin/sh
# Usage: build.sh examples/portability/telemetry_monitor.c output.elf
set -eu
local_socket=0
pubsub=0
pipe_define=
shell_define=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --native-shell) shell_define=-DJNOX_SDK_NATIVE_SHELL; pipe_define=-DJNOX_SDK_PIPES ;;
        --pipes) pipe_define=-DJNOX_SDK_PIPES ;;
        --local-socket) local_socket=1 ;;
        --pubsub) pubsub=1 ;;
        *) break ;;
    esac
    shift
done
if [ "$#" -ne 2 ]; then
    echo "usage: $0 [--pipes|--native-shell] [--local-socket] [--pubsub] APP.c OUTPUT.elf" >&2
    exit 2
fi
sdk=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
app=$1
out=$2
set --
if [ "$local_socket" = 1 ]; then
    set -- "$sdk/local_socket.c" -Wl,--wrap=read,--wrap=write,--wrap=close,--wrap=fcntl
fi
if [ "$pubsub" = 1 ]; then set -- "$sdk/pubsub.c" "$@"; fi
if [ -n "$shell_define" ]; then set -- "$shell_define" "$@"; fi
if [ -n "$pipe_define" ]; then set -- "$pipe_define" "$@"; fi
exec clang --target=aarch64-none-elf -fuse-ld=lld -std=c11 -O2 \
    -ffreestanding -nostdinc -nostdlib -static -fno-pic -fno-pie \
    -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
    -mgeneral-regs-only -mno-outline-atomics -I"$sdk/include" \
    -Wl,-T,"$sdk/app.ld",-z,max-page-size=4096,--build-id=none \
    "$sdk/start.S" "$sdk/io.c" "$app" "$@" -o "$out"
