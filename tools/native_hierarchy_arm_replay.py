"""Independent reconstruction of native hierarchy reservations and actionable timer targets."""
MAX = 2**64 - 1
DISTANCE = 2**63 - 1


def check(ok, message):
    if not ok:
        raise ValueError(message)


class Hierarchy:
    def __init__(self, config, groups):
        _, self.epoch, _, self.period, parents, children, leaves = config
        check(1 <= parents <= 2 and 1 <= children <= 3 and 1 <= leaves <= 3, "invalid topology size")
        self.paths = []
        self.quota = {}
        for level, (name, count) in enumerate(zip(("PARENT", "CHILD", "LEAF"), (parents, children, leaves))):
            rows = groups[name]
            check(len(rows) == count, f"missing {name} configuration")
            for i, row in enumerate(rows):
                check(len(row) == (2 if level == 0 else 3) and row[0] == i
                      and 0 < row[-1] <= self.period, f"invalid {name} configuration")
                self.quota[level, i] = row[-1]
                if level:
                    check((level - 1, row[1]) in self.quota, f"invalid {name} parent")
                if level == 2:
                    child = row[1]
                    self.paths.append(((0, groups["CHILD"][child][1]), (1, child), (2, i)))
        self.balance = dict(self.quota)
        self.last = self.epoch
        self.owner = 3
        self.reservation = 0

    def decision(self, row, arm):
        now, source, target, ready, woke, exited = row[4], *row[8:13]
        slots = len(self.paths)
        mask = (1 << slots) - 1
        check(self.owner != 4 and self.epoch <= self.last <= now and source == self.owner, "invalid settlement source/time")
        window = (now - self.epoch) // self.period
        previous = (self.last - self.epoch) // self.period
        elapsed = now - self.last
        check(row[17] == (elapsed if source < slots else 0) and
              row[18] == (max(0, elapsed - self.reservation) if source < slots else 0), "invalid grant settlement")
        check((row[15], row[16]) == ((self.last, self.last + self.reservation) if source < slots else (0, 0)),
              "previous grant endpoints mismatch")
        active = self.paths[source] if source < slots else ()
        for node, credit in self.quota.items():
            left = self.balance[node]
            if node in active:
                if previous == window:
                    left -= elapsed
                else:
                    boundary = self.epoch + (previous + 1) * self.period
                    left = min(credit, left - (boundary - self.last) + credit)
                    left -= (window - previous - 1) * (self.period - credit) + (now - self.epoch) % self.period
            else:
                left = min(credit, left + (window - previous) * credit)
            self.balance[node] = left
        debts = [max(0, -self.balance[node]) for node in active] if active else [0] * 3
        check(row[29:32] == debts, "recorded settlement debt mismatch")
        sleep = row[32:35]
        check(not any(sleep[slots:]) and all(d == 0 or d > now for d in sleep)
              and not (exited | ready | woke) & ~mask and not exited & ready and not woke & ~ready
              and ready == mask & ~exited & ~sum(1 << i for i, d in enumerate(sleep) if d),
              "invalid ready, sleep or exit masks")
        due_wake = min((d for d in sleep if d), default=None)
        waits = []
        eligible = []
        for i, path in enumerate(self.paths):
            left = [self.balance[n] for n in path]
            check(row[35+3*i:38+3*i] == [max(0, v) for v in left], "path balance mismatch")
            if ready & (1 << i):
                wait = max(0 if v > 0 else (-v) // self.quota[n] + 1 for n, v in zip(path, left))
                waits.append(wait)
                if wait == 0 and now < MAX:
                    eligible.append(i)
        reserved = 0
        policy_due = None
        if eligible:
            selected = min(eligible, key=self.paths.__getitem__)
            check(target == selected, "target differs from eligible hierarchy winner")
            reserved = min(self.period - (now - self.epoch) % self.period, MAX - now,
                           *(self.balance[n] for n in self.paths[selected]))
            policy_due = now + reserved
        elif exited == mask:
            check(target == 4, "all exited requires terminal target")
        else:
            check(target == 3, "idle target required")
            if waits and now < MAX:
                candidate = self.epoch + (window + min(waits)) * self.period
                if candidate < MAX:
                    policy_due = candidate
        check(row[28] == reserved, "invalid reservation")
        kind, due = cause(target, now, policy_due, due_wake)
        check((arm["kind"], arm["due"], row[14]) == (kind, due, due), "arm cause/target/compare mismatch")
        self.owner, self.last, self.reservation = target, now, reserved


def cause(target, now, policy_due, wake):
    if target == 4:
        check(policy_due is None and wake is None, "terminal carries pending work")
        return 6, 0
    due = min((d for d in (policy_due, wake) if d is not None), default=None)
    check(due is not None and due > now, "missing future arm")
    compare = min(due, now + DISTANCE)
    if compare < due:
        return 5, compare
    if target == 3 and due == policy_due:
        return (2 if wake == due else 1), due
    return (3 if due == wake else 4), due
