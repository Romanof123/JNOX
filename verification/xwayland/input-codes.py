"""Stage the numeric Wayland input event definitions for the JNOX target."""
import argparse
import hashlib
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("source", type=Path)
p.add_argument("include_dir", type=Path)
a = p.parse_args()
data = a.source.read_bytes()
if hashlib.sha256(data).hexdigest() != "6b0bed55d5bb59958a04bfe49bbcee1a83d50148f9069cb47303f547edf79167":
    p.error("expected input-event-codes.h from linux-libc-dev 5.15.0-204.214")
dest = a.include_dir / "linux/input-event-codes.h"
dest.parent.mkdir(parents=True, exist_ok=True)
dest.write_bytes(data)
