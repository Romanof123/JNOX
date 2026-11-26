#!/bin/sh
# Build a Cargo package containing one no_std native application binary.
set -eu
bao_gateway=
local_socket=0
pubsub=0
pipe_define=
shell_define=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --native-shell) shell_define=-DJNOX_SDK_NATIVE_SHELL; pipe_define=-DJNOX_SDK_PIPES ;;
        --pipes) pipe_define=-DJNOX_SDK_PIPES ;;
        --bao-gateway) bao_gateway=jnox-native/bao-gateway ;;
        --local-socket) local_socket=1 ;;
        --pubsub) pubsub=1 ;;
        *) break ;;
    esac
    shift
done
if [ "$#" -ne 2 ]; then
    echo "usage: $0 [--pipes|--native-shell] [--local-socket] [--pubsub] [--bao-gateway] Cargo.toml OUTPUT.elf" >&2
    exit 2
fi
sdk=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
clang --target=aarch64-none-elf -I"$sdk/include" -c "$sdk/start.S" -o "$tmp/start.o"
clang ${pipe_define:+"$pipe_define"} ${shell_define:+"$shell_define"} --target=aarch64-none-elf -std=c11 -O2 -ffreestanding -nostdinc \
    -fno-pic -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
    -mgeneral-regs-only -I"$sdk/include" -c "$sdk/io.c" -o "$tmp/io.o"
manifest=$1
out=$2
set --
if [ "$local_socket" = 1 ]; then
    clang --target=aarch64-none-elf -std=c11 -O2 -ffreestanding -nostdinc \
        -fno-pic -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
        -mgeneral-regs-only -mno-outline-atomics -I"$sdk/include" \
        -c "$sdk/local_socket.c" -o "$tmp/local_socket.o"
    set -- -C link-arg="$tmp/local_socket.o" -C link-arg=--wrap=read \
        -C link-arg=--wrap=write -C link-arg=--wrap=close
fi
if [ "$pubsub" = 1 ]; then
    clang --target=aarch64-none-elf -std=c11 -O2 -ffreestanding -nostdinc \
        -fno-pic -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
        -mgeneral-regs-only -mno-outline-atomics -I"$sdk/include" \
        -c "$sdk/pubsub.c" -o "$tmp/pubsub.o"
    set -- "$@" -C link-arg="$tmp/pubsub.o"
fi
cargo rustc ${bao_gateway:+--features "$bao_gateway"} --manifest-path "$manifest" --release --target aarch64-unknown-none-softfloat \
    --message-format=json-render-diagnostics -- -C panic=abort -C relocation-model=static \
    -C link-arg="$tmp/start.o" -C link-arg="$tmp/io.o" -C link-arg=-T"$sdk/app.ld" \
    -C link-arg=-z -C link-arg=max-page-size=4096 -C link-arg=--build-id=none "$@" > "$tmp/artifacts"
python3 - "$tmp/artifacts" "$out" <<'PY'
import json
import shutil
import sys

with open(sys.argv[1]) as messages:
    images = [msg["executable"] for line in messages if
              (msg := json.loads(line)).get("executable") and "bin" in msg["target"]["kind"]]
if len(images) != 1:
    sys.exit("expected one native application binary")
shutil.copyfile(images[0], sys.argv[2])
PY