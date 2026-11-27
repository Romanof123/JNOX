#!/bin/sh
# SOURCE_ARCHIVE NEW_WORK NEW_TARGET_PREFIX
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
archive=$(realpath "$1")
work=$(realpath -m "$2")
prefix=$(realpath -m "$3")
: "${JNOX_GRAPHICS_RUNTIME:?verified native graphics runtime directory required}"
mkdir "$work" "$prefix"
cd "$work"
printf '%s  %s\n' a929998a8767de7dfa36d6da4751cdbeef34ed630714f2f4a767b351f2442e01 "$archive" | sha256sum -c -
tar -xf "$archive"
src=libxcvt-0.1.3
printf '%s  %s\n' 4849851554bdeda9a2a85ce93946b786605e009685941b8b907a9a36b02ae068 "$src/COPYING" | sha256sum -c -
unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH
cc="$here/../graphics-cc.py"
python3 "$cc" -Os -I"$src/include" -c "$src/lib/libxcvt.c" -o libxcvt.o
mkdir -p "$prefix/lib/pkgconfig" "$prefix/include/libxcvt" "$prefix/share/licenses/libxcvt"
llvm-ar rcs "$prefix/lib/libxcvt.a" libxcvt.o
cp "$src/include/libxcvt/"*.h "$prefix/include/libxcvt/"
cp "$src/COPYING" "$prefix/share/licenses/libxcvt/"
cat > "$prefix/lib/pkgconfig/libxcvt.pc" <<'PC'
prefix=${pcfiledir}/../..
Name: libxcvt
Description: VESA CVT mode timing generator
Version: 0.1.3
Libs: -L${prefix}/lib -lxcvt
Cflags: -I${prefix}/include
PC
export PKG_CONFIG_PATH='' PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
python3 "$cc" "$here/probe.c" $(pkg-config --cflags --static --libs libxcvt) -o probe.elf
readelf -hW libxcvt.o | grep -q 'Machine:.*AArch64'
readelf -hW -lW -dW probe.elf > probe.readelf
grep -q 'Machine:.*AArch64' probe.readelf
if grep -Eq ' INTERP |\(NEEDED\)' probe.readelf; then exit 1; fi
sha256sum "$prefix/lib/libxcvt.a" probe.elf > artifacts.sha256
