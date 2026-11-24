#!/usr/bin/env python3
"""Capture one multilevel guest. Set ORDINARY_MULTILEVEL_CAPTURE_DIR to a new path."""
import os
import sys

from qemu_capture import journal_location, main


if __name__ == "__main__":
    journal = "ordinary_multilevel_witness"
    extra = ()
    if os.environ.get("JNOX_CAPTURE_PROFILE") in (
        "mlq-blocked", "mlq-exited", "mlq-saturated", "mlfq-delayed"
    ):
        kernel = sys.argv[sys.argv.index("-kernel") + 1]
        if journal_location(kernel, journal, optional=True) is None:
            journal = "multilevel_terminal_ctl"
        else:
            extra = ("multilevel_terminal_ctl",)
    sys.exit(main(journal, "ORDINARY_MULTILEVEL_CAPTURE_DIR", "fixed_runtime_idle", extra_symbols=extra))
