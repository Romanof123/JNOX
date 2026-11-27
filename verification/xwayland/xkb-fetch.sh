#!/bin/sh
set -eu
pins=$(realpath "$(dirname "$0")/xkb-sources.sha256")
mkdir -p "$1"
cd "$1"
while read -r hash name; do
    case "$name" in
        libX11-*|libXau-*|libxkbfile-*) origin=https://www.x.org/releases/individual/lib ;;
        *) origin=https://xcb.freedesktop.org/dist ;;
    esac
    if [ ! -e "$name" ]; then
        tmp=$(mktemp)
        trap 'rm -f "$tmp"' EXIT
        curl -fLsS "$origin/$name" -o "$tmp"
        printf '%s  %s\n' "$hash" "$tmp" | sha256sum -c -
        mv "$tmp" "$name"
        trap - EXIT
    fi
done < "$pins"
sha256sum -c "$pins"
