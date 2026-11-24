"""Exact LF-delimited Pi 4 host observation profiles."""


def transcript(prefix, events):
    return b"JNOX PI4 BOOT\nJNOX PI4 READY\n" + b"".join(
        prefix + b" " + event.encode("ascii") + b"\n" for event in events)


PROFILES = {
    "pi4-scheduler": {"scheduler": transcript(b"SCHED", (
        "LOW RUN", "TIMER IRQ", "HIGH RUN", "LOW RESUMED", "PASS"))},
    "pi4-fixed-fifo": {"fifo": transcript(b"FIXED FIFO", (
        "START", "A START", "B YIELD", "A RESUME", "HIGH", "A DONE", "B SLEEP", "B DONE"))},
    "pi4-fixed-rr": {
        "rr-a-first": transcript(b"FIXED RR", (
            "START", "A START", "HIGH", "B START", "A DONE", "B DONE")),
        "rr-high-first": transcript(b"FIXED RR", (
            "START", "HIGH", "A START", "B START", "A DONE", "B DONE")),
    },
}


class Matcher:
    """Keep only offsets and the remaining finite alternatives."""
    def __init__(self, profile):
        self.candidates = list(PROFILES[profile].items())
        self.offset = 0
        self.failure_offset = None
        self.variant = None

    def feed(self, chunk):
        for byte in chunk:
            self.candidates = [(name, raw) for name, raw in self.candidates
                               if self.offset < len(raw) and raw[self.offset] == byte]
            if not self.candidates:
                self.failure_offset = self.offset
                self.variant = None
                return False
            self.offset += 1
            self.variant = next((name for name, raw in self.candidates
                                 if self.offset == len(raw)), None)
        return True
