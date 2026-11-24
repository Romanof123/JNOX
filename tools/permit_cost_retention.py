"""Host-only CHG-061 terminal attestation and durable capture primitives."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import tempfile
import time


def termination_output(stdout, stderr, stop, returncode):
    if stdout or not stop or stop["reason"] != "guest-terminal" or not stop["sent"]:
        raise ValueError("unexpected output or unconfirmed terminal stop")
    if returncode != 0 or stop.get("escalated") or stop["signal"] != signal.SIGTERM:
        raise ValueError("abnormal emulator termination")
    if stop["target_pid"] != stop["child_pid"] or stop["sender_pid"] != os.getpid():
        raise ValueError("termination actor mismatch")
    if stop["target_exe"] != stop["expected_exe"] or not stop["sender_name"]:
        raise ValueError("termination executable mismatch")
    expected = (f"{Path(stop['target_exe']).name}: terminating on signal 15 from pid "
                f"{stop['sender_pid']} ({stop['sender_name']})\n").encode("ascii")
    if stderr and stderr != expected:
        raise ValueError("unexpected, mixed or incomplete emulator stderr")
    return "matched-sigterm-ack" if stderr else "silent-terminal-stop"


def request_stop(proc, reason, expected_exe):
    if proc.poll() is not None:
        raise ValueError("emulator exited without collector termination")
    target = os.readlink(f"/proc/{proc.pid}/exe")
    if target != expected_exe:
        raise ValueError("unexpected running executable")
    record = dict(reason=reason, target_pid=proc.pid, child_pid=proc.pid,
                  expected_exe=expected_exe, target_exe=target, sender_pid=os.getpid(),
                  sender_name=Path("/proc/self/comm").read_text().rstrip("\n"),
                  sender_exe=os.readlink("/proc/self/exe"), signal=int(signal.SIGTERM),
                  requested_ns=time.monotonic_ns(), sent=False, escalated=False)
    os.kill(proc.pid, signal.SIGTERM)
    record.update(sent=True, sent_ns=time.monotonic_ns())
    return record


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def fingerprint(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError("missing or nonregular artifact: " + str(path))
    with path.open("rb") as f:
        h = hashlib.sha256()
        for block in iter(lambda: f.read(65536), b""):
            h.update(block)
        os.fsync(f.fileno())
        return {"bytes": f.tell(), "sha256": h.hexdigest()}


def manifest(source, names):
    return {name: fingerprint(source / name) for name in sorted(names)}


def publish(source, destination, names, required):
    """Never overwrite a destination or promote a partial copy."""
    if destination.exists() or not set(required).issubset(names):
        raise ValueError("duplicate destination or missing required artifact")
    snapshot = manifest(source, names)
    sync_dir(source)
    temporary = Path(tempfile.mkdtemp(prefix=".partial-", dir=destination.parent))
    for name in snapshot:
        rel = Path(name)
        if rel.is_absolute() or ".." in rel.parts:
            raise ValueError("invalid manifest path")
        target = temporary / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / rel, target)
    if manifest(temporary, names) != snapshot or manifest(source, names) != snapshot:
        raise ValueError("source or copy differs from frozen manifest")
    receipt = temporary / "retained-manifest.json"
    receipt.write_text(json.dumps(snapshot, sort_keys=True, indent=2)+"\n", encoding="utf-8")
    fingerprint(receipt)
    for folder, _, _ in os.walk(temporary, topdown=False):
        sync_dir(Path(folder))
    os.rename(temporary, destination)
    sync_dir(destination.parent)
    if json.loads((destination / receipt.name).read_bytes()) != manifest(destination, names):
        raise ValueError("published manifest mismatch")
    return snapshot
