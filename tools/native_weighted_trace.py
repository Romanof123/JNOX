"""Parse CPU 0's actual lower-EL exception and return log in execution order."""
from dataclasses import dataclass
import re

from native_el0_replay import need


@dataclass(frozen=True)
class Transition:
    kind: str
    pc: int
    vector: int = 0
    esr: int = 0


def transitions(raw):
    lines = iter(line for line in raw.decode("ascii").splitlines() if line.strip())

    def take(pattern):
        match = re.fullmatch(pattern, next(lines, ""))
        need(match is not None, "missing or unexpected QEMU record: " + pattern)
        return match

    for line in lines:
        ret = re.fullmatch(
            r"Exception return from AArch64 EL1 to AArch64 EL0 PC (0x[\da-fA-F]+)", line
        )
        if ret:
            yield Transition("eret", int(ret[1], 16))
            continue
        event = re.fullmatch(
            r"Taking exception (2|5) \[(SVC|IRQ)\](?: on CPU 0)?", line
        )
        need(
            event and event.groups() in (("2", "SVC"), ("5", "IRQ")),
            "unexpected exception source",
        )
        take(r"\.\.\.from EL0 to EL1")
        syndrome = take(r"\.\.\.with ESR (0x[\da-fA-F]+)/(0x[\da-fA-F]+)")
        elr = take(r"\.\.\.with ELR (0x[\da-fA-F]+)")
        vector = take(r"\.\.\.to EL1 PC (0x[\da-fA-F]+) PSTATE (0x[\da-fA-F]+)")
        need(int(vector[2], 16) & 15 == 5, "expected EL1h handler")
        if event[2] == "SVC":
            need(
                int(syndrome[1], 16) == 0x15 and int(syndrome[2], 16) == 0x56004A01,
                "unexpected native SVC",
            )
        yield Transition(
            event[2].lower(), int(elr[1], 16), int(vector[1], 16), int(syndrome[2], 16)
        )
