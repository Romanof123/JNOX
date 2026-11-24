#!/usr/bin/env python3
"""Capture one EEVDF guest. Set EEVDF_CAPTURE_DIR to a new path."""
import os
import sys

from qemu_capture import main


if __name__ == "__main__":
    sys.exit(main("eevdf_witness", "EEVDF_CAPTURE_DIR", trace_symbol="eevdf_updates",
                  require_trace=os.environ.get("JNOX_CAPTURE_PROFILE") == "eevdf-dynamic"))
