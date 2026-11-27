#!/bin/sh
# Usage: MESON=... sh pixman.sh ARCHIVE NEW_WORK PREVIOUS_CONFIGURE/jnox.ini TARGET_PREFIX
set -eu
archive=$(realpath "$1")
work=$(realpath -m "$2")
cross=$(realpath "$3")
prefix=$(realpath "$4")
echo "89a4c1e1e45e0b23dffe708202cb2eaffde0fe3727d7692b2e1739fec78a7dac  $archive" | sha256sum -c -
mkdir "$work"
tar xf "$archive" -C "$work"
export PKG_CONFIG_PATH=
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig:$prefix/share/pkgconfig"
unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH CFLAGS CPPFLAGS LDFLAGS
"${MESON:-meson}" setup "$work/build" "$work/pixman-0.44.0" --cross-file "$cross" --wrap-mode=nofallback --prefix "$prefix" --libdir lib -Ddefault_library=static -Dauto_features=disabled
ninja -C "$work/build" -j1 install
lib="$prefix/lib/libpixman-1.a"
readelf -h "$lib" > "$work/archive-headers.txt"
awk '/Machine:/ { count++; if ($NF != "AArch64") exit 1 } END { if (!count) exit 1 }' "$work/archive-headers.txt"
