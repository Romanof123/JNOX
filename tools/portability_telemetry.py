#!/usr/bin/env python3
"""Prepare telemetry inputs, capture the local executable, or compare independent recordings.

prepare DIR writes each input and its argv to DIR/CASE. Capture stdout.bin,
stderr.bin and status.txt (decimal exit status followed by LF) for each case.
host DIR EXECUTABLE creates that directory and records a local execution.
check DIR scores pinned bytes and status. compare HOST TARGET also checks pairwise equality.
For an independent target, copy the prepared fixtures and run the target binary in
each case directory with its argv; capture each stream separately without a TTY,
and record the process status. Return the original case folders plus provenance.txt
(describe the target, deployed image/build and collection procedure) and run.json:
{"version":1,"role":"donor","system":"<target OS/version>","executable":"<deployed path>",
 "executable_sha256":"<64 lowercase hex>","origin":"independent target capture"}
Use role "J-NOX" for a real J-NOX execution. Executable hash is the measured target
binary digest. Role and identity in an imported record are collector declarations.
These cases cover regular-file input; synthetic summary overflow and forced OS
read/close/write faults have no measured coverage in this suite.
"""
import argparse
import hashlib
import json
import platform
import re
import subprocess
import sys
from pathlib import Path


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def prepare(root):
    root.mkdir()
    for name, (data, args, *_) in cases().items():
        folder = root / name
        folder.mkdir()
        write_json(folder / "argv.json", args)
        if data is not None:
            (folder / "input.txt").write_bytes(data)


def host(root, executable):
    executable = executable.resolve(strict=True)
    digest = sha(executable)
    prepare(root)
    write_json(
        root / "run.json",
        {
            "version": 1,
            "role": "host",
            "system": platform.platform(),
            "executable_sha256": digest,
            "executable": str(executable),
            "origin": "local subprocess",
        },
    )
    for name, (_, args, *_) in cases().items():
        folder = root / name
        with (folder / "stdout.bin").open("xb") as out, (folder / "stderr.bin").open(
            "xb"
        ) as err:
            result = subprocess.run(
                [str(executable), *args],
                cwd=folder,
                stdin=subprocess.DEVNULL,
                stdout=out,
                stderr=err,
                timeout=15,
            )
        (folder / "status.txt").write_bytes(f"{result.returncode}\n".encode("ascii"))
    if sha(executable) != digest:
        raise ValueError("executable changed during capture")


def observation(root, name, data, args):
    folder = root / name
    if json.loads((folder / "argv.json").read_bytes()) != args:
        raise ValueError(f"{root}/{name}: argument vector mismatch")
    input_path = folder / "input.txt"
    if (data is None and input_path.exists()) or (
        data is not None and input_path.read_bytes() != data
    ):
        raise ValueError(f"{root}/{name}: input fixture mismatch")
    if (folder / "missing.txt").exists():
        raise ValueError(f"{root}/{name}: absent-path fixture exists")
    status = (folder / "status.txt").read_bytes()
    if not re.fullmatch(rb"(0|[1-9][0-9]{0,2})\n", status) or int(status) > 255:
        raise ValueError(f"{root}/{name}: invalid exit status")
    return (
        int(status),
        (folder / "stdout.bin").read_bytes(),
        (folder / "stderr.bin").read_bytes(),
    )


def cases():
    file_arg = ["input.txt"]
    sample = b"0\n0007\n65535\n"
    invalid = (4, b"", b"invalid sample\n")
    entries = {
        "zero-max": (
            sample,
            file_arg,
            0,
            b"samples=3 min=0 max=65535 sum=65542\n",
            b"",
        ),
        # 127 complete records occupy offsets 0..253; the five-digit token crosses offset 256.
        "split-token": (
            b"1\n" * 127 + b"65535\n",
            file_arg,
            0,
            b"samples=128 min=1 max=65535 sum=65662\n",
            b"",
        ),
        "repeated": (
            b"0042\n42\n42\n",
            file_arg,
            0,
            b"samples=3 min=42 max=42 sum=126\n",
            b"",
        ),
        "empty": (b"", file_arg, 3, b"", b"empty input\n"),
        "overflow-value": (b"65536\n", file_arg, *invalid),
        "missing-lf": (b"7\n42", file_arg, *invalid),
        "absent-path": (None, ["missing.txt"], 5, b"", b"input error\n"),
        "missing-operand": (None, [], 2, b"", b"usage: telemetry_monitor PATH\n"),
        "extra-operand": (
            None,
            ["missing.txt", "second"],
            2,
            b"",
            b"usage: telemetry_monitor PATH\n",
        ),
    }
    for name, token in (
        ("plus", b"+7"),
        ("minus", b"-7"),
        ("space", b" 7"),
        ("crlf", b"7\r"),
        ("blank", b""),
        ("nul", b"7\x00"),
        ("nonascii", b"\xff"),
        ("text", b"7x"),
    ):
        entries[name] = (b"3\n" + token + b"\n", file_arg, *invalid)
    return entries


def metadata(root):
    record = json.loads((root / "run.json").read_bytes())
    role = record["role"]
    if record["version"] != 1 or role not in ("host", "donor", "J-NOX"):
        raise ValueError("unsupported capture version or role")
    for key in ("system", "executable", "origin"):
        if not isinstance(record[key], str) or not record[key].strip():
            raise ValueError("missing capture metadata: " + key)
    if not re.fullmatch("[0-9a-f]{64}", record["executable_sha256"]):
        raise ValueError("missing SHA256 of executed binary")
    if role == "host" and record["origin"] != "local subprocess":
        raise ValueError("unsupported host recorder")
    if role != "host":
        if record["origin"] != "independent target capture":
            raise ValueError("target requires independent collection")
        provenance = (root / "provenance.txt").read_bytes()
        if not provenance.strip():
            raise ValueError("missing independent target collection record")
    return record


def score(root):
    info = metadata(root)
    observed = {}
    errors = []
    for name, (data, args, status, out, err) in cases().items():
        actual = observation(root, name, data, args)
        observed[name] = actual
        for field, value, expected in zip(
            ("status", "stdout", "stderr"), actual, (status, out, err)
        ):
            if value != expected:
                errors.append(
                    f"{info['role']} {name} {field}: expected {expected!r}, got {value!r}"
                )
    print(
        f"{info['role']} {info['system']}: {len(observed)} recorded cases; executable SHA256 {info['executable_sha256']}"
    )
    return info, observed, errors


def run(args):
    if args.command == "prepare":
        prepare(args.root)
        return 0
    if args.command == "host":
        host(args.root, args.executable)
    info, observed, errors = score(args.root)
    if args.command == "compare":
        other, target, target_errors = score(args.target)
        if (
            info["role"] != "host"
            or other["role"] not in ("donor", "J-NOX")
            or args.root.samefile(args.target)
        ):
            raise ValueError(
                "compare requires separate host and independently recorded target directories"
            )
        errors.extend(target_errors)
        for name in observed:
            for field, local, remote in zip(
                ("status", "stdout", "stderr"), observed[name], target[name]
            ):
                if local != remote:
                    errors.append(f"{name} {field}: host and target differ")
        print(
            "Comparison covers the recorded argument, file, byte-stream and exit-status cases. "
            "Target system and binary identity come from the external collection record."
        )
    elif info["role"] == "host":
        print("Host reference only. Donor and J-NOX executions are unobserved.")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("All recorded cases match the pinned contract.")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("prepare").add_argument("root", type=Path)
    local = sub.add_parser("host")
    local.add_argument("root", type=Path)
    local.add_argument("executable", type=Path)
    sub.add_parser("check").add_argument("root", type=Path)
    cmp = sub.add_parser("compare")
    cmp.add_argument("root", type=Path)
    cmp.add_argument("target", type=Path)
    try:
        sys.exit(run(parser.parse_args()))
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as exc:
        parser.exit(2, f"Incomplete or invalid recording: {exc}\n")
