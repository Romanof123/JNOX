"""List unresolved imports from the selected Xwayland target's real link command."""

import argparse
import json
from pathlib import Path
import re
import shlex
import subprocess
import tempfile


def report(build):
    target = "hw/xwayland/Xwayland"
    commands = subprocess.check_output(["ninja", "-t", "commands", target], cwd=build, text=True)
    args = shlex.split(commands.splitlines()[-1])
    output = args.index("-o") + 1
    if args[output] != target or not any("clang" in Path(arg).name for arg in args[:3]):
        raise ValueError("unexpected linker invocation for the configured target")
    with tempfile.TemporaryDirectory(dir=build) as scratch:
        args[output] = str(Path(scratch) / "Xwayland")
        args.append("-Wl,--error-limit=0")
        result = subprocess.run(args, cwd=build, capture_output=True, text=True)
    errors = [line.removeprefix("ld.lld: error: ") for line in result.stderr.splitlines()
              if line.startswith("ld.lld: error: ")]
    if result.returncode and not errors:
        raise RuntimeError("link failed without LLD diagnostics; inspect the primary build log")
    symbols = set()
    for error in errors:
        match = re.fullmatch(r"undefined symbol: (\S+)", error)
        if not match:
            raise RuntimeError(f"additional linker error: {error}")
        symbols.add(match[1])
    return {"target": target, "link_status": result.returncode,
            "count": len(symbols), "undefined_symbols": sorted(symbols)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("build", type=Path, help="configured Meson build with compiled server objects")
    print(json.dumps(report(parser.parse_args().build.resolve()), indent=2))
