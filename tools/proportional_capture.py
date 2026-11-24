#!/usr/bin/env python3
"""Capture one proportional guest. Set PROPORTIONAL_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("proportional_witness", "PROPORTIONAL_CAPTURE_DIR"))
