#!/usr/bin/env python3
"""Capture one power guest. Set POWER_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("power_witness", "POWER_CAPTURE_DIR"))
