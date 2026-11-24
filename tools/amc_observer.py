"""Validate the complete LF-normalized UART of the one-CPU CHG-022 guest."""

from typing import NamedTuple


MAX_BYTES = 4096
U64_MAX = (1 << 64) - 1
RAM_START = 0x40200000
RAM_END = 0x48000000
STACK_BYTES = 16 * 1024


def _values(line: bytes, prefix: bytes, names: bytes) -> list[int]:
    """Parse exactly the requested fields in the guest's canonical u64 encoding."""
    if not line.startswith(prefix + b" "):
        raise ValueError
    words = line[len(prefix) + 1:].split(b" ")
    values = []
    keys = names.split()
    if len(keys) != len(words):
        raise ValueError
    for name, word in zip(keys, words):
        key, _, number = word.partition(b"=")
        if key != name or len(number) > 20 or not number.isdigit():
            raise ValueError
        if len(number) > 1 and number.startswith(b"0"):
            raise ValueError
        value = int(number)
        if value > U64_MAX:
            raise ValueError
        values.append(value)
    return values


class _Slot(NamedTuple):
    t: int
    task: int
    mode: int
    used: int
    left: int
    deadline: int
    cval: int
    irq: int
    elr: int
    sp: int
    progress: int
    value: int


# Logical accounting in dispatch order: task, mode, used, left, deadline, x23.
_OUTCOMES = (
    (0, 0, 1, 0, 3, 36),
    (1, 0, 1, 3, 5, 0),
    (1, 1, 2, 2, 5, 0),
    (1, 1, 3, 1, 5, 0),
    (3, 1, 1, 0, 6, 528),
    (0, 0, 1, 0, 8, 136),
)
_STACK_INSTANCES = (0, 2, 2, 2, 4, 5)


def _frames_match(slots: list[_Slot]) -> bool:
    first = slots[0]
    allocation = first.sp + 16 - STACK_BYTES
    if first.sp % 16 or not RAM_START <= allocation <= RAM_END - 6 * STACK_BYTES:
        return False
    for row, instance in zip(slots, _STACK_INSTANCES):
        if row.sp != first.sp + instance * STACK_BYTES:
            return False
        if row.elr % 4 or not RAM_START <= row.elr < allocation:
            return False

    # The first ELR identifies one of two instructions in the initial loop.
    # In amc.S each of four finite entries occupies 52 bytes with its loop
    # at +44. The HI entry follows those entries and starts its loop at +48.
    # This checks relative loop membership independent of the ELF load address.
    # Absolute begin/end symbols are checked against the frozen ELF separately.
    loops = ((0, 8), (212, 248), (212, 248), (212, 248), (104, 112), (156, 164))
    return any(
        all(base + low <= row.elr < base + high for row, (low, high) in zip(slots, loops))
        for base in (first.elr, first.elr - 4)
    )


def validate(transcript: bytes) -> bool:
    """Return whether one bounded, complete transcript satisfies the AMC witness."""
    if not isinstance(transcript, bytes) or len(transcript) > MAX_BYTES:
        return False
    lines = transcript.split(b"\n")
    if len(lines) != 17 or lines[:3] != [b"JNOX M1 BOOT", b"JNOX M1 READY", b"AMC BEGIN"]:
        return False
    if lines[11] != b"AMC GUARD new_lo=0 early_reset=0 fourth=0 fault=1":
        return False
    if lines[13:] != [b"AMC RESET unapproved=0 approved=1", b"AMC STOP", b"AMC PASS", b""]:
        return False
    try:
        frequency, origin, quantum = _values(lines[3], b"AMC CLOCK", b"freq origin quantum")
        fields = b"t id mode used left deadline cval irq elr sp progress value"
        slots = [_Slot(*_values(line, b"AMC SLOT", fields)) for line in lines[4:10]]
        switch_irq, commit, work, latency = _values(
            lines[10], b"AMC SWITCH t=2 carry=2,3,2,1 drop=5", b"irq commit work latency"
        )
        due_irq, limit, result = _values(
            lines[12], b"AMC DEADLINE id=3 end=5 due=6", b"irq limit value"
        )
    except ValueError:
        return False

    if not 20 <= frequency <= 0xffffffff or quantum != frequency // 20:
        return False
    if origin + 7 * quantum > U64_MAX:
        return False
    for t, (row, expected) in enumerate(zip(slots, _OUTCOMES)):
        actual = (row.task, row.mode, row.used, row.left, row.deadline, row.value)
        if row.t != t or actual != expected or row.progress <= 1:
            return False
        if row.cval != origin + (t + 1) * quantum or not row.cval <= row.irq < row.cval + quantum:
            return False
    trigger, resumed, final_hi, survivor = slots[1:5]
    return (
        _frames_match(slots)
        and trigger.progress < resumed.progress < final_hi.progress
        and switch_irq == trigger.irq
        and switch_irq <= commit <= work < resumed.cval
        and latency == work - trigger.cval
        and due_irq == survivor.irq
        and limit == origin + survivor.deadline * quantum
        and due_irq < limit
        and result == survivor.value
    )
