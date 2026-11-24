#!/usr/bin/env python3
"""Capture fixed application observations and verify the terminal idle PC in Linux QEMU."""
import os
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("fixed_app_observations", "FIXED_RUNTIME_CAPTURE_DIR",
                  idle_symbol="fixed_runtime_idle", trace_symbol="fixed_runtime_trace",
                  require_trace=os.environ.get("JNOX_FIXED_TRACE") == "1"))
