#!/usr/bin/env python3
"""Capture the strict MLQ admission result, including failed and partial runs."""
import json
import os
from pathlib import Path
import re
import struct
import sys

from mlq_receipt_check import check
from qemu_boot import TRANSCRIPTS
from qemu_capture import journal_location, main

DIRECTORY = "MLQ_ADMISSION_CAPTURE_DIR"


def capture():
    profile = os.environ["JNOX_CAPTURE_PROFILE"]
    if profile not in ("mlq-admit", "mlq-refuse"):
        raise ValueError("expected mlq-admit or mlq-refuse")
    kernel = sys.argv[sys.argv.index("-kernel") + 1]
    out = Path(os.environ[DIRECTORY])
    status = main("mlq_admission_witness", DIRECTORY,
                  trace_symbol="ordinary_multilevel_witness", freeze=True)
    data = (out / "audit.bin").read_bytes()
    words = struct.unpack("<30Q", data)
    states = ("reset", "admitted", "refused", "measured", "completed", "violation")
    report = {"profile": profile, "status": words[0], "stage": words[27],
              "state": states[words[0]] if words[0] < len(states) else "unknown",
              "pass": False}
    try:
        if status:
            raise ValueError(f"QEMU shutdown status {status}")
        if (out / "uart.bin").read_bytes().replace(b"\r\n", b"\n") != TRANSCRIPTS[profile]:
            raise ValueError("unexpected or incomplete UART")
        check(data, profile == "mlq-refuse")
        if profile == "mlq-admit":
            address, size = journal_location(kernel, "fixed_runtime_idle")
            pc = re.search(r"\bPC=([0-9a-fA-F]+)\b", (out / "registers.txt").read_text())
            if not pc or not address <= int(pc[1], 16) < address + size:
                raise ValueError("completed guest outside fixed_runtime_idle")
        report["pass"] = True
    except ValueError as error:
        report["error"] = str(error)
    (out / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    sys.exit(capture())
