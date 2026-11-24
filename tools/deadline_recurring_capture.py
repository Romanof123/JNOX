#!/usr/bin/env python3
"""QEMU wrapper for a finite recurring deadline capture (Linux/WSL)."""
from qemu_capture import main

if __name__ == "__main__":
    raise SystemExit(main(
        "deadline_recurring_returns", "JNOX_RECURRING_CAPTURE",
        idle_symbol="deadline_recurring_terminal",
        extra_symbols=("deadline_recurring_launch", "ordinary_deadline_row_count",
                       "ordinary_deadline_rows", "deadline_restore_count"),
    ))
