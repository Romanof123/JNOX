#!/usr/bin/env python3
"""Capture one extension guest. Set EXTENSION_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("extension_witness", "EXTENSION_CAPTURE_DIR"))
