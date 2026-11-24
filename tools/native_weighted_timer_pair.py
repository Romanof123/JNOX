#!/usr/bin/env python3
"""Run #190's two frozen positions once; retain each on a preprovisioned Windows D: destination."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from zipfile import ZipFile, ZIP_DEFLATED

from ordinary_amc_capture import sha

TOOLS = Path(__file__).resolve().parent
REV = "af952e34cea8fbe0aef5c7b4cb1f945a2884fff5"
ASSETS = {
    "kernel.elf": "7639609e478d59ffbd769063b19d00c6e08902a6417d943a279ef72ff0c5f4b0",
    "user.elf": "9d3c6350a04536da091dd7bc77cfdcc81729f8baa431d6315e5258c28da67206",
    "probe.c": "69a4fd0d9e566f405cc78cfa03a9a3e0ef81fd795cdd72d18fe452fbf911aa61",
}
QEMU = Path("/usr/bin/qemu-system-aarch64")
PIN = "0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8"
RAW = {
    "launch.json",
    "session.json",
    "ram.bin",
    "exceptions.log",
    "uart.bin",
    "stderr",
    *ASSETS,
}
POINTS = {
    "cval",
    "gic_ppi",
    "cpu_irq_line",
    "recalc_active",
    "handler_disable",
    "handler_iar",
}
INTERVALS = (
    "cval_to_ppi_us",
    "ppi_to_cpu_line_us",
    "ppi_to_iar_us",
    "ppi_to_handler_disable_us",
)


def write_record(path, record):
    with path.open("x", encoding="utf8") as out:
        json.dump(record, out, indent=2)
        out.write("\n")


def run_command(stage, name, cmd):
    write_record(stage / (name + ".argv.json"), cmd)
    with (stage / (name + ".stdout")).open("xb") as out, (
        stage / (name + ".stderr")
    ).open("xb") as err:
        try:
            rc = subprocess.run(
                cmd, stdout=out, stderr=err, stdin=subprocess.DEVNULL
            ).returncode
            record = {"returncode": rc}
        except OSError as exc:
            record = {"returncode": None, "error": str(exc)}
    write_record(stage / (name + ".exit.json"), record)
    return record["returncode"]


def measurement_gaps(stage, instrumented):
    # Retention success is independent of program and measurement outcomes.
    try:
        report = json.loads((stage / "replay.stdout").read_bytes())
        if (
            report["arm"] != "share"
            or report["clock"] != "default"
            or len(report["prefix"]) != 36
        ):
            return ["missing original share/default 36-closure report"]
        gaps = list(report["timing_misses"])
        if instrumented:
            obs = report["timer_observation"]
            irqs = obs["irqs"]
            expected = [g["lease"] for g in report["closures"] if g["cause"] == "irq"]
            if not expected or [row["lease"] for row in irqs] != expected:
                gaps.append("missing IRQ timing rows")
            if obs.get("host_schedstat") is None:
                gaps.append("vCPU schedstat unobserved")
            for row in irqs:
                fields = [k for k in sorted(POINTS) if not row["host_points"].get(k)]
                fields += [k for k in INTERVALS if row.get(k) is None]
                if row.get("irq_log_line") is None:
                    fields.append("irq_log_line")
                if fields:
                    gaps.append(f"lease {row['lease']} missing: " + ", ".join(fields))
        return gaps
    except (OSError, ValueError, TypeError, KeyError) as exc:
        return ["missing or incomplete official report: " + str(exc)]


def capture_stage(stage, inputs, instrumented):
    session = stage / "session"
    cmd = [
        sys.executable,
        str(TOOLS / "native_weighted_capture.py"),
        "share",
        "default",
        str(session),
        *(str(inputs / name) for name in ASSETS),
        REV,
        "--qemu",
        str(QEMU),
    ]
    if instrumented:
        cmd.append("--timer-observation")
    rc = run_command(stage, "capture", cmd)
    lines = (stage / "capture.stdout").read_bytes().splitlines()
    digests = [
        line.decode("ascii") for line in lines if re.fullmatch(rb"[0-9a-f]{64}", line)
    ]
    expected = RAW | ({"host-timing.json"} if instrumented else set())
    missing = sorted(name for name in expected if not (session / name).is_file())
    errors = []
    replay_rc = None
    if len(digests) == 1:
        digest = digests[0]
        (stage / "session.external.sha256").write_text(digest + "\n", encoding="ascii")
        replay_rc = run_command(
            stage,
            "replay",
            [
                sys.executable,
                str(TOOLS / "native_weighted_replay.py"),
                str(session),
                digest,
            ],
        )
    else:
        reason = "capture stdout did not provide a unique session SHA-256"
        errors.append(reason)
        write_record(
            stage / "replay.exit.json", {"returncode": None, "skipped": reason}
        )
    errors += measurement_gaps(stage, instrumented)
    return dict(
        position=stage.name,
        capture_rc=rc,
        replay_rc=replay_rc,
        missing=missing,
        gaps=errors,
        qualified=(rc == 0 and replay_rc == 0 and not missing and not errors),
    )


def windows_path(path):
    return subprocess.check_output(["wslpath", "-w", str(path)], text=True).strip()


def retain(stage, dest, powershell):
    files = sorted(p for p in stage.rglob("*") if p.is_file())
    members = [
        dict(name=p.relative_to(stage).as_posix(), size=p.stat().st_size, sha256=sha(p))
        for p in files
    ]
    archive = dest / "position.zip"
    with ZipFile(archive, "x", compression=ZIP_DEFLATED, compresslevel=1) as out:
        for path, item in zip(files, members):
            out.write(path, item["name"])
    manifest = dest / "manifest.json"
    external = stage / "session.external.sha256"
    session_digest = (
        external.read_text(encoding="ascii").strip() if external.is_file() else None
    )
    if session_digest is not None:
        with (dest / "capture-session.sha256").open("x", encoding="ascii") as out:
            out.write(session_digest + "\n")
    archive_hash = sha(archive)
    write_record(
        manifest,
        dict(
            archive_sha256=archive_hash, session_sha256=session_digest, members=members
        ),
    )
    (dest / "position.zip.sha256").write_text(
        archive_hash + "  position.zip\n", encoding="ascii"
    )
    manifest_hash = sha(manifest)
    with ZipFile(archive) as reopened:
        if reopened.namelist() != [i["name"] for i in members]:
            raise ValueError("reopened archive member set/order")
        for item in members:
            with reopened.open(item["name"]) as src:
                h = hashlib.sha256()
                for block in iter(lambda: src.read(1 << 20), b""):
                    h.update(block)
            if (
                h.hexdigest() != item["sha256"]
                or reopened.getinfo(item["name"]).file_size != item["size"]
            ):
                raise ValueError("reopened archive bytes: " + item["name"])
    write_record(
        dest / "linux-reopened.json",
        dict(path=str(archive), members=len(members), archive_sha256=archive_hash),
    )
    cmd = [
        powershell,
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        windows_path(TOOLS / "native_weighted_timer_verify.ps1"),
        "-Folder",
        windows_path(dest),
        "-ManifestHash",
        manifest_hash,
    ]
    code = run_command(dest, "windows-reopen", cmd)
    if code != 0:
        raise ValueError(
            "native Windows verification failed; retain local stage and stop"
        )
    receipt = json.loads(
        (dest / "windows-reopen.stdout").read_bytes().decode("utf-8-sig")
    )
    if (
        receipt["archive_sha256"] != archive_hash
        or receipt["manifest_sha256"] != manifest_hash
        or receipt["members"] != len(members)
    ):
        raise ValueError("native Windows receipt mismatch")
    print(f"Verified {receipt['path']}  SHA256 {archive_hash}", flush=True)


def dependency_snapshot(inputs):
    import elftools
    import native_weighted_capture
    import native_weighted_replay
    import native_weighted_timer
    import native_weighted_trace
    from elftools.elf.elffile import ELFFile
    from native_el0_replay import elf_file

    image = (inputs / "user.elf").read_bytes()
    parsed = elf_file(image)
    if (
        not isinstance(parsed, ELFFile)
        or hashlib.sha256(image).hexdigest() != ASSETS["user.elf"]
    ):
        raise ValueError("preflight could not open the pinned user ELF")
    return dict(
        python=sys.executable,
        python_version=sys.version,
        pythonpath=os.environ.get("PYTHONPATH"),
        pyelftools=elftools.__version__,
        package=elftools.__file__,
        entry=parsed["e_entry"],
        modules=[
            m.__file__
            for m in (
                native_weighted_capture,
                native_weighted_replay,
                native_weighted_timer,
                native_weighted_trace,
            )
        ],
    )


def preflight(dest, stage, inputs, powershell):
    durable = dest / "preflight"
    durable.mkdir()
    # Use exactly the interpreter and inherited module environment used by capture_stage.
    parent = dependency_snapshot(inputs)
    write_record(durable / "wrapper-python.json", parent)
    probe = (
        "import json,sys; from pathlib import Path; sys.path.insert(0,sys.argv[1]); "
        "from native_weighted_timer_pair import dependency_snapshot; "
        "print(json.dumps(dependency_snapshot(Path(sys.argv[2]))))"
    )
    code = run_command(
        durable, "child-python", [sys.executable, "-c", probe, str(TOOLS), str(inputs)]
    )
    if (
        code != 0
        or json.loads((durable / "child-python.stdout").read_bytes()) != parent
    ):
        raise ValueError(
            "capture/replay interpreter dependencies differ or failed to load"
        )
    from native_weighted_timer import EVENTS

    nm = shutil.which("aarch64-linux-gnu-nm")
    if (
        not nm
        or run_command(durable, "nm", [nm, "-nS", str(inputs / "kernel.elf")]) != 0
    ):
        raise ValueError("AArch64 symbol reader unavailable")
    if run_command(durable, "trace-capabilities", [str(QEMU), "-trace", "help"]) != 0:
        raise ValueError("QEMU trace capability listing unavailable")
    if not set(EVENTS).issubset(
        (durable / "trace-capabilities.stdout").read_text().splitlines()
    ):
        raise ValueError("required QEMU timer/GIC tracepoints unavailable")
    fixture = stage / "preflight"
    fixture.mkdir()
    data = b"JNOX #190 third cohort preflight\n"
    (fixture / "fixture.bin").write_bytes(data)
    # Exercise both independent digest sidecars with fixture data, before any guest exists.
    digest = hashlib.sha256(data).hexdigest()
    (fixture / "session.external.sha256").write_text(digest + "\n", encoding="ascii")
    retain(fixture, durable, powershell)
    write_record(
        durable / "complete.json",
        dict(kind="fixture-only", guest_count=0, fixture_sha256=digest),
    )


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("inputs", type=Path)
    p.add_argument("destination", type=Path)
    p.add_argument("--scratch", type=Path, default=Path("/tmp"))
    a = p.parse_args()
    dest = a.destination.resolve(strict=True)
    scratch = a.scratch.resolve(strict=True)
    inputs = a.inputs.resolve(strict=True)
    if (
        not dest.is_dir()
        or any(dest.iterdir())
        or not windows_path(dest).upper().startswith("D:\\")
    ):
        p.error(
            "provide an existing empty independently provisioned Windows D: directory"
        )
    write_record(
        dest / "cohort.json",
        dict(
            ticket=190,
            cohort=3,
            feature_parent="b418cbbf961ebf5ccf13150d80da142bf8dd033d",
            guest_source=REV,
            positions=["plain", "instrumented"],
            qemu_sha256=PIN,
            inputs=ASSETS,
        ),
    )
    phase = "preflight"
    try:
        fs = subprocess.check_output(
            ["findmnt", "-n", "-o", "FSTYPE", "-T", str(scratch)], text=True
        ).strip()
        if fs not in {"ext4", "btrfs", "xfs", "tmpfs"}:
            raise ValueError("scratch must be on a Linux-local filesystem")
        powershell = shutil.which("powershell.exe")
        if (
            not powershell
            or sha(QEMU) != PIN
            or any(sha(inputs / n) != h for n, h in ASSETS.items())
        ):
            raise ValueError(
                "Windows PowerShell, pinned QEMU and exact #183 assets are required"
            )
        stage = Path(tempfile.mkdtemp(prefix="jnox-190-third-", dir=scratch))
        write_record(
            dest / "stage.json",
            dict(stage=str(stage), python=sys.executable, powershell=powershell),
        )
        print("Retaining Linux stage", stage, flush=True)
        preflight(dest, stage, inputs, powershell)
        qualified = True
        for name in ("plain", "instrumented"):
            phase = name
            local = stage / name
            place = dest / name
            place.mkdir()
            local.mkdir()
            problem = None
            try:
                verdict = capture_stage(local, inputs, name == "instrumented")
            except Exception as exc:
                problem = str(exc)
                verdict = dict(position=name, qualified=False, error=problem)
            write_record(local / "position.json", verdict)
            retain(local, place, powershell)
            if problem is not None:
                raise ValueError(problem)
            qualified &= verdict["qualified"]
    except Exception as exc:
        write_record(
            dest / "infrastructure-failure.json",
            dict(phase=phase, error=str(exc), exception=type(exc).__name__),
        )
        print(f"Infrastructure failure in {phase}: {exc}", file=sys.stderr)
        return 2
    return 0 if qualified else 1


if __name__ == "__main__":
    sys.exit(main())
