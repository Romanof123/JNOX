#!/usr/bin/env python3
"""Capture one smp guest. Set SMP_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("smp_audit", "SMP_CAPTURE_DIR"))
