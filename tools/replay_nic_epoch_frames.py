"""Check saved raw Ethernet frames from two native NIC ownership epochs."""
import argparse
from pathlib import Path

HOST = bytes.fromhex("020000000001")
GUEST = bytes.fromhex("525400123456")
BROADCAST = b"\xff" * 6
PROTOCOL = b"\x88\xb5JNDM"


def check_exchange(request, response, epoch):
    if len(request) != 60 or len(response) != 60:
        raise ValueError(f"epoch {epoch}: expected 60-byte Ethernet frames")
    if request[:20] != BROADCAST + HOST + PROTOCOL + bytes([epoch, 1]):
        raise ValueError(f"epoch {epoch}: invalid challenge header")
    if response[:20] != HOST + GUEST + PROTOCOL + bytes([epoch, 2]):
        raise ValueError(f"epoch {epoch}: invalid reply header")
    if request[20:] != response[20:]:
        raise ValueError(f"epoch {epoch}: reply payload differs")
    return request[20:36]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("primary_challenge", "primary_reply", "spare_challenge", "spare_reply"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    try:
        first = check_exchange(args.primary_challenge.read_bytes(), args.primary_reply.read_bytes(), 1)
        second = check_exchange(args.spare_challenge.read_bytes(), args.spare_reply.read_bytes(), 2)
        if first == second:
            raise ValueError("challenge nonce reused across ownership epochs")
    except (OSError, ValueError) as exc:
        parser.exit(1, f"{exc}\n")
    print("Two distinct wire challenges matched their epoch replies.")


if __name__ == "__main__":
    main()
