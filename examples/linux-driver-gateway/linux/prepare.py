#!/usr/bin/env python3
"""Prepare the unchanged Linux driver guest and the gateway userspace, once for both generations."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / "tools"))
from verify_linux_virtio_net_baseline import INIT_PREFIX, prepare, require, save, sha


INIT = INIT_PREFIX + """generation=
for arg in $($bb cat /proc/cmdline); do
    case "$arg" in
        jnox.generation=*)
            [ -z "$generation" ] || exit 7
            generation=${arg#jnox.generation=}
            ;;
    esac
done
[ -n "$generation" ] || exit 7
exec /gateway "$generation"
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--intake", required=True, type=Path, help="directory with the two pinned Alpine APKs")
    parser.add_argument("--out", required=True, type=Path, help="new output directory")
    parser.add_argument("--cc", default="aarch64-linux-gnu-gcc")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    compiler = shutil.which(args.cc)
    require(compiler, "AArch64 static cross compiler missing")
    endpoint = HERE / "endpoint.c"
    program = out / "gateway"
    flags = ["-std=c11", "-Os", "-static", "-s", "-Wall", "-Wextra", "-Werror", "-Wl,--build-id=none"]
    subprocess.run([compiler, *flags, str(endpoint), "-o", str(program)], check=True)
    data = program.read_bytes()
    require(data.startswith(b"\x7fELF\x02\x01") and data[18:20] == b"\xb7\x00", "expected an AArch64 ELF")
    operands = prepare(args.intake, out, init_script=INIT, service_files={"gateway": data})
    operands["gateway"] = {
        "source_sha256": sha(endpoint.read_bytes()), "elf_sha256": sha(data), "flags": flags,
        "compiler": subprocess.check_output([compiler, "--version"], text=True).splitlines()[0],
    }
    save(out / "operands.json", operands)
    print("Prepared Linux gateway kernel and initramfs for both generations")


if __name__ == "__main__":
    main()
