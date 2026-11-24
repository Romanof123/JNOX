#!/bin/sh
# Build a Cargo package containing one no_std native application binary.
set -eu
if [ "$#" -ne 2 ]; then
    echo "usage: $0 Cargo.toml OUTPUT.elf" >&2
    exit 2
fi
sdk=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
clang --target=aarch64-none-elf -I"$sdk/include" -c "$sdk/start.S" -o "$tmp/start.o"
clang --target=aarch64-none-elf -std=c11 -O2 -ffreestanding -nostdinc \
    -fno-pic -fno-stack-protector -fno-unwind-tables -fno-asynchronous-unwind-tables \
    -mgeneral-regs-only -I"$sdk/include" -c "$sdk/io.c" -o "$tmp/io.o"
cargo rustc --manifest-path "$1" --release --target aarch64-unknown-none-softfloat \
    --message-format=json-render-diagnostics -- -C panic=abort -C relocation-model=static \
    -C link-arg="$tmp/start.o" -C link-arg="$tmp/io.o" -C link-arg=-T"$sdk/app.ld" \
    -C link-arg=-z -C link-arg=max-page-size=4096 -C link-arg=--build-id=none > "$tmp/artifacts"
python3 - "$tmp/artifacts" "$2" <<'PY'
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