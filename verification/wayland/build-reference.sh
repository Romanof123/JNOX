#!/bin/sh
# Run on amd64 Ubuntu with cc, Python 3 and the scanner's shared dependencies.
set -eu
if [ "$#" -ne 2 ]; then
    echo "usage: sh build-reference.sh SYSROOT NEW_OUTPUT_DIRECTORY" >&2
    exit 2
fi
src=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(realpath "$1")
out=$(realpath -m "$2")
python3 "$src/inputs.py" check "$root"
mkdir "$out"
scanner="$root/usr/bin/wayland-scanner"
xdg="$root/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml"
libs="$root/usr/lib/x86_64-linux-gnu"
"$scanner" client-header "$root/usr/share/wayland/wayland.xml" "$out/wayland-client-protocol.h"
cmp "$out/wayland-client-protocol.h" "$root/usr/include/wayland-client-protocol.h"
"$scanner" client-header "$xdg" "$out/xdg-shell-client-protocol.h"
"$scanner" private-code "$xdg" "$out/xdg-shell-protocol.c"
cc -std=c11 -Wall -Wextra -Werror -Wno-unused-parameter -O2 -I"$out" -I"$root/usr/include" \
    "$src/reference-client.c" "$out/xdg-shell-protocol.c" \
    "$libs/libwayland-client.so.0.20.0" "$libs/libxkbcommon.so.0.0.0" \
    -Wl,-rpath,"$libs" -o "$out/reference-client"
(cd "$out" && sha256sum wayland-client-protocol.h xdg-shell-client-protocol.h \
    xdg-shell-protocol.c reference-client > generated.sha256)
