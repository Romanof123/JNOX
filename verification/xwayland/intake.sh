#!/bin/sh
# Run on a Linux host with curl, dpkg-deb, GNU binutils and coreutils.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "${1:?supply an intake directory}"
cd "$1"
while read -r hash name; do
    case "$name" in
        xwayland-*) base=https://www.x.org/releases/individual/xserver ;;
        *) base=https://deb.debian.org/debian/pool/main/x/xwayland ;;
    esac
    if [ ! -f "$name" ]; then
        tmp=$(mktemp "./$name.XXXXXX")
        trap 'rm -f "$tmp"' EXIT
        curl -fLsS "$base/$name" -o "$tmp"
        echo "$hash  $tmp" | sha256sum -c -
        mv "$tmp" "$name"
        trap - EXIT
    fi
done < "$here/downloads.sha256"
sha256sum -c "$here/downloads.sha256"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
dpkg-deb -x xwayland_24.1.6-1_arm64.deb "$work"
bin="$work/usr/bin/Xwayland"
echo "eb75578b20b3429e721149e3c8b625278c9f91c3f41974a930f2f6b4066e2def  $bin" | sha256sum -c -
LC_ALL=C readelf -hW -lW -dW "$bin" > elf.txt
cat elf.txt
