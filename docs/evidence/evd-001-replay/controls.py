"""Run public runner against synthetic host-process UART controls in WSL."""
from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile

base = Path(__file__).resolve().parent
okay = b"JNOX M1 BOOT\nJNOX M1 READY\n"
cases = [
    ("full", okay, b"", "stay", 0),
    ("missing", okay.splitlines(keepends=True)[0], b"", "stay", 1),
    ("reversed", b"JNOX M1 READY\nJNOX M1 BOOT\n", b"", "stay", 1),
    ("duplicate", okay + okay, b"", "stay", 1),
    ("extra", okay + b"!", b"", "stay", 1),
    ("partial", okay[:-1], b"", "stay", 1),
    ("stderr_only", b"", okay, "stay", 1),
    ("exit_early", okay, b"", "exit", 1),
    ("during_quiet", okay, b"", "delayed", 1),
    ("on_shutdown", okay, b"", "shutdown", 1)
]
standin = """#!/usr/bin/env python3
import os, signal, sys, time
behavior = os.environ['CONTROL_BEHAVIOR']
if behavior == 'shutdown':
    def on_stop(sig, frame):
        os.write(1, b'?')
        sys.exit(0)
    signal.signal(signal.SIGTERM, on_stop)
os.write(1, bytes.fromhex(os.environ['CONTROL_UART']))
os.write(2, bytes.fromhex(os.environ['CONTROL_STDERR']))
if behavior == 'delayed':
    time.sleep(0.04)
    os.write(1, b'?')
if behavior != 'exit':
    time.sleep(5)
"""
records = {}
with tempfile.TemporaryDirectory() as tmp:
    exe = Path(tmp) / "fake-qemu"
    exe.write_text(standin)
    exe.chmod(0o755)
    for label, uart, err, behavior, expected in cases:
        cmd = [sys.executable, "a/tools/qemu_boot.py", "--kernel", "a/target/aarch64-unknown-none/release/jnox-kernel",
               "--qemu", str(exe), "--runs", "1", "--timeout", "0.4"]
        env = dict(os.environ, CONTROL_UART=uart.hex(), CONTROL_STDERR=err.hex(), CONTROL_BEHAVIOR=behavior)
        p = subprocess.run(cmd, cwd=base, env=env, capture_output=True, timeout=5)
        records[label] = dict(expected=expected, actual=p.returncode,
                              stdout=p.stdout.decode(), stderr=p.stderr.decode())
(base / "controls.json").write_text(json.dumps(records, indent=2))
print({key:r["actual"] for key,r in records.items()})
raise SystemExit(0 if all(r["actual"] == r["expected"] for r in records.values()) else 1)
