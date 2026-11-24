#!/usr/bin/env python3
"""Capture one priority-resume guest. Set PRIORITY_RESUME_CAPTURE_DIR to a new path."""
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("priority_resume_witness", "PRIORITY_RESUME_CAPTURE_DIR"))
