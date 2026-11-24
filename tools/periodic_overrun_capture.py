#!/usr/bin/env python3
"""Capture one periodic-overrun guest. Set PERIODIC_OVERRUN_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("periodic_overrun_witness", "PERIODIC_OVERRUN_CAPTURE_DIR"))
