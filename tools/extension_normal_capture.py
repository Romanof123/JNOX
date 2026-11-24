#!/usr/bin/env python3
"""Capture the scheduling witness and nine worker-authored records."""
import sys
from qemu_capture import main

if __name__ == "__main__":
    sys.exit(main("extension_witness", "EXTENSION_CAPTURE_DIR",
                  trace_symbol="extension_normal_results", require_trace=True))
