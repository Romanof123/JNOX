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


RECEIPT = "retained-manifest.json"


def exact_files(root, names):
    """The caller supplies the intended paths. A directory scan can only disprove them."""
    paths = set(names)
    directories = set()
    for name in names:
        path = Path(name)
        if (not name or path == Path(".") or path.is_absolute() or ".." in path.parts
                or path.as_posix() != name or chr(92) in name or ":" in name):
            raise ValueError("invalid artifact name: " + repr(name))
        directories.update(parent.as_posix() for parent in path.parents if parent != Path("."))
    if len(paths) != len(names) or paths & directories or root.is_symlink() or not root.is_dir():
        raise ValueError("invalid bundle root or path collision")
    seen = set()
    def visit(folder):
        for item in folder.iterdir():
            name = item.relative_to(root).as_posix()
            if item.is_symlink():
                raise ValueError("unexpected symlink: " + name)
            if item.is_dir() and name in directories:
                visit(item)
            elif item.is_file() and name in paths:
                seen.add(name)
            else:
                raise ValueError("unexpected entry or type: " + name)
    visit(root)
    if seen != paths:
        raise ValueError("missing artifacts: " + repr(sorted(paths - seen)))
    return {name: fingerprint(root / name) for name in sorted(paths)}


def verify(source, replica, names, snapshot):
    if RECEIPT in names or set(snapshot) != set(names):
        raise ValueError("invalid expected manifest keys")
    original = exact_files(source, names)
    files = exact_files(replica, [*names, RECEIPT])
    del files[RECEIPT]
    expected = (json.dumps(snapshot, sort_keys=True, indent=2) + chr(10)).encode("utf-8")
    if original != snapshot or files != snapshot or (replica / RECEIPT).read_bytes() != expected:
        raise ValueError("published manifest, source or replica mismatch")


def publish(source, destination, names, required, expected=None):
    """Keep a rejected staging or final directory in place for diagnosis."""
    if os.path.lexists(destination) or RECEIPT in names or not set(required).issubset(names):
        raise ValueError("duplicate destination or missing required artifact")
    snapshot = exact_files(source, names)
    if expected is not None and snapshot != expected:
        raise ValueError("source differs from the capture record")
    for folder, _, _ in os.walk(source, topdown=False):
        sync_dir(Path(folder))
    temporary = Path(tempfile.mkdtemp(prefix="." + destination.name + "-partial-", dir=destination.parent))
    for name in snapshot:
        target = temporary / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / name, target)
    exact_files(temporary, names)
    with (temporary / RECEIPT).open("x", encoding="utf-8", newline=chr(10)) as receipt:
        receipt.write(json.dumps(snapshot, sort_keys=True, indent=2) + chr(10))
    verify(source, temporary, names, snapshot)
    for folder, _, _ in os.walk(temporary, topdown=False):
        sync_dir(Path(folder))
    os.rename(temporary, destination)
    sync_dir(destination.parent)
    verify(source, destination, names, snapshot)
    return snapshot
