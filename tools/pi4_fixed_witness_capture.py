"""Collect two separately witnessed Pi 4 engineering boots or ingest two labeled replays."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import uuid

from pi4_fixed_witness import PROFILES
from pi4_serial_capture import SECOND, artifacts, serial_port, write_json

def observe(folder, read, *, now=time.monotonic_ns, sleep=time.sleep):
    """Read bounded chunks. Keep every observed byte including a failed readout."""
    count = 0
    complete = None
    engineering_bytes = bytearray()
    h = hashlib.sha256()
    with (folder / "uart.bin").open("xb") as raw, (folder / "reads.jsonl").open(
            "x", encoding="utf-8") as log:
        epoch = time.time_ns()
        start = now()
        while True:
            chunk = read(4096)
            elapsed = now() - start
            if chunk:
                raw.write(chunk)
                h.update(chunk)
                log.write(json.dumps({"elapsed_ns": elapsed, "offset": count,
                                      "length": len(chunk)}) + "\n")
                end = count + len(chunk)
                engineering_bytes.extend(chunk)
                if end > 256 * 1024 or complete is not None:
                    count = end
                    verdict = "unexpected_bytes"
                    break
                footer = engineering_bytes.rfind(b"\nPW END ")
                if footer >= 0 and b"\n" in engineering_bytes[footer+1:]:
                    complete = elapsed
                count = end
            if (complete is None and elapsed >= 30 * SECOND) or (
                    complete is not None and complete > 30 * SECOND):
                verdict = "timeout"
                break
            if complete is not None and elapsed - complete >= SECOND:
                verdict = "matched"
                break
            if not chunk:
                sleep(0.005)
    return {"uart_verdict": verdict, "arm_unix_ns": epoch, "arm_monotonic_ns": start,
            "elapsed_ns": elapsed, "final_lf_elapsed_ns": complete,
            "bytes": count, "uart_sha256": h.hexdigest()}



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=PROFILES, required=True)
    parser.add_argument("--image-id", type=int, help="engineering package ID embedded at build")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--port", help="operator-declared physical serial adapter")
    source.add_argument("--replay", nargs=2, type=Path, metavar=("RUN1", "RUN2"))
    parser.add_argument("--boot-dir", required=True, type=Path)
    parser.add_argument("--elf", required=True, type=Path)
    parser.add_argument("--board-revision", required=True)
    parser.add_argument("--eeprom", required=True, help="operator-reported bootloader version")
    parser.add_argument("--output", required=True, type=Path, help="new evidence directory")
    args = parser.parse_args()
    try:
        if args.image_id is None or not 0 < args.image_id < 2**64:
            raise ValueError("engineering profile requires a nonzero u64 image ID")
        metadata = artifacts(args.boot_dir, args.elf)
        metadata.update(profile=args.profile,image_id=args.image_id,
                            image_sha256=metadata["files_sha256"]["kernel8.img"])
        mode = "file_replay" if args.replay else "operator_declared_physical"
        if args.replay and any(not path.is_file() for path in args.replay):
            raise ValueError("each replay source must be a regular file")
        args.output.mkdir()
        write_json(args.output / "session.json", dict(metadata, mode=mode,
                   session_id=str(uuid.uuid4()), board_revision=args.board_revision,
                   eeprom=args.eeprom, witness_status="unsigned"))
        passed = True
        for index in range(2):
            folder = args.output / f"run-{index + 1}"
            folder.mkdir()
            if args.replay:
                path = args.replay[index]
                stream = path.open("rb")
                label = str(path.resolve())
            else:
                input(f"Run {index + 1}: fully remove board power. Press Enter while off. ")
                stream = serial_port(args.port)
                label = args.port
            with stream:
                if not args.replay:
                    stream.reset_input_buffer()
                    input("Press Enter to arm capture, then immediately apply board power. ")
                result = observe(folder, stream.read)
            if result["uart_verdict"] == "matched":
                from pi4_fixed_witness import score
                import struct
                import subprocess
                try:
                    evidence = score((folder/"uart.bin").read_bytes(),args.profile,args.image_id,args.elf)
                    result["witness"] = evidence
                    if not evidence["complete"]:
                        result["uart_verdict"] = "preemption_or_peer_resume_unobserved"
                except (ValueError, IndexError, OSError, struct.error, subprocess.CalledProcessError) as error:
                    result.update(uart_verdict="invalid_witness", detail=str(error))
            write_json(folder / "result.json", dict(result, run_id=str(uuid.uuid4()),
                       run=index + 1, mode=mode, source=label))
            passed = passed and result["uart_verdict"] == "matched"
        return 0 if passed else 1
    except (ValueError, OSError, EOFError) as error:
        print(f"Capture failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
