#!/bin/sh
# Usage from any directory: sh path/to/build.sh OUT
set -eu
[ "$#" -eq 1 ] || { echo "usage: $0 OUT" >&2; exit 2; }
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
sdk="$here/../../sdk/native-aarch64"
mkdir -p "$1"
out=$(CDPATH= cd -- "$1" && pwd)
for app in sensor heavy late fault; do
    sh "$sdk/build.sh" "$here/$app.c" "$out/$app.elf"
done
CARGO_TARGET_DIR="$out/cargo" sh "$sdk/build-rust.sh" "$here/heartbeat/Cargo.toml" "$out/heartbeat.elf"
cp "$here/policy.txt" "$out/policy.txt"
sed 's/^task heavy burn burn_entry 600 110 15 30$/task heavy burn burn_entry 600 110 15 off/' \
    "$out/policy.txt" > "$out/unmonitored.txt"
cp "$here/scale20.txt" "$out/scale20.txt"
(cd "$out" && sha256sum sensor.elf heavy.elf late.elf fault.elf heartbeat.elf > apps.sha256)
