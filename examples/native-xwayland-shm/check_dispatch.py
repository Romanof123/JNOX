"""Check shared-FD dispatch remains inside its published SVC set and clear of tablet."""
import argparse
from pathlib import Path
import re
import subprocess

root = Path(__file__).absolute().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--tablet-ref", help="published tablet Git revision in this clone")
args = parser.parse_args()
kernel = (root/"kernel/src/native_el0.rs").read_text()
match = re.search(r"op @ \(([^)]+)\) => shared_fd::direct", kernel)
if not match:
    raise ValueError("could not identify shared-FD dispatcher")
calls = set()
for span in match[1].split("|"):
    first, *last = span.strip().split("..=")
    calls.update(range(int(first), int(last[0] if last else first) + 1))
if calls != {180,181,182,183,*range(187,197)}:
    raise ValueError(f"shared-FD dispatcher changed published allocation: {sorted(calls)}")
header = (root/"sdk/native-aarch64/include/jnox/shared_fd_abi.h").read_text()
if not re.search(r"^#define JNOX_CALL_SHM_DUP 188$", header, re.M):
    raise ValueError("SDK duplicated descriptors outside shared-FD SVC 188")
if args.tablet_ref:
    reference = subprocess.check_output(
        ["git", "-c", "safe.directory="+root.as_posix(), "-C", root, "show",
         args.tablet_ref+":kernel/src/native_el0.rs"], text=True)
    if not re.search(r"\b197\s*=>\s*tablet::call", reference):
        raise ValueError("tablet reference does not contain its published 197 dispatch")
print("Shared-FD SVC set preserved; 188 belongs to shared FD, 197 excluded.")
