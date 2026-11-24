#!/usr/bin/env python3
"""Capture one EEVDF guest. Set EEVDF_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("eevdf_witness", "EEVDF_CAPTURE_DIR"))
