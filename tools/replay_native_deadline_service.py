#!/usr/bin/env python3
"""Replay a single native deadline-service UART capture, without running QEMU.

    python3 tools/replay_native_deadline_service.py out/uart.txt
    python3 tools/replay_native_deadline_service.py out/refusal.txt --expect cap

The JSON report concerns guest scheduling evidence. Verify application-owned
RAM and external exception traces with the oracle appropriate to the ELFs.
"""

import argparse
from collections import deque, namedtuple
import json
from pathlib import Path
import sys


Event = namedtuple("Event", """
    seq kind entry cell source target op src_pc src_sp dst_pc dst_sp src_x0 dst_x0
    old_compare next_compare arm returned physical_start elapsed ordinal
    remaining_a remaining_b fifo terminal failure
""")
SERVICE, IDLE, PARK = 2, 3, 4
WAIT_CALL = 18
LIMIT = (1 << 64) - 1


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_rows(path):
    policy = None
    rows = {"CONFIG": [], "JOB": [], "EVENT": [], "REFUSED": [], "END": []}
    widths = {"CONFIG": 6, "JOB": 4, "EVENT": len(Event._fields), "REFUSED": 3, "END": 3}
    with path.open("rb") as stream:
        for line_no, raw in enumerate(stream, 1):
            fields = raw.split()
            if len(fields) < 3 or fields[0] != b"NATIVE" or fields[2] != b"RECURRING":
                continue
            where = f"line {line_no}"
            require(fields[1] in (b"EDF", b"LLF"), f"{where}: unknown policy")
            current = fields[1].decode()
            require(policy in (None, current), f"{where}: mixed policy captures")
            policy = current
            require(len(fields) >= 4, f"{where}: truncated recurring row")
            label = fields[3].decode("ascii")
            require(label in widths and len(fields[4:]) == widths.get(label),
                    f"{where}: invalid {label} record width")
            require(all(field.isdigit() for field in fields[4:]), f"{where}: invalid unsigned field")
            values = tuple(map(int, fields[4:]))
            require(all(value <= LIMIT for value in values), f"{where}: field exceeds u64")
            require(not rows["END"] and not rows["REFUSED"], f"{where}: record after disposition")
            require(label == "CONFIG" or rows["CONFIG"], f"{where}: missing initial CONFIG")
            require(label in ("CONFIG", "JOB") or len(rows["JOB"]) == 2, f"{where}: missing job descriptors")
            rows[label].append(values)
    require(len(rows["CONFIG"]) == 1, "expected one recurring CONFIG")
    require(len(rows["JOB"]) == 2, "expected two recurring JOB rows")
    return policy, rows


def insufficient_interval(jobs, cells):
    for start in range(16):
        for end in range(start + 1, 17):
            demand = sum(cost for release, due, cost in jobs.values() if start <= release and due <= end)
            supply = sum(k not in cells for k in range(start, end))
            if demand > supply:
                return [start, end]
    return None


class JobReplay:
    def __init__(self, jobs, policy):
        self.jobs = jobs
        self.policy = policy
        self.cell = -1
        self.remaining = {}
        self.queue = []
        self.running = None

    def advance(self, cell):
        if self.running is not None:
            left = self.remaining[self.running]
            self.remaining[self.running] = max(1, left - (cell - self.cell))
        for pid, (release, _, cost) in sorted(self.jobs.items(), key=lambda item: (item[1][0], item[0])):
            if self.cell < release <= cell:
                self.remaining[pid] = cost
                self.queue.append(pid)
        self.cell = cell

    def finish(self, pid):
        require(self.running == pid, "exit does not belong to selected job")
        del self.remaining[pid]
        self.running = None

    def choose(self, reserved):
        if reserved:
            if self.running is not None:
                self.queue.insert(0, self.running)
                self.running = None
            return None
        def rank(pid):
            due = self.jobs[pid][1]
            return due if self.policy == "EDF" else due - self.cell - self.remaining[pid]

        if self.queue:
            winner = min(self.queue, key=rank)
            if self.running is None or rank(winner) < rank(self.running):
                self.queue.remove(winner)
                if self.running is not None:
                    self.queue.append(self.running)
                self.running = winner
        return self.running


def modeled_miss(jobs, cells, policy):
    model = JobReplay(jobs, policy)
    for k in range(17):
        model.advance(k)
        if any(jobs[pid][1] <= k for pid in model.remaining):
            return True
        winner = model.choose(k in cells)
        if winner is not None and model.remaining[winner] == 1:
            model.finish(winner)
    return False


def refusal_name(reason, cap, interval, miss):
    if reason == 10 and cap < 6:
        return "cap"
    if (reason == 8 and interval is not None) or (reason == 9 and miss):
        return "supply"
    raise ValueError(f"unsupported or unjustified refusal {reason}")


def replay_events(rows, jobs, cells, epoch, q, cap, policy):
    events = [Event(*values) for values in rows["EVENT"]]
    require(events and rows["END"] == [(0, len(events), 0)], "incomplete or failed terminal journal")
    model = JobReplay(jobs, policy)
    previous_stamp = epoch
    owner = IDLE
    saved = {}
    exited = {}
    starts = []
    completions = []
    window = deque()
    peak = 0
    last_grid = None
    for number, event in enumerate(events):
        prefix = f"event {number}"
        def check(ok, message):
            require(ok, f"{prefix}: {message}")

        k = event.cell
        check(event.seq == number and event.failure == 0, "sequence or failure")
        check(previous_stamp <= event.entry and (event.entry - epoch) // q == k, "entry/cell mismatch")
        check(event.source == owner, "source differs from preceding return")
        terminal = number == len(events) - 1
        check(event.terminal == int(terminal) and (event.target == PARK) == terminal, "terminal marker")
        check(event.target in range(5), "invalid return owner")
        if event.source in (0, 1, SERVICE):
            check(event.src_pc and event.src_sp, "missing interrupted frame")
            saved[event.source] = (event.src_pc, event.src_sp)

        model.advance(k)
        if event.kind == 1:
            check(event.op == 30, "unexpected IRQ")
            check(k == (0 if last_grid is None else last_grid+1), "missing or duplicate grid IRQ")
            check(event.old_compare == epoch + k*q and event.entry >= event.old_compare, "late or early grid IRQ")
            last_grid = k
            if k in cells:
                check(event.target == SERVICE and k not in starts, "reserved cell entry")
                starts.append(k)
                if len(starts) > 1:
                    check(event.dst_x0 == len(starts)-1, "wait returned wrong activation ordinal")
        elif event.kind == 2:
            check(last_grid == k and event.old_compare == epoch + (k+1)*q, "syscall outside armed cell")
            if owner == SERVICE:
                check(starts and k == starts[-1] and k not in completions, "service outside activation")
                check(event.op == (1 if k == cells[-1] else WAIT_CALL), "incorrect service wait/exit")
                check(event.target == IDLE, "unused reserved tail must idle")
                completions.append(k)
            else:
                check(owner in jobs and owner not in exited and event.op == 1, "unexpected scheduling syscall")
                check(event.entry <= epoch + jobs[owner][1]*q, "actual job exit missed deadline")
                exited[owner] = event.entry
                model.finish(owner)
        else:
            check(False, "unexpected exception kind")

        selected = model.choose(k in cells)
        if k not in cells and not terminal:
            check(event.target == (IDLE if selected is None else selected), "deadline policy selection mismatch")
        if not terminal:
            ready_fifo = 0
            for pid in model.queue:
                ready_fifo = (ready_fifo << 4) | (pid + 1)
            check(event.fifo == ready_fifo, "ready FIFO differs from policy replay")
            check((event.remaining_a, event.remaining_b) == tuple(model.remaining.get(pid, LIMIT) for pid in (0, 1)),
                  "live estimate differs from policy replay")
            check(event.next_compare == epoch + (k+1)*q, "next compare off absolute grid")
            check(event.returned >= event.entry and (event.returned-epoch)//q == k, "return crossed cell")
            if event.target in (0, 1, SERVICE):
                check(event.dst_pc and event.dst_sp, "missing return frame")
                if event.target in saved:
                    check(saved[event.target] == (event.dst_pc, event.dst_sp), "saved frame differs from restore")
            if event.target in jobs:
                check(k not in cells and k >= jobs[event.target][0] and event.target not in exited,
                      "dispatched unavailable job")
            elif event.target == SERVICE:
                check(k in cells and k not in completions, "unexpected service dispatch")
            elif k not in cells:
                check(all(release > k or pid in exited for pid, (release, _, _) in jobs.items()),
                      "idle while regular work ready")

        if not terminal and owner != event.target:
            while window and event.entry - window[0] >= q:
                window.popleft()
            window.append(event.entry)
            peak = max(peak, len(window))
        previous_stamp = event.entry if terminal else event.returned
        owner = event.target

    require(starts == cells and completions == cells, "missing or displaced service activation")
    require(len(exited) == 2 and events[-1].entry >= epoch + (cells[-1]+1)*q, "early terminal park")
    require(peak <= min(cap, 6), "sliding-Q owner-change bound exceeded")
    return {"verdict": "accepted", "events": len(events), "peak_changes_per_Q": peak,
            "service_cells_completed": completions, "job_exit_counters": exited}


def replay(path, expect):
    policy, rows = read_rows(path)
    version, epoch, q, hz, mask, cap = rows["CONFIG"][0]
    require(version == 1 and q > 0 and hz == 10*q and 0 < mask < 1 << 16, "invalid v1 configuration")
    cells = [k for k in range(16) if mask & (1 << k)]
    jobs = {pid: (release, due, cost) for pid, release, due, cost in rows["JOB"]}
    require(set(jobs) == {0, 1} and all(0 <= release < due <= 16 and cost for release, due, cost in jobs.values()),
            "invalid job tuples")
    interval = insufficient_interval(jobs, cells)
    miss = modeled_miss(jobs, cells, policy)
    if rows["REFUSED"]:
        require(len(rows["REFUSED"]) == 1 and not rows["EVENT"], "refused run contains dispatch records")
        reason, arms, entries = rows["REFUSED"][0]
        require(arms == entries == 0, "refusal after operational arm or user entry")
        disposition = refusal_name(reason, cap, interval, miss)
        require(expect in ("auto", disposition), f"expected {expect} but got {disposition}")
        report = {"verdict": "refused", "reason": disposition, "reason_code": reason, "deficient_interval": interval}
    else:
        require(expect in ("auto", "accepted") and cap >= 6 and interval is None and not miss, "unexpected admission")
        report = replay_events(rows, jobs, cells, epoch, q, cap, policy)
    return dict(report, policy=policy, calendar=cells, cap=cap, quantum=q)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("uart", type=Path)
    parser.add_argument("--expect", choices=["auto", "accepted", "cap", "supply"], default="auto")
    args = parser.parse_args()
    try:
        report = replay(args.uart, args.expect)
    except (OSError, ValueError) as error:
        print(f"Replay failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
