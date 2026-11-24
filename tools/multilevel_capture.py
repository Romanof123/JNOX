#!/usr/bin/env python3
"""Capture one multilevel guest. Set MULTILEVEL_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("multilevel_witness", "MULTILEVEL_CAPTURE_DIR"))
