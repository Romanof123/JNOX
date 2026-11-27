"""Prepare private credentials and check individual observations from the guest profile."""
import argparse
import os
from pathlib import Path
import re
import secrets
import sys

from mosaic_fixture import GEOMETRY, auth_record, expected_scene


def node_label(value):
    if not re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?", value):
        raise argparse.ArgumentTypeError("expected a 1-63 byte native package node label")
    return value.encode("ascii")


def prepare(args):
    server = secrets.token_bytes(16)
    wrong = secrets.token_bytes(16)
    while wrong == server:
        wrong = secrets.token_bytes(16)
    client_node = args.client_node or args.node
    files = (("server.auth", args.node, server), ("client.auth", client_node, server),
             ("wrong.auth", client_node, wrong))
    args.directory.mkdir(mode=0o700)
    root = os.open(args.directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for name, node, cookie in files:
            fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         mode=0o600, dir_fd=root)
            with os.fdopen(fd, "wb") as output:
                output.write(auth_record(256, node, b"0", cookie))
    finally:
        os.close(root)


def read_fixture(path):
    raw = path.read_bytes()
    # prepare() writes exactly one FamilyLocal record. Check its framing before
    # using its secret to check UART for unwanted disclosure.
    if len(raw) < 2 or raw[:2] != b"\x01\x00":
        raise ValueError("unexpected private authority format")
    fields = []
    offset = 2
    for _ in range(4):
        if offset + 2 > len(raw):
            raise ValueError("truncated private authority")
        size = int.from_bytes(raw[offset:offset + 2], "big")
        offset += 2
        if offset + size > len(raw):
            raise ValueError("truncated private authority field")
        fields.append(raw[offset:offset + size])
        offset += size
    node, number, name, cookie = fields
    if (not node or number != b"0" or name != b"MIT-MAGIC-COOKIE-1" or
            len(cookie) != 16 or offset != len(raw)):
        raise ValueError("unexpected private authority record")
    return cookie


def check_exit(args):
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
    from verify_native_display import observed_exits
    good = read_fixture(args.private / "server.auth")
    right = read_fixture(args.private / "client.auth")
    wrong = read_fixture(args.private / "wrong.auth")
    if good != right or good == wrong:
        raise ValueError("fixture credentials do not form a matching and mismatching pair")
    uart = args.uart.read_bytes()
    for secret in (good, wrong):
        if secret in uart or secret.hex().encode() in uart.lower():
            raise ValueError("credential appeared in UART; keep this capture private")
    exits = [status for pid, status in observed_exits(uart) if pid == 4]
    expected = 114 if args.mode == "wrong" else 0
    if exits != [expected]:
        raise ValueError(f"expected a single Mosaic PID 4 exit {expected}, got {exits}")
    print(f"Mosaic PID 4 exit {expected}")


def check_frame(args):
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
    from verify_native_display import PPM
    data = args.ppm.read_bytes()
    header = PPM.match(data)
    if not header or tuple(map(int, header.groups())) != (320, 240):
        raise ValueError("expected a 320 x 240 QMP P6 screendump")
    pixels = data[header.end():]
    x, y = args.origin
    _, _, w, h = GEOMETRY
    if len(pixels) != 320 * 240 * 3 or not (0 <= x <= 320-w and 0 <= y <= 240-h):
        raise ValueError("frame size or observed window origin is invalid")
    start = 3 * (320*y + x)
    crop = b"".join(pixels[i:i + 3*w] for i in range(start, start + 3*320*h, 3*320))
    if crop != expected_scene(args.state == "blue"):
        raise ValueError("frame does not match the complete Mosaic window")
    print(f"Mosaic {w} x {h} {args.state} frame at ({x}, {y})")


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    actions = cli.add_subparsers(dest="action", required=True)
    gen = actions.add_parser("prepare", help="create a new private directory with binary fixtures")
    gen.add_argument("directory", type=Path)
    gen.add_argument("--node", type=node_label, required=True)
    gen.add_argument("--client-node", type=node_label)
    gen.set_defaults(run=prepare)
    exit_check = actions.add_parser("check-exit")
    exit_check.add_argument("mode", choices=("right", "wrong"))
    exit_check.add_argument("uart", type=Path)
    exit_check.add_argument("--private", type=Path, required=True)
    exit_check.set_defaults(run=check_exit)
    frame = actions.add_parser("check-frame")
    frame.add_argument("state", choices=("coral", "blue"))
    frame.add_argument("ppm", type=Path)
    frame.add_argument("--origin", type=int, nargs=2, default=GEOMETRY[:2], metavar=("X", "Y"))
    frame.set_defaults(run=check_frame)
    args = cli.parse_args()
    try:
        args.run(args)
    except (OSError, ValueError) as error:
        cli.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
