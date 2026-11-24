#!/usr/bin/env python3
"""Capture one cbs-wake guest. Set CBS_WAKE_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("cbs_wake_witness", "CBS_WAKE_CAPTURE_DIR"))
