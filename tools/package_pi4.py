"""Stage a Raspberry Pi 4 FAT boot tree in a new host directory."""

import argparse
import hashlib
import json
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
from urllib.request import urlopen


BOARD = Path(__file__).resolve().parents[1] / "boards" / "pi4"


def stage(kernel, firmware_dir, staging):
    if not kernel.is_file():
        raise ValueError(f"expected a regular raw kernel image: {kernel}")
    image = kernel.read_bytes()
    if not image or image.startswith(b"\x7fELF"):
        raise ValueError("expected a nonempty raw kernel image")
    config = (BOARD / "config.txt").read_bytes().replace(b"\r\n", b"\n")
    files = {"config.txt": config, "kernel8.img": image}
    manifest = json.loads((BOARD / "firmware.json").read_text(encoding="utf-8"))
    base = f"https://raw.githubusercontent.com/raspberrypi/firmware/{manifest['commit']}/boot"
    for name, expected in manifest["files"].items():
        if firmware_dir is None:
            with urlopen(f"{base}/{name}", timeout=30) as response:
                data = response.read()
        else:
            data = (firmware_dir / name).read_bytes()
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"firmware SHA-256 mismatch: {name}")
        files[name] = data

    sums = "".join(f"{hashlib.sha256(files[name]).hexdigest()}  {name}\n"
                   for name in sorted(files))
    files["SHA256SUMS"] = sums.encode("ascii")
    for name, data in files.items():
        target = staging / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)


def package(kernel, output, firmware_dir):
    # Build beside the destination so publication uses same-filesystem renames.
    with TemporaryDirectory(dir=output.parent, prefix=".pi4-") as folder:
        staging = Path(folder)
        stage(kernel, firmware_dir, staging)
        entries = sorted(staging.iterdir(), key=lambda entry: entry.name == "SHA256SUMS")
        output.mkdir()  # Exclusive reservation on both POSIX and Windows.
        moved = []
        try:
            for entry in entries:
                entry.rename(output / entry.name)
                moved.append(entry)
        except BaseException:
            for entry in reversed(moved):
                (output / entry.name).rename(entry)
            output.rmdir()
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", required=True, type=Path, help="raw kernel8.img")
    parser.add_argument("--output", required=True, type=Path,
                        help="new directory under an existing parent")
    parser.add_argument("--firmware-dir", type=Path,
                        help="offline boot tree containing start4.elf and overlays/")
    args = parser.parse_args()
    try:
        package(args.kernel, args.output, args.firmware_dir)
    except (OSError, ValueError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(f"Staged Raspberry Pi 4 boot files in {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
