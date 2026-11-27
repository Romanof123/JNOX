#!/bin/sh
# Download a complete hash-checked source/host-tool intake into the chosen cache.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$1"
cd "$1"
while read -r digest name; do
    case "$name" in
        wayland-protocols_*) url=https://deb.debian.org/debian/pool/main/w/wayland-protocols ;;
        wayland_*) url=https://deb.debian.org/debian/pool/main/w/wayland ;;
        libffi_*) url=https://deb.debian.org/debian/pool/main/libf/libffi ;;
        libexpat1-dev_*) url=https://archive.ubuntu.com/ubuntu/pool/main/e/expat ;;
        *) exit 1 ;;
    esac
    if [ ! -f "$name" ]; then
        temporary=$(mktemp "./$name.XXXXXX")
        trap 'rm -f "$temporary"' EXIT
        curl -fLsS "$url/$name" -o "$temporary"
        echo "$digest  $temporary" | sha256sum -c -
        mv "$temporary" "$name"
        trap - EXIT
    fi
done < "$here/inputs.sha256"
sha256sum -c "$here/inputs.sha256"
