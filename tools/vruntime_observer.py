"""Check weighted service from the complete LF-normalized one-CPU guest UART."""

import re
from typing import NamedTuple


MAX_BYTES = 8192
_U64_MAX = (1 << 64) - 1
_NUMBER = re.compile(rb"-?(?:0|[1-9][0-9]{0,19})")
_RAM_START, _RAM_END = 0x40200000, 0x48000000
_STACK_BYTES = 16 * 1024


def _fields(line: bytes, prefix: bytes, names: bytes) -> tuple:
    if not line.startswith(prefix + b" "):
        raise ValueError
    words = line[len(prefix) + 1:].split(b" ")
    keys = names.split()
    if len(words) != len(keys):
        raise ValueError
    values = []
    for key, word in zip(keys, words):
        label, _, digits = word.partition(b"=")
        if label != key or not _NUMBER.fullmatch(digits) or digits == b"-0":
            raise ValueError
        value = int(digits)
        if abs(value) > _U64_MAX or (value < 0 and not key.startswith(b"lag_")):
            raise ValueError
        values.append(value)
    return tuple(values)


class _Slot(NamedTuple):
    t: int
    task: int
    ready: int
    start: int
    cval: int
    irq: int
    charge: int
    a2: int
    b2: int
    floor2: int
    elr: int
    sp: int
    progress: int


def _frames_match(rows: list[_Slot]) -> bool:
    first = rows[0]  # Lower-ID A owns the initial tie.
    allocation = first.sp + 16 - _STACK_BYTES
    if first.sp % 16 or not _RAM_START <= allocation <= _RAM_END - 2 * _STACK_BYTES:
        return False
    for row in rows:
        if row.sp != first.sp + row.task * _STACK_BYTES:
            return False
        if row.elr % 4 or not _RAM_START <= row.elr < allocation:
            return False
    # Each work loop has two instructions. B's loop follows A's by 32 bytes
    # in vruntime.S. Absolute addresses are bound separately to the frozen ELF.
    return any(
        all(row.elr - base - 32 * row.task in (0, 4) for row in rows)
        for base in (first.elr, first.elr - 4)
    )


def validate(transcript: bytes) -> bool:
    """Validate one complete witness, replaying each decision from counter deltas."""
    if not isinstance(transcript, bytes) or len(transcript) > MAX_BYTES:
        return False
    lines = transcript.split(b"\n")
    if len(lines) != 32 or lines[:3] != [b"JNOX M1 BOOT", b"JNOX M1 READY", b"VR BEGIN"]:
        return False
    if lines[29:] != [b"VR STOP", b"VR PASS", b""]:
        return False
    try:
        frequency, quantum = _fields(lines[3], b"VR CLOCK", b"freq quantum")
        names = b"t id ready start cval irq charge a2 b2 floor2 elr sp progress"
        rows = [_Slot(*_fields(line, b"VR SLOT", names)) for line in lines[4:25]]
        share = _fields(lines[25], b"VR SHARE slots=12", b"a b error tolerance")
        sleep = _fields(lines[26], b"VR SLEEP t=12 id=1", b"before2 floor2")
        wake = _fields(lines[27], b"VR WAKE t=15 id=1", b"before2 floor2 after2 lag_before lag_after")
        end = _fields(lines[28], b"VR END slots=21", b"a b floor2 lag_b")
    except ValueError:
        return False
    if not 50 <= frequency <= 0xffffffff or quantum != frequency // 50:
        return False

    # Twice virtual runtime is an exact common integer grid for weights 1, 2.
    v = [0, 0]
    service = [0, 0]
    progress = [1, 1]
    floor = 0
    previous_irq = 0
    recovery = set()
    for t, row in enumerate(rows):
        if t == 12:
            error = abs(2 * service[0] - service[1])
            if not all(service) or error > 4 * quantum:
                return False
            if share != (service[0], service[1], error, 4 * quantum):
                return False
            floor = v[0]  # A is the sole ready task immediately after B sleeps.
            if sleep != (v[1], floor):
                return False
        if t == 15:
            before = v[1]
            if before >= floor:
                return False
            v[1] = floor  # A's integer runtime lies exactly on B's half-tick grid.
            if wake != (before, floor, v[1], floor - before, 0):
                return False

        mask = 1 if 12 <= t < 15 else 3
        winner = 1 if mask == 3 and v[1] < v[0] else 0
        if (row.t, row.task, row.ready) != (t, winner, mask):
            return False
        if row.start < previous_irq or row.cval != row.start + quantum:
            return False
        if not row.cval <= row.irq < row.cval + quantum:
            return False
        elapsed = row.irq - row.start
        if row.charge != elapsed or row.progress <= progress[winner]:
            return False
        previous_irq = row.irq
        progress[winner] = row.progress
        service[winner] += elapsed
        v[winner] += elapsed * (2 if winner == 0 else 1)
        floor = max(floor, min(v) if mask == 3 else v[0])
        if (row.a2, row.b2, row.floor2) != (v[0], v[1], floor):
            return False
        if t >= 15:
            recovery.add(winner)

    lag_b = floor - v[1]
    return (
        recovery == {0, 1}
        and -2 * quantum <= lag_b <= 0
        and end == (service[0], service[1], floor, lag_b)
        and _frames_match(rows)
    )
