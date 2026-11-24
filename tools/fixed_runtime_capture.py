#!/usr/bin/env python3
"""Capture fixed application observations and verify the terminal idle PC in Linux QEMU."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("fixed_app_observations", "FIXED_RUNTIME_CAPTURE_DIR", "fixed_runtime_idle"))
