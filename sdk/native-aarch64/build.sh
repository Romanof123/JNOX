#!/bin/sh
# Usage: build.sh examples/portability/telemetry_monitor.c output.elf
set -eu
local_socket=0
pubsub=0
permit_codec=0
pipe_define=
shell_define=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --native-shell) shell_define=-DJNOX_SDK_NATIVE_SHELL; pipe_define=-DJNOX_SDK_PIPES ;;
        --pipes) pipe_define=-DJNOX_SDK_PIPES ;;
        --local-socket) local_socket=1 ;;
        --pubsub) pubsub=1 ;;
        --permit-codec) permit_codec=1 ;;
        *) break ;;
    esac
    shift
done
if [ "$#" -ne 2 ]; then
    echo "usage: $0 [--pipes|--native-shell] [--local-socket] [--pubsub] [--permit-codec] APP.c OUTPUT.elf" >&2
    exit 2
fi
sdk=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
app=$1
out=$2
set --
if [ "$local_socket" = 1 ]; then
    set -- "$sdk/local_socket.c" -Wl,--wrap=read,--wrap=write,--wrap=close
fi
if [ "$pubsub" = 1 ]; then set -- "$sdk/pubsub.c" "$@"; fi
if [ "$permit_codec" = 1 ]; then
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    target=aarch64-unknown-none-softfloat
    cargo build --manifest-path "$sdk/../native-safety-channel/c-api/Cargo.toml" \
        --frozen --release --target "$target" --target-dir "$tmp"
    # Keep the native errno fallback so even a stack-only C app has the ABI-required RW segment.
    set -- "$@" "$tmp/$target/release/libjnox_permit_codec.a" -Wl,--gc-sections,--undefined=__jnox_errno_location
fi
if [ -n "$shell_define" ]; then set -- "$shell_define" "$@"; fi
if [ -n "$pipe_define" ]; then set -- "$pipe_define" "$@"; fi
clang --target=aarch64-none-elf -fuse-ld=lld -std=c11 -O2 \
    -ffreestanding -nostdinc -nostdlib -static -fno-pic -fno-pie \
    -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
    -mgeneral-regs-only -mno-outline-atomics -I"$sdk/include" \
    -Wl,-T,"$sdk/app.ld",-z,max-page-size=4096,--build-id=none \
    "$sdk/start.S" "$sdk/io.c" "$app" "$@" -o "$out"
