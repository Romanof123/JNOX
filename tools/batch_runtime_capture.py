#!/usr/bin/env python3
"""Use qemu_boot.py --qemu tools/batch_runtime_capture.py on Linux."""
from qemu_capture import main

if __name__ == "__main__":
    raise SystemExit(main("jnox_batch_events", "JNOX_BATCH_CAPTURE_DIR", idle_symbol="jnox_batch_idle"))
