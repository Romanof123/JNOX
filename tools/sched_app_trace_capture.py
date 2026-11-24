#!/usr/bin/env python3
"""Local halted-VM wrapper for qemu_boot.py --qemu (one fresh directory per boot)."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import uuid

import qemu_capture

SYMBOL = "jnox_sched_app_trace_v1"
WITNESS = "mlfq_wake_witness"
ENV = "SCHED_APP_TRACE_CAPTURE_DIR"


def capture():
    out = Path(os.environ[ENV]).resolve()
    out.mkdir(parents=True, exist_ok=False)
    root = Path(__file__).resolve().parents[1]
    flags = {key: os.environ.get(key) for key in (
        "JNOX_BOARD", "JNOX_PROFILE", "JNOX_APP", "JNOX_MULTILEVEL_TRACE",
        "JNOX_SCHED_APP_TRACE")}
    metadata = {"run_uuid": str(uuid.uuid4()), "flags": flags, "argv": sys.argv[1:],
                "capture_directory": str(out), "tools": {"python": sys.version}}
    try:
        mode = flags["JNOX_SCHED_APP_TRACE"]
        if mode not in (None, "disabled", "active", "saturate"):
            raise ValueError("unknown mode")
        if [flags[k] for k in ("JNOX_BOARD", "JNOX_PROFILE", "JNOX_APP")] != [
                "virt", "multilevel", "mlfq-wake"]:
            raise ValueError("expected virt/multilevel/mlfq-wake selectors")
        if flags["JNOX_MULTILEVEL_TRACE"] not in (None, "1"):
            raise ValueError("unknown multilevel trace selector")
        metadata["source_commit"] = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
        metadata["source_status"] = subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=root, text=True)
        for tool in ("aarch64-linux-gnu-nm", "qemu-system-aarch64"):
            metadata["tools"][tool] = subprocess.check_output(
                [tool, "--version"], text=True).splitlines()[0]
        index = sys.argv.index("-kernel") + 1
        elf = out / "kernel.elf"
        shutil.copyfile(sys.argv[index], elf)
        sys.argv[index] = str(elf)
        if (qemu_capture.journal_location(elf, SYMBOL, optional=True) is None) != (mode is None):
            raise ValueError("mode conflicts with recorder symbol")
        extra = ["multilevel_terminal_ctl"]
        if flags["JNOX_MULTILEVEL_TRACE"]:
            extra.append("ordinary_multilevel_witness")
        metadata["result"] = qemu_capture.main(
            WITNESS, ENV, trace_symbol=SYMBOL if mode else None,
            require_trace=bool(mode), extra_symbols=extra, freeze=True, prepared_dir=out,
            audit_name=WITNESS + ".bin")
        return metadata["result"]
    except Exception as error:
        metadata["error"] = str(error)
        raise
    finally:
        (out / "capture.json").write_text(json.dumps(metadata, indent=2) + "\n")


if __name__ == "__main__":
    sys.exit(capture())
