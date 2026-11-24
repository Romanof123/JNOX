#!/usr/bin/env python3
"""Freeze first-attempt feedback workload RAM, trace, UART, exceptions and PC."""
import sys

from qemu_capture import main

if __name__ == "__main__":
    sys.exit(main("mlfq_wake_witness", "MLFQ_WAKE_CAPTURE_DIR",
                  trace_symbol="ordinary_multilevel_witness", freeze=True))
