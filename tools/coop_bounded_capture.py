#!/usr/bin/env python3
"""Save raw single-CPU bounded FIFO evidence from the terminal idle guest."""
from qemu_capture import main

if __name__ == "__main__":
    raise SystemExit(main("coop_bounded_observations", "COOP_BOUNDED_CAPTURE_DIR",
                          idle_symbol="fixed_runtime_idle"))
