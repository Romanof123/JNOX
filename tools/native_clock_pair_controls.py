#!/usr/bin/env python3
"""Run the #353 production C sampler controls on a Linux LP64 host (CC defaults to cc)."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SDK = ROOT / "sdk/native-aarch64"
code = (SDK / "io.c").read_text()
start = "/* All cache state is in the application's private, pristine-on-restart BSS. */"
end = "int clock_nanosleep("
if code.count(start) != 1 or code.count(end) != 1:
    raise SystemExit("sampler boundaries changed; inspect the production source")
sampler = code[code.index(start):code.index(end)]
counter = r'__asm__ volatile("isb\n\tmrs %0,cntpct_el0\n\tdmb ishld" : "=r"(now) :: "memory");'
if sampler.count(counter) != 1:
    raise SystemExit("counter operand changed; inspect the production source")

# Keep the actual loads, retry loop, cache, conversion and fallback calls.
# The host provides the privileged interface's operands at the SDK's real VA.
with tempfile.TemporaryDirectory() as tmp:
    work = Path(tmp)
    (work / "sampler.inc").write_text(sampler.replace(counter, "now = control_counter();"))
    binary = work / "clock-control"
    subprocess.run([os.environ.get("CC", "cc"), "-std=c11", "-O2", "-Wall", "-Wextra",
                    "-Werror", "-pthread", "-I", str(work), "-I", str(ROOT),
                    str(ROOT / "tools/native_clock_pair_controls.c"), "-o", str(binary)],
                   check=True, timeout=30)
    subprocess.run([str(binary)], check=True, timeout=10)
