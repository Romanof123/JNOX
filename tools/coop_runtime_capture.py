#!/usr/bin/env python3
"""Capture the ordinary cooperative app journal and verify terminal idle in QMP."""
from qemu_capture import main

if __name__ == "__main__":
    raise SystemExit(main("coop_app_observations", "COOP_CAPTURE_DIR", idle_symbol="fixed_runtime_idle"))
