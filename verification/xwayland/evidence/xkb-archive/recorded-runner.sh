#!/usr/bin/env bash
# Execute under WSL only after PM releases the exclusive build gate.
set -euo pipefail
base=/home/ywcho/jnox406-lab
src=/mnt/d/codex/.jnox-work/xwayland-source-release
rev=2f8e3071bf5f57a51b1cfd79fabdd729947deb4d
input=/mnt/d/codex/jnox448-xwayland
runtime="$base/448-graphics-93e41a9/attempts/attempt05/runtime"
out="$base/448-xkb-first-${1:?supply fresh numeric attempt}"
[[ $1 =~ ^[0-9]+$ ]]
exec 9>"$base/run.lock"
flock -n 9
mkdir "$out" "$out/src" "$out/protocols"
exec > >(tee "$out/build.log") 2>&1
git --git-dir=/mnt/d/codex/.jnox-work/policy-v1/.git archive "$rev" verification/xwayland | tar -x -C "$out/src"
printf '%s\n' "$rev" > "$out/source-commit"
cp -a "$input/deps-03/target/." "$out/protocols/"
test -s "$out/protocols/share/pkgconfig/xproto.pc"
cd "$out/src"
export LC_ALL=C PYTHONDONTWRITEBYTECODE=1
export MESON="$input/configure-tools/root/usr/bin/meson"
export PYTHONPATH="$input/configure-tools/root/usr/lib/python3/dist-packages"
python3 verification/xwayland/xkb-build.py --cache /mnt/d/codex/.jnox-work \
  --runtime "$runtime" --protocols "$out/protocols" --out "$out/build"
sha256sum "$out/build/libxkbfile.a" "$out/build/probe-archive.elf" "$out/build/probe-archive.map" > "$out/artifacts.sha256"
