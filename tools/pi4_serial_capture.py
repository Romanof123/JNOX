"""Collect two Pi 4 scheduler UART runs for independent witness review.

--boot-dir names the mounted card boot directory or a staged copy. The operator
must identify which was hashed, witness two full power removals on the same
card and board, correlate power-on with each arm prompt, and sign the records.
USB read timestamps are host observations. Replays cannot establish board
operation, power timing or electrical silence.
"""

import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import uuid

if __package__:
    from .pi4_transcript import Matcher, PROFILES
else:
    from pi4_transcript import Matcher, PROFILES


EXPECTED = PROFILES["pi4-scheduler"]["scheduler"]
SECOND = 1_000_000_000
FIRMWARE = Path(__file__).resolve().parents[1] / "boards/pi4/firmware.json"


def digest(path):
    with path.open("rb") as source:
        h = hashlib.sha256()
        for chunk in iter(lambda: source.read(65536), b""):
            h.update(chunk)
        return h.hexdigest()


def write_json(path, record):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(record, stream, indent=2)
        stream.write("\n")


def artifacts(boot, elf):
    pinned = json.loads(FIRMWARE.read_text(encoding="utf-8"))
    names = sorted(set(pinned["files"]) | {"kernel8.img", "config.txt"})
    hashes = {name: digest(boot / name) for name in names}
    for name, expected in pinned["files"].items():
        if hashes[name] != expected:
            raise ValueError(f"firmware mismatch: {name}")
    manifest = boot / "SHA256SUMS"
    expected_lines = [f"{hashes[name]}  {name}" for name in names]
    if sorted(manifest.read_text(encoding="ascii").splitlines()) != sorted(expected_lines):
        raise ValueError("boot files disagree with the packaged SHA256SUMS")
    return {"boot_dir": str(boot.resolve()), "files_sha256": hashes,
            "firmware_commit": pinned["commit"], "package_manifest_sha256": digest(manifest),
            "elf": str(elf.resolve()), "elf_sha256": digest(elf)}


def observe(folder, read, *, profile="pi4-scheduler", now=time.monotonic_ns, sleep=time.sleep, arm=None):
    """Read bounded chunks. Keep every observed byte including a failed readout."""
    matcher = Matcher(profile)
    count = 0
    complete = None
    h = hashlib.sha256()
    with (folder / "uart.bin").open("xb") as raw, (folder / "reads.jsonl").open(
            "x", encoding="utf-8") as log:
        epoch, start = arm if arm is not None else (time.time_ns(), now())
        while True:
            chunk = read(4096)
            elapsed = now() - start
            if chunk:
                raw.write(chunk)
                h.update(chunk)
                log.write(json.dumps({"elapsed_ns": elapsed, "offset": count,
                                      "length": len(chunk)}) + "\n")
                end = count + len(chunk)
                if not matcher.feed(chunk):
                    count = end
                    verdict = "unexpected_bytes"
                    break
                count = end
                if matcher.variant is not None:
                    complete = elapsed
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
            "bytes": count, "uart_sha256": h.hexdigest(), "profile": profile,
            "matched_variant": matcher.variant if verdict == "matched" else None,
            "first_unexpected_offset": matcher.failure_offset,
            "quiet_elapsed_ns": None if complete is None else elapsed - complete}


def serial_port(port):
    try:
        import serial
    except ImportError as error:
        raise ValueError("physical serial capture requires pyserial") from error
    adapter = serial.Serial(port=None, baudrate=115200, bytesize=8, parity="N", stopbits=1,
                            timeout=0.02, xonxoff=False, rtscts=False, dsrdtr=False)
    adapter.dtr = False
    adapter.rts = False
    adapter.port = port
    adapter.open()
    return adapter


def build_association(path, metadata, profile):
    """Compare a submitted build record to the files actually hashed on this host."""
    result = {"status": "no_submitted_build_record", "capture_boot_identity": "unverified"}
    if path is None:
        return result
    raw = path.read_bytes()
    submission = json.loads(raw)
    if not isinstance(submission, dict):
        raise ValueError("build record must be a JSON object")
    keys = ("profile", "revision", "build_options", "elf_sha256", "files_sha256",
            "package_manifest_sha256", "firmware_commit")
    declared = {key: submission.get(key) for key in keys}
    mismatches = [key for key in keys[3:] if declared[key] != metadata[key]]
    if declared["profile"] != profile:
        mismatches.append("profile")
    for key in ("revision", "build_options"):
        if not declared[key]:
            mismatches.append(key)
    result.update(status="submitted_values_differ_or_missing" if mismatches
                  else "submitted_hashes_match_candidate_files", fields_to_verify=mismatches,
                  path=str(path.resolve()), sha256=hashlib.sha256(raw).hexdigest(), declared=declared)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--port", help="operator-declared physical serial adapter")
    source.add_argument("--replay", nargs=2, type=Path, metavar=("RUN1", "RUN2"))
    parser.add_argument("--boot-dir", required=True, type=Path)
    parser.add_argument("--elf", required=True, type=Path)
    parser.add_argument("--board-revision", help="required for physical capture")
    parser.add_argument("--eeprom", help="operator-reported bootloader version")
    parser.add_argument("--output", required=True, type=Path, help="new evidence directory")
    parser.add_argument("--profile", choices=PROFILES, default="pi4-scheduler")
    parser.add_argument("--build-record", type=Path, help="submitted JSON build and package association")
    parser.add_argument("--witness", help="declared witness identity")
    parser.add_argument("--card-id", help="operator or archive card identifier")
    parser.add_argument("--adapter-id", help="operator or archive UART adapter identifier")
    parser.add_argument("--boot-dir-source", choices=("mounted-card", "staged-copy", "archive"),
                        help="origin of the hashed boot directory")
    args = parser.parse_args()
    try:
        if not args.replay and (not args.board_revision or not args.eeprom):
            parser.error("--board-revision and --eeprom are required for physical capture")
        metadata = artifacts(args.boot_dir, args.elf)
        mode = "file_replay" if args.replay else "operator_declared_physical"
        if args.replay and any(not path.is_file() for path in args.replay):
            raise ValueError("each replay source must be a regular file")
        association = build_association(args.build_record, metadata, args.profile)
        session_id = str(uuid.uuid4())
        args.output.mkdir()
        write_json(args.output / "session.json", dict(metadata, mode=mode,
                   session_id=session_id, board_revision=args.board_revision, profile=args.profile,
                   witness=args.witness, card_id=args.card_id, adapter_id=args.adapter_id,
                   boot_dir_source=args.boot_dir_source, build_association=association,
                   eeprom=args.eeprom, witness_status="unsigned"))
        passed = True
        for index in range(2):
            folder = args.output / f"run-{index + 1}"
            folder.mkdir()
            run_id = str(uuid.uuid4())
            write_json(folder / "identity.json", dict(run_id=run_id, session_id=session_id,
                       run=index + 1, profile=args.profile, mode=mode))
            off_ack_unix_ns = None
            if args.replay:
                path = args.replay[index]
                stream = path.open("rb")
                label = str(path.resolve())
            else:
                input(f"Run {index + 1}: fully remove board power. Press Enter while off. ")
                off_ack_unix_ns = time.time_ns()
                stream = serial_port(args.port)
                label = args.port
            with stream:
                arm = None
                if not args.replay:
                    stream.reset_input_buffer()
                    input("Keep power off and press Enter when ready to arm. ")
                    arm = (time.time_ns(), time.monotonic_ns())
                    print("Armed. Apply board power now.", flush=True)
                result = observe(folder, stream.read, profile=args.profile, arm=arm)
            write_json(folder / "result.json", dict(result, run_id=run_id, session_id=session_id,
                       operator_off_ack_unix_ns=off_ack_unix_ns,
                       run=index + 1, mode=mode, source=label))
            passed = passed and result["uart_verdict"] == "matched"
        return 0 if passed else 1
    except (ValueError, OSError, EOFError) as error:
        print(f"Capture failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
