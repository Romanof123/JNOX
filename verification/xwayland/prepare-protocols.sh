#!/bin/sh
# Prepare an isolated fresh target prefix and a patched copy of the pinned source.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cache=$(realpath "$1")
release=$(realpath "$2")
dest=$(realpath -m "$3")
mkdir "$dest"
prefix="$dest/target"
mkdir -p "$prefix/include" "$prefix/share/pkgconfig"
( cd "$cache" && echo '6a6764500eb8171d3cd6d793c09902974f8ea87bd20c68d9eac5f1eeaf3fc407  x11proto-dev_2024.1-1_all.deb' | sha256sum -c - )
( cd "$release" && echo '737e612ca36bbdf415a911644eb7592cf9389846847b47fa46dc705bd754d2d7  xwayland-24.1.6.tar.xz' | sha256sum -c - )
dpkg-deb -x "$cache/x11proto-dev_2024.1-1_all.deb" "$dest/xorgproto"
cp -R "$dest/xorgproto/usr/include/." "$prefix/include/"
for pc in "$dest"/xorgproto/usr/share/pkgconfig/*.pc; do
    # Upstream's architecture-independent metadata is relocatable at this depth.
    sed 's|^prefix=/usr$|prefix=${pcfiledir}/../..|' "$pc" > "$prefix/share/pkgconfig/$(basename "$pc")"
done
tar xf "$release/xwayland-24.1.6.tar.xz" -C "$dest"
patch -d "$dest/xwayland-24.1.6" -p1 < "$here/jnox-server.patch"
