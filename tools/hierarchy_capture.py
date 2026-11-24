#!/usr/bin/env python3
"""Capture one hierarchy guest. Set HIERARCHY_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("hierarchy_witness", "HIERARCHY_CAPTURE_DIR"))
