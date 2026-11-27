"""Xauthority wire encoding and Mosaic's window-relative RGB test pattern."""
import struct

AUTH_NAME = b"MIT-MAGIC-COOKIE-1"
GEOMETRY = (32, 32, 240, 160)


def auth_record(family, host, number, cookie):
    fields = (host, number, AUTH_NAME, cookie)
    return struct.pack(">H", family) + b"".join(struct.pack(">H", len(f)) + f for f in fields)


def expected_scene(changed):
    _, _, width, height = GEOMETRY
    rgb = bytearray(bytes.fromhex("182840") * width * height)
    for x, y, w, h, color in ((16, 16, 64, 128, "20b8a0"), (96, 16, 128, 48, "f4b644"),
                              (96, 80, 128, 64, "498cf0" if changed else "e85c50")):
        for row in range(y, y+h):
            start = (row * width + x) * 3
            rgb[start:start+w*3] = bytes.fromhex(color) * w
    return rgb
