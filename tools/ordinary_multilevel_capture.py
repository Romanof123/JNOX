#!/usr/bin/env python3
"""Capture one multilevel guest. Set ORDINARY_MULTILEVEL_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("ordinary_multilevel_witness", "ORDINARY_MULTILEVEL_CAPTURE_DIR", "fixed_runtime_idle"))
