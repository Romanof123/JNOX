"""Rehash altered copies of a sealed placement first and require semantic rejection."""
import argparse
import json
from pathlib import Path
import re
import shutil
import tempfile

from native_energy_replay import inspect, NONE
from ordinary_amc_capture import sha


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("case")
    p.add_argument("capture", type=Path)
    p.add_argument("--session-sha256", required=True)
    p.add_argument("--elf-sha256", required=True)
    p.add_argument("--manifest-sha256", required=True)
    p.add_argument("--trace", action="store_true")
    args = p.parse_args()
    original = inspect(args)
    if args.case == "missing-limit":
        p.error("controls require an original guest capture")
    mutations = {"stale absolute deadline": (8, lambda v: v-1),
                 "wrong owner": (10, lambda v: 0 if v == NONE else 1-v),
                 "invented or shifted dispatch": (18, lambda v: 0 if v == NONE else v+1)}
    for name, (index, alter) in mutations.items():
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shutil.copytree(args.capture, root, dirs_exist_ok=True)
            uart = root / "uart.bin"
            def change(match):
                fields = match[1].split()
                fields[index] = str(alter(int(fields[index]))).encode()
                return b"NATIVE SMP ENERGY " + b" ".join(fields)
            changed, count = re.subn(rb"(?m)^NATIVE SMP ENERGY ([0-9 ]+)", change, uart.read_bytes())
            if count != 1:
                raise ValueError("expected exactly one ENERGY record")
            uart.write_bytes(changed)
            receipt = root / "session.json"
            record = json.loads(receipt.read_bytes())
            record["files"]["uart.bin"] = sha(uart)
            receipt.write_text(json.dumps(record) + "\n", encoding="utf8")
            altered = argparse.Namespace(**vars(args))
            altered.capture, altered.session_sha256 = root, sha(receipt)
            try:
                inspect(altered)
            except ValueError as exc:
                print(f"{name}: rejected ({exc})")
            else:
                raise ValueError(f"accepted altered capture: {name}")
    print("original disposition", original["disposition"])


if __name__ == "__main__":
    main()
