"""Bounded UART matcher for the two EVD-129 native Pi 4 prime workers."""

PROFILES = ("pi4-native-fifo", "pi4-native-rr")
APP_HASHES = (
    "4e106b3e8355870dcc41bb015be760233d4eeb145de8654d8569e56513e386fa",
    "915e86f291706cbb460dadd8b0e21905cab534285bfd8c61ea5aba75f4e7b0c8",
)
NONE = (1 << 64) - 1
EVENT = b"NATIVE FIXED EVENT "
FREQ = b"NATIVE PI4 CNTFRQ "
HEADERS = (b"JNOX PI4 BOOT", b"JNOX PI4 READY", None,
           b"NATIVE EL0 ELF " + APP_HASHES[0].encode(),
           b"NATIVE EL0 PEER ELF " + APP_HASHES[1].encode())
DROP = b"NATIVE FIXED DROPPED "
TAIL = (b"NATIVE EL0 EXIT PID 0 12",
        b"NATIVE EL0 EXIT PID 1 77")


def number(raw):
    if not raw.isdigit() or len(raw) > 20 or (len(raw) > 1 and raw.startswith(b"0")):
        raise ValueError
    value = int(raw)
    if value > NONE:
        raise ValueError
    return value


class Matcher:
    """feed accepts valid prefixes; variant is set only after the final LF."""

    def __init__(self, profile):
        if profile not in PROFILES:
            raise ValueError("unknown native prime profile")
        self.profile = profile
        self.rr = profile == "pi4-native-rr"
        self.offset = 0
        self.failure_offset = self.variant = None
        self.packet_complete = self.journal_complete = False
        self.observation = self.dropped = None
        self.line = bytearray()
        self.header = self.tail = self.events = self.rotations = 0
        self.frequency = self.tick = self.compare = 0
        self.current = NONE
        self.live = {0, 1}
        self.service = [0, 0]
        self.saved = [(0x400000, 16846768)] * 2
        self.budgets = [1250000 if self.rr else 0] * 2

    def _event(self, line):
        if self.events >= 128 or not line.startswith(EVENT):
            return False
        fields = [number(v) for v in line[len(EVENT):].split(b" ")]
        if len(fields) != 21:
            return False
        now, why, arg, old, new, pc, sp, rpc, rsp, wake, expiry, compare = fields[:12]
        if old != self.current or arg or wake or expiry != self.compare:
            return False
        if self.events == 0:
            if (why, new, pc, sp) != (0, 0, 0, 0):
                return False
        else:
            if old not in self.live or now <= self.tick or pc == 0 or pc % 4 or sp == 0 or sp % 16:
                return False
            elapsed = now - self.tick
            self.service[old] += elapsed
            self.budgets[old] = max(0, self.budgets[old] - elapsed)
            self.saved[old] = (pc, sp)
            if why == 3:
                if self.rr and len(self.live) == 2 and self.rotations < 2:
                    return False
                self.live.remove(old)
                if new != next(iter(self.live), NONE):
                    return False
            elif why == 4 and self.rr and len(self.live) == 2 and expiry and now >= expiry:
                if new != 1 - old:
                    return False
                self.rotations += 1
                self.budgets[old] = 1250000
            elif why == 4 and new == old:
                # An early or unarmed timer poll preserves the owner.
                pass
            else:
                return False
        if (rpc, rsp) != (self.saved[new] if new in self.live else (0, 0)):
            return False
        waiting = [pid for pid in self.live if pid != new]
        if fields[12:15] != waiting + [NONE] * (3 - len(waiting)):
            return False
        expected = now + self.budgets[new] if self.rr and waiting else 0
        if compare != expected or fields[15:18] != self.service + [0] or fields[18:] != self.budgets + [0]:
            return False
        self.current, self.tick, self.compare = new, now, compare
        self.events += 1
        return True

    def _line(self, line):
        if self.header < len(HEADERS):
            expected = HEADERS[self.header]
            if expected is None:
                if not line.startswith(FREQ):
                    return False
                self.frequency = number(line[len(FREQ):])
                if not 0 < self.frequency <= 0xffffffff:
                    return False
            elif line != expected:
                return False
            self.header += 1
            return True
        if self.dropped is None:
            if not line.startswith(DROP):
                return bool(self.live) and self._event(line)
            dropped = number(line[len(DROP):])
            # The producer fills all 128 rows, then drops only the suffix.
            # Each remaining live worker must have a distinct exit event there.
            if dropped:
                if self.events != 128 or not self.live or dropped < len(self.live):
                    return False
            elif self.live:
                return False
            self.dropped = dropped
            return True
        if line != TAIL[self.tail]:
            return False
        self.tail += 1
        if self.tail == len(TAIL):
            self.packet_complete = True
            self.journal_complete = self.dropped == 0
            if self.journal_complete:
                self.observation = "complete-journal"
            elif self.rr and self.rotations >= 2:
                self.observation = "rr-alternation-retained-prefix"
            elif not self.rr and len(self.live) == 1:
                self.observation = "fifo-retention-through-first-exit"
            else:
                self.observation = "inconclusive-retained-prefix"
                return True
            self.variant = self.profile
        return True

    def feed(self, chunk):
        if self.failure_offset is not None:
            return False
        for byte in chunk:
            ok = not self.packet_complete
            if ok and byte == 10:
                try:
                    ok = self._line(bytes(self.line))
                except ValueError:
                    ok = False
                self.line.clear()
            elif ok and 32 <= byte <= 126 and len(self.line) < 512:
                self.line.append(byte)
            else:
                ok = False
            if not ok:
                self.failure_offset = self.offset
                self.variant = self.observation = None
                self.packet_complete = self.journal_complete = False
                return False
            self.offset += 1
        return True
