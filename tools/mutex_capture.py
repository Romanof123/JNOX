#!/usr/bin/env python3
"""Capture one mutex guest. Set MUTEX_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("mutex_witness", "MUTEX_CAPTURE_DIR"))
