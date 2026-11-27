#!/bin/bash
# Run from the Git source checkout in WSL. Argument is a fresh attempt directory.
set -euo pipefail
lab=$(realpath "${JNOX_LAB_ROOT:?set the shared local build root}")
inputs=$(realpath "${JNOX_INPUT_ROOT:?set the pinned archive directory}")
out=$(realpath -m "${1:?use a fresh output directory}")
source_git=(git)
if [[ -n ${JNOX_GIT_DIR:-} ]]; then source_git+=(--git-dir="$JNOX_GIT_DIR"); fi
revision=$("${source_git[@]}" rev-parse --verify "${2:-HEAD}^{commit}")
server_options=()
if (( $# > 2 )); then
    if [[ $# != 6 || $3 != --selected-xkm ]]; then
        echo "usage: server-lab.sh OUT REV [--selected-xkm INPUT_RS XKBCOMP_DEB LIBXKBFILE_DEB]" >&2
        exit 2
    fi
    server_options=(--selected-xkm "$(realpath "$4")" "$(realpath "$5")" "$(realpath "$6")")
fi
exec 9>"$lab/run.lock"
flock -x 9
mkdir "$out"
exec > >(tee "$out/build.log") 2>&1
mkdir "$out/src"
printf '%s\n' "$revision" > "$out/source-commit"
"${source_git[@]}" archive "$revision" verification/xwayland sdk/native-aarch64 | tar xf - -C "$out/src"
driver="$out/src/verification/xwayland"
export MESON="${MESON:-meson}"
export JNOX_GRAPHICS_RUNTIME="${JNOX_GRAPHICS_RUNTIME:-$lab/graphics-runtime}"
wl="${JNOX_WAYLAND_WORK:-$lab/wayland}"
rt="${JNOX_WAYLAND_RUNTIME:-$lab/wayland-runtime}"
font="${JNOX_FONT_PREFIX:-$lab/font-prefix}"
pin() { printf '%s  %s\n' "$1" "$2" | sha256sum -c -; }
pin 395577d9df9bbecf31f2841f491d18086e07a187da60cf6ed63be91c968c4a0f "$wl/target/lib/libwayland-client.a"
pin 9c4de749fbe57a99c81cd3693584a263cd02029d6bd466e44da4cb94745aaedb "$wl/target/lib/libffi.a"
pin 817c6c30cbd87face94251f5542a2cd55132293fe23baf32178ff93ff8792090 "$wl/host/bin/wayland-scanner"
pin 767f2debf49b48fd9c2160840e3c54d957eca1c3ecb9d07a9dd6423dadf8aea8 "$rt/target/lib/libjnox-wayland-os.a"
pin 36752e71bb20296c02dedcaa7fbdfd7dce869a20b4ea4cf72d8f02831bb73b6e "$rt/runtime.o"
pin 89cf9ffa6bf9f88e9e04caf9c275951645a1b9951b2ef5c781e4e964e76e5a87 "$rt/start.o"
pin 49f12564c105d16e0f9ff821b3b415f08a8a6f308ce94aa37a6af5d05cfbe185 "$font/lib/libXfont2.a"
pin 31b3480f7bee59b0d17da9e2db808c0c713dc78fbe2fbe725d3e2a6b64e93e2f "$font/lib/libfontenc.a"
pin ea51270835a8c27b8ec7c9e0831fdf55dfb0b54212fbb9db793594001a957fa7 "$font/lib/libz.a"
os="$out/os-prefix"
mkdir -p "$os/lib"
cp -R "$rt/target/include" "$os/include"
ln -s "$rt/target/lib/libc.a" "$os/lib/libc.a"
auth=()
case "${JNOX_NATIVE_XAUTHORITY:-0}" in
    0) ;;
    1) auth+=(--native-xauthority) ;;
    *) echo "JNOX_NATIVE_XAUTHORITY must be 0 or 1" >&2; exit 2 ;;
esac
python3 "$out/src/sdk/native-aarch64/graphics/os/install.py" --prefix "$os" "${auth[@]}"
pin a6f665483ac749136e97dce505ae8fbe4a52b58deae89f102c2200405706b091 "$os/lib/libjnox-wayland-os.a"
sh "$driver/xcvt/build.sh" "$inputs/libxcvt-0.1.3.tar.xz" "$out/xcvt" "$out/xcvt-prefix"
python3 "$driver/xkb-build.py" --build-executable "${server_options[@]}" "${auth[@]}" --cache "$inputs" \
  --out "$out/server" --runtime "$rt" --runtime-profile-sha256 "${JNOX_WAYLAND_PROFILE_SHA256:-00266859edb773b5fcad350d35d3e3b7f58621599014b666cc8e8ed308081643}" \
  --protocols "$font" --wayland-work "$wl" --server "$inputs/xwayland-24.1.6.tar.xz" \
  --xcvt-prefix "$out/xcvt-prefix" --os-prefix "$os" --libmd-source "$inputs/libmd_1.1.0.orig.tar.xz"
