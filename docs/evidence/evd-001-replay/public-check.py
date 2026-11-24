from pathlib import Path
import json
import subprocess
import sys

base = Path(__file__).resolve().parent
records = {}
for name, arg in [("positive", ["a/target/aarch64-unknown-none/release/jnox-kernel"]),
                  ("negative", ["mutant/target/aarch64-unknown-none/release/jnox-kernel", "--runs", "1", "--timeout", "1"])]:
    cmd = [sys.executable, "a/tools/qemu_boot.py", "--kernel", *arg]
    run = subprocess.run(cmd, cwd=base, capture_output=True, timeout=20)
    records[name] = dict(args=cmd, actual_exit_code=run.returncode,
                         stdout=run.stdout.decode(), stderr=run.stderr.decode())
(base / "public.json").write_text(json.dumps(records, indent=2))
for name, result in records.items():
    print(name, result["actual_exit_code"], result["stdout"].splitlines()[-1],
          result["stderr"].strip())
assert records["positive"]["actual_exit_code"] == 0 and records["negative"]["actual_exit_code"] == 1
