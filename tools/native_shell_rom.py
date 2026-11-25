"""Canonical immutable input file image for the native shell package."""
import hashlib
from pathlib import Path
import struct


def basename(value):
    if (not 1 <= len(value) <= 31 or value in (".", "..") or not value.isascii()
            or not all(c.isalnum() or c in "._-" for c in value)):
        raise ValueError("expected a 1..31 byte ASCII basename using letters, digits, dot, dash or underscore")
    return value


def shell_package(children, files):
    names = {0: "sh"}
    for entry in children:
        slot, sep, value = entry.partition(":")
        if not sep or slot not in ("1", "2") or int(slot) in names:
            raise ValueError("each child slot 1 and 2 must be named exactly once")
        name = basename(value)
        if name in names.values():
            raise ValueError("duplicate executable basename")
        names[int(slot)] = name
    if len(names) != 3:
        raise ValueError("expected two named child slots")
    if len(files) > 8:
        raise ValueError("ROM supports at most eight files")
    paths = {}
    for entry in files:
        value, sep, path = entry.partition("=")
        name = basename(value)
        if not sep or not path or name in paths:
            raise ValueError("expected a unique ROM basename and explicit host path")
        paths[name] = Path(path)
    image = bytearray(8192)
    struct.pack_into("<8sI", image, 0, b"JNOXROM1", len(paths))
    offset = 400
    records = []
    for index, name in enumerate(sorted(paths)):
        source = paths[name]
        if not source.is_file():
            raise ValueError(f"ROM input is not a regular file: {source}")
        with source.open("rb") as handle:
            data = handle.read(len(image) - offset + 1)
        if offset + len(data) > len(image):
            raise ValueError("ROM file data exceed 7792 bytes")
        struct.pack_into("<32sII", image, 16 + index * 48, name.encode("ascii"), offset, len(data))
        image[offset:offset + len(data)] = data
        records.append(dict(name=name, offset=offset, length=len(data), sha256=hashlib.sha256(data).hexdigest()))
        offset += len(data)
    return [names[i] for i in range(3)], records, image
