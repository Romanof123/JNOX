"""CHG-061 host-side parsing of the pinned QEMU 6.2 log backend."""
import collections
import re

from permit_phase_observations import summarize

DEVICE = "/machine/unattached/device[6]"
HEX = r"(0x[0-9a-fA-F]+)"
FORMATS = {
    "reset": r"reset",
    "write": rf"offset {HEX} value {HEX}",
    "read": rf"offset {HEX} value {HEX}",
    "set_output": r"setting output ([0-7]) to ([01])",
    "input_change": r"input ([0-7]) changed to ([01])",
    "update": rf"GPIODIR {HEX} GPIODATA {HEX} pullups {HEX} floating {HEX}",
    "update_istate": rf"GPIORIS {HEX} GPIOIE {HEX} interrupt level ([01])",
}
REDUCED = ",".join("trace:pl061_" + name for name in FORMATS if name != "read")


class ContainmentError(ValueError):
    """An actual or reported containment alarm, never a normal timing result."""


class Lines:
    def __init__(self, receive):
        self.receive = receive
        self.tail = b""

    def feed(self, chunk):
        parts = (self.tail + chunk).split(b"\n")
        self.tail = parts.pop()
        if len(self.tail) > 4096:
            raise ValueError("unterminated oversized line")
        for line in parts:
            if len(line) > 4096:
                raise ValueError("oversized log line")
            self.receive(line.rstrip(b"\r").decode("ascii"))

    def finish(self):
        if self.tail:
            raise ValueError("truncated final line")


class Guard:
    def __init__(self, arm):
        self.arm = arm
        self.counts = collections.Counter()
        self.other_devices = collections.Counter()
        self.writes = []
        self.output_zero = False
        self.device_sequence = []
        self.irqs = []
        self.svcs = []
        self.returns = []
        self.exception = None
        self.terminal = False

    def uart(self, line):
        if "CONTAINMENT FAIL" in line:
            raise ContainmentError(line)
        words = line.split()
        if len(words) == 4 and all(t.isdecimal() for t in words):
            tag, _, a, b = map(int, words)
            if (tag in (81, 82) and b & 1) or (tag in (18, 19) and a & 1):
                raise ContainmentError(line)
        if self.terminal:
            raise ValueError("UART after terminal marker")
        self.terminal = line == "CONTROL END"

    def mechanism(self, line):
        if line.startswith("pl061_"):
            fields = line.split(" ", 2)
            if len(fields) != 3:
                raise ValueError("malformed PL061 record")
            name, device, body = fields
            name = name[6:]
            match = re.fullmatch(FORMATS.get(name, r"(?!)"), body)
            if not match:
                raise ValueError("unknown or malformed PL061 record: " + line)
            if device != DEVICE:
                self.other_devices[device] += 1
                return
            args = [int(x, 0) for x in match.groups()]
            self.counts[name] += 1
            if name in ("read", "write", "reset"):
                self.device_sequence.append((name, *args))
            if name in ("read", "write"):
                offset, value = args
                if offset & 3 or offset >= 0x1000:
                    raise ValueError("unexpected PL061 offset")
                if offset < 0x400 and offset & 4 and value & 1:
                    raise ContainmentError(line)
                if name == "write":
                    self.writes.append((offset, value))
                elif self.arm == "R":
                    raise ValueError("read logging enabled in reduced arm")
            elif name == "set_output" and args[0] == 0:
                if args[1]:
                    raise ContainmentError(line)
                self.output_zero = True
            elif name == "update" and args[0] & args[1] & 1:
                raise ContainmentError(line)
            if name == "reset" and self.counts[name] != 1:
                raise ValueError("unexpected repeated reset")
            if not self.counts["reset"]:
                raise ValueError("target device event before reset")
            return
        if line.startswith("Taking exception "):
            kinds = {"Taking exception 5 [IRQ]": "IRQ", "Taking exception 2 [SVC]": "SVC"}
            if self.exception or line not in kinds:
                raise ValueError("unresolved or unsupported exception")
            self.exception = kinds[line]
        elif line.startswith("...with ELR "):
            if not self.exception:
                raise ValueError("unmatched ELR")
            address = int(line.split()[-1], 16)
            (self.irqs if self.exception == "IRQ" else self.svcs).append(address)
            self.exception = None
        elif line.startswith("Exception return"):
            match = re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC " + HEX, line)
            if not match or self.exception:
                raise ValueError("invalid ERET")
            self.returns.append(int(match[1], 16))
        elif not line.startswith(("...from EL1 to EL1", "...with ESR ", "...to EL1 PC ")):
            raise ValueError("unexpected mechanism line: " + line)

    def join(self, uart, key):
        outcome = summarize(uart)
        policy, _, arm = key.split("-")
        if (outcome["mode"], outcome["policy"], outcome["arm"]) != (
            "characterization", policy.upper(), "A" + arm
        ):
            raise ValueError("UART identity differs from frozen OFF image")
        lines = uart.read_text(encoding="ascii").splitlines()
        rows = [tuple(map(int, s.split())) for s in lines[lines.index("CONTROL RAW v1") + 1:-1]]
        events = lambda tag: [r for r in rows if r[0] == tag]
        writes = len(events(80))
        if not self.output_zero or self.exception or self.counts["reset"] != 1:
            raise ValueError("incomplete device or exception coverage")
        if self.writes != [(4, 0), (0x400, 1)] + [(4, 0)] * (writes - 1):
            raise ValueError("configuration/data writes disagree with guest ordinals")
        if self.arm == "F":
            seq = self.device_sequence
            for i, record in enumerate(seq):
                if record[:2] == ("write", 4) and (
                    i == 0 or i + 1 >= len(seq)
                    or seq[i-1] != ("read", 4, 0) or seq[i+1] != ("read", 4, 0)
                ):
                    raise ValueError("missing modeled before/after data read")
        if not self.counts["update"] or (self.arm == "F" and self.counts["read"] < 2 * writes):
            raise ValueError("missing device corroboration")
        if self.returns != [r[3] for r in events(93)] or self.irqs != [r[3] for r in events(45)]:
            raise ValueError("external ERET/IRQ PCs differ from guest")
        # SVC entry is observed at the CPU; rows 35 include scheduler returns, not SVC PCs.
        if len(self.svcs) != len(events(35)):
            raise ValueError("SVC count differs from guest dispatch records")
        targets = {}
        irq_delays = []
        for tag, tick, index, value in rows:
            if tag == 2:
                targets[index] = value
            elif tag == 3:
                if index not in targets or tick < targets[index]:
                    raise ValueError("IRQ has no matching preceding absolute compare")
                irq_delays.append(tick - targets[index])
        outcome.update(device_events=dict(self.counts), external_reads=self.counts["read"] if self.arm == "F" else None,
                       external_guest_tick_mapping=None, irq_delays=irq_delays, svc_elrs=self.svcs,
                       first_service=next((r[3] for r in events(8) if r[2] == 0), None),
                       first_phase_delay=next((r[1]-r[3] for r in events(10)), None),
                       first_fault=events(66)[:1], guest_intervals={str(t):events(t) for t in
                       (2,3,4,5,6,8,9,10,17,21,33,34,35,66,76,78,84,90)})
        return outcome
