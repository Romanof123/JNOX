#!/bin/sh
# Usage: TARGET_CC=/absolute/jnox-cc sh font-static.sh CACHE NEW_WORK TARGET_PREFIX
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cache=$(realpath "$1")
work=$(realpath -m "$2")
prefix=$(realpath "$3")
: "${TARGET_CC:?set to the JNOX AArch64 C compiler/linker wrapper with a complete target C runtime and libm}"
test -x "$TARGET_CC"
mkdir "$work"
cd "$work"
cat > sources.sha256 <<EOF
9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23  zlib-1.3.1.tar.gz
7b02c3d405236e0d86806b1de9d6868fe60c313628b38350b032914aa4fd14c6  libfontenc-1.1.8.tar.xz
8b7b82fdeba48769b69433e8e3fbb984a5f6bf368b0d5f47abeec49de3e58efb  libXfont2-2.0.7.tar.xz
EOF
(cd "$cache" && sha256sum -c "$work/sources.sha256")
for name in zlib-1.3.1.tar.gz libfontenc-1.1.8.tar.xz libXfont2-2.0.7.tar.xz; do
    tar -xf "$cache/$name"
done
unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH LIBRARY_PATH CFLAGS CPPFLAGS LDFLAGS LIBS PKG_CONFIG_SYSROOT_DIR
export CC="$TARGET_CC" AR="${TARGET_AR:-llvm-ar}" RANLIB="${TARGET_RANLIB:-llvm-ranlib}"
export PKG_CONFIG_PATH='' PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig:$prefix/share/pkgconfig"
export CPPFLAGS="-I$prefix/include" LDFLAGS="-L$prefix/lib -static"
pkg-config --exists 'fontsproto >= 2.1.3' xproto xtrans
for pkg in xproto fontsproto xtrans; do
    pkg-config --modversion "$pkg"
    pkg-config --variable=pcfiledir "$pkg"
done > protocol-inputs.txt
sha256sum "$TARGET_CC" > compiler-wrapper.sha256
guess=$(find libXfont2-2.0.7 -name config.guess -type f)
test -f "$guess"
build=$(sh "$guess")
"$CC" -c "$here/font-fd.c" -o font-fd.o
"$AR" rcs "$prefix/lib/libjnox-font-fd.a" font-fd.o
export LIBS="-ljnox-font-fd"
host=aarch64-none-elf
(cd zlib-1.3.1 && CHOST="$host" ./configure --static --prefix="$prefix" &&
    make -j1 libz.a && make -j1 install)
(cd libfontenc-1.1.8 && ./configure --build="$build" --host="$host" --prefix="$prefix" \
    --with-fontrootdir=/usr/share/fonts/X11 --with-encodingsdir=/usr/share/fonts/X11/encodings \
    --disable-shared --enable-static && make -j1 install)
(cd libXfont2-2.0.7 && ./configure --build="$build" --host="$host" --prefix="$prefix" \
    --disable-shared --enable-static --enable-builtins --enable-pcfformat --disable-freetype \
    --disable-fc --disable-bdfformat --disable-snfformat --without-bzip2 && make -j1 install-libLTLIBRARIES install-libXfontincludeHEADERS install-pkgconfigDATA)
for archive in libz.a libfontenc.a libXfont2.a; do
    readelf -hW "$prefix/lib/$archive" > "$archive.readelf"
    awk '/Machine:/ {n++; if ($NF != "AArch64") exit 1} END {if (!n) exit 1}' "$archive.readelf"
done
pkg-config --static --libs xfont2 > static-link-flags.txt
# Static linkage permits the probe to exercise the archive's internal built-in FPE reader.
"$CC" -DJNOX_FONT_UART -static -I"$prefix/include" -IlibXfont2-2.0.7/include \
    -IlibXfont2-2.0.7/src/builtins "$here/font-builtins-probe.c" \
    -L"$prefix/lib" $(pkg-config --static --libs xfont2) -ljnox-font-fd -o font-builtins-probe.elf
readelf -hW -lW -dW font-builtins-probe.elf > font-builtins-probe.readelf
grep -q 'Machine:.*AArch64' font-builtins-probe.readelf
if grep -Eq ' INTERP |\(NEEDED\)' font-builtins-probe.readelf; then exit 1; fi
sha256sum "$prefix/lib/libz.a" "$prefix/lib/libfontenc.a" "$prefix/lib/libXfont2.a" \
    font-builtins-probe.elf > artifacts.sha256
