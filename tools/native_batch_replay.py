#!/usr/bin/env python3
"""Decode native SJF/SRTF/HRRN capture artifacts into an operator decision ledger."""
import argparse
from collections import defaultdict, namedtuple
import json
from pathlib import Path
import re
import sys

from native_el0_replay import PA_MASK, Snapshot, argument_stacks, elf_file, permissions, symbol, vread
from ordinary_amc_capture import sha

MAX = (1 << 64) - 1
EVENT = namedtuple("Event", """seq kind entry decision op arg result src dst releases wakes
    admission ready released sleeping exited queue prior_compare next_compare cval ctl
    src_pc src_sp src_pstate src_root dst_pc dst_sp dst_pstate dst_root src_frame dst_frame
    last_return segment returned idle_x eoi sleep_target transparent_count transparent_ticks""")


class Incomplete(Exception):
    pass


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def present(condition, reason):
    if not condition:
        raise Incomplete(reason)


def decode(uart):
    rows = defaultdict(list)
    widths = dict(CONFIG=5, JOB=4, EVENT=len(EVENT._fields), STATE=6, DROPPED=1)
    for line in uart.splitlines():
        if not line.startswith(b"NATIVE BATCH "):
            continue
        tag, *parts = line[13:].decode("ascii").split()
        if tag in ("FAULT", "REFUSED"):
            raise ValueError(line.decode("ascii"))
        if tag in widths:
            require(len(parts) == widths[tag] and all(x.isdecimal() for x in parts), f"malformed {tag}")
            values = tuple(map(int, parts))
            require(all(x <= MAX for x in values), f"{tag} field overflow")
            rows[tag].append(values)
    for tag in ("CONFIG", "DROPPED"):
        present(rows[tag], f"missing {tag}")
        require(len(rows[tag]) == 1, f"duplicate {tag}")
    return rows


def slot_statuses(uart, n):
    if n == 1:
        matches = re.findall(rb"(?m)^NATIVE EL0 EXIT (-?\d+)\r?$", uart)
        present(len(matches) == 1, "missing solo exit")
        return [int(matches[0])]
    matches = re.findall(rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$", uart)
    exits = {int(i): int(code) for i, code in matches}
    present(len(exits) == n, "missing process exit")
    require(len(matches) == n and sorted(exits) == list(range(n)), "invalid process exits")
    return [exits[i] for i in range(n)]


def product(role, elf, mem, stack, status):
    fields = {}
    pages = set()
    def read(name, width):
        addr, size = symbol(elf, name)
        require(size == width, name + " size mismatch")
        permissions(mem, stack["root"], addr, "user rw")
        pages.add(mem.leaf(stack["root"], addr) & PA_MASK)
        value = int.from_bytes(vread(mem, stack["root"], addr, size), "little")
        fields[name] = value
        return value

    if role == "batch":
        count = read("private_progress", 4)
        argv = stack["argv"][1:]
        ok = (not argv and 1 <= count <= 65536) or (len(argv) == 2 and argv[0] == "--blocks"
                                                     and int(argv[1]) == count and count > 0)
        ok = read("private_result", 8) == 522240 * count and ok and status == 17
    elif role == "urgent":
        ok = read("private_progress", 4) == 8 and read("private_result", 4) == 2048 and status == 43
    elif role == "checksum":
        ok = read("private_blocks", 4) == 16 and read("private_result", 8) == 8355840 and status == 17
    elif role in ("sleep_first", "sleep_last"):
        expected = 53 if role == "sleep_first" else 59
        request = read("request_ns", 8)
        ok = (read("observed_ns", 8) >= request > 0 and read("private_progress", 4) == 2
              and read("private_result", 4) == status == expected)
    else:
        raise ValueError("unknown product " + role)
    require(ok, f"slot {stack['slot']} {role} output mismatch")
    return dict(role=role, slot=stack["slot"], exit=status, physical_pages=sorted(pages), **fields)


def ledger(rows, manifest, stacks):
    version, mode, epoch, frequency, n = rows["CONFIG"][0]
    require(version == 1 and frequency and n == len(stacks) and mode in range(3)
            and manifest["policy"] == ("sjf", "srtf", "hrrn")[mode], "batch configuration mismatch")
    present(len(rows["JOB"]) == n, "missing job records")
    due = {}
    estimates = {}
    roots = {}
    for i, offset, estimate, root in rows["JOB"]:
        require(i in range(n) and i not in due and (root & PA_MASK) == stacks[i]["root"]
                and epoch + offset <= MAX and estimate > 0, "job configuration mismatch")
        require(manifest["slots"][i]["batch_job"] == dict(offset_ticks=offset, estimate_ticks=estimate),
                "package batch tuple mismatch")
        due[i], estimates[i], roots[i] = epoch + offset, estimate, root
    states = {}
    for seq, slot, remain, ticks, wait, wake in rows["STATE"]:
        require((seq, slot) not in states and slot in range(n), "duplicate or invalid slot state")
        states[seq, slot] = dict(remaining=remain, service=ticks, eligible_wait=wait, wake=wake)
    events = []
    previous_dst = 3
    last_entry = 0
    wake_targets = {}
    released = 0
    service = [0] * n
    waits = [0] * n
    prior_queue = []
    last_decision = 0
    for seq, values in enumerate(rows["EVENT"]):
        e = EVENT(*values)
        require(e.seq == seq and e.kind in range(4) and e.src == previous_dst and
                e.dst in (*range(n), 3, 4) and last_entry <= e.entry <= e.decision, "event continuity")
        if e.kind == 1:
            require(e.op & 1023 == 30 and e.eoi == e.op, "PPI30 acknowledgement mismatch")
        if e.src < n:
            require(e.src_root == roots[e.src] and e.src_frame and e.src_sp and e.src_pc and
                    (e.segment == 0 if e.kind == 3 else e.entry >= e.last_return and
                     e.segment == e.entry - e.last_return),
                    f"event {seq} outgoing service or frame")
            service[e.src] += e.segment + e.transparent_ticks
        if e.dst < n:
            require(e.dst_root == roots[e.dst] and e.dst_frame and e.dst_pc and e.dst_sp
                    and e.ready & (1 << e.dst), f"event {seq} selected frame or readiness")
        arrivals = []
        if e.sleep_target and e.src < n:
            wake_targets[e.src] = e.sleep_target
        for i in range(n):
            if e.releases >> i & 1:
                require(not released >> i & 1 and due[i] <= e.decision, "invalid release")
                arrivals.append((due[i], 0, i))
            if e.wakes >> i & 1:
                require(i in wake_targets and wake_targets[i] <= e.decision, "invalid wake")
                arrivals.append((wake_targets.pop(i), 1, i))
        expected_releases = sum(1 << i for i in range(n) if not released >> i & 1 and due[i] <= e.decision)
        require(e.releases == expected_releases and all(t > e.decision for t in wake_targets.values()),
                "missing due admission")
        ordering = 0
        for _, kind, slot in sorted(arrivals):
            ordering = (ordering << 4) | (slot + 1 + 4 * kind)
        require(e.admission == ordering, "admission order mismatch")
        if e.prior_compare:
            require(e.cval == e.prior_compare, "prior hardware compare mismatch")
        queue = []
        encoded = e.queue
        while encoded:
            queue.append((encoded & 15) - 1)
            encoded >>= 4
        queue.reverse()
        require(len(set(queue)) == len(queue) and all(i in range(n) for i in queue),
                "invalid encoded ready queue")
        released |= e.releases
        sleeping = sum(1 << i for i in wake_targets)
        require(e.released == released and e.sleeping == sleeping and
                e.ready == released & ~sleeping & ~e.exited, "process masks mismatch")
        future = [due[i] for i in range(n) if not released >> i & 1] + list(wake_targets.values())
        expected_compare = min(min(future), e.decision + (MAX >> 1)) if future else 0
        require(e.next_compare == expected_compare, "next compare mismatch")
        require(set(queue) == {i for i in range(n) if e.ready >> i & 1 and i != e.dst},
                "runnable queue membership mismatch")
        require(e.decision >= last_decision, "decision counter regressed")
        for slot in prior_queue:
            waits[slot] += e.decision - last_decision
        for at, _, slot in arrivals:
            waits[slot] += e.decision - at
        prior_queue, last_decision = queue, e.decision
        snapshot = []
        for i in range(n):
            present((seq, i) in states, f"missing state for event {seq} slot {i}")
            state = states.pop((seq, i))
            if state["service"] != MAX:
                require(state["service"] == service[i], f"event {seq} slot {i} cumulative charge")
            if state["eligible_wait"] != MAX:
                require(state["eligible_wait"] == waits[i], f"event {seq} slot {i} eligible wait")
            if state["remaining"] != MAX:
                require(state["remaining"] == max(1, estimates[i] - service[i]), "remaining mismatch")
            require(state["wake"] == wake_targets.get(i, 0), "stored wake target mismatch")
            snapshot.append(dict(slot=i, **state))
        decision = ("park" if e.dst == 4 else "idle" if e.dst == 3 else
                    "dispatch" if e.src == 3 else "retain" if e.src == e.dst else
                    "preempt" if e.ready & (1 << e.src) and not (e.kind == 2 and e.op == 8) else "switch")
        events.append(dict(**e._asdict(), action=decision, states=snapshot, queued_slots=queue,
                           arrivals=[dict(due=t, reason=("release", "wake")[kind], slot=i,
                                          handled=e.decision) for t, kind, i in sorted(arrivals)]))
        previous_dst, last_entry = e.dst, e.entry
    require(not states, "orphan slot state")
    return dict(epoch=epoch, frequency=frequency, estimates=estimates, due=due, events=events,
                charged_ticks=service, dropped=rows["DROPPED"][0][0])

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--session-sha256", help="expected SHA256 printed by the capture command")
    parser.add_argument("--products", help="ordered comma-separated workload roles, one per ELF")
    args = parser.parse_args()
    report = {}
    try:
        capture = args.capture
        report["session_sha256"] = sha(capture / "session.json")
        if args.session_sha256:
            require(report["session_sha256"] == args.session_sha256, "session hash mismatch")
        session = json.loads((capture / "session.json").read_bytes())
        launch = json.loads((capture / "launch.json").read_bytes())
        manifest = json.loads((args.package / "manifest.json").read_bytes())
        for name in ("launch.json", "kernel.elf", "uart.bin", "ram.bin"):
            present(name in session["hashes"], f"missing captured {name}")
            require(sha(capture / name) == session["hashes"][name], f"{name} digest mismatch")
        require(launch["argument_package"] == manifest and manifest["kernel_elf_sha256"] ==
                launch["kernel_sha256"] == session["hashes"]["kernel.elf"], "package mismatch")
        uart = (capture / "uart.bin").read_bytes()
        mem = Snapshot(launch["ram_base"], (capture / "ram.bin").read_bytes())
        stacks = argument_stacks((capture / "kernel.elf").read_bytes(), mem, manifest, uart)
        report["policy"] = manifest["policy"]
        report["slots"] = stacks
        report["statuses"] = slot_statuses(uart, len(stacks))
        roles = args.products.split(",") if args.products else []
        require(not roles or len(roles) == len(stacks), "provide one product per slot")
        report["products"] = []
        backing = set()
        for i, slot in enumerate(manifest["slots"]):
            filename = slot["elf"]
            require(Path(filename).name == filename, "invalid package ELF name")
            path = args.package / filename
            require(sha(path) == slot["elf_sha256"], "slot ELF mismatch")
            if roles:
                item = product(roles[i], elf_file(path.read_bytes()), mem, stacks[i], report["statuses"][i])
                require(backing.isdisjoint(item["physical_pages"]), "shared product backing")
                backing.update(item["physical_pages"])
                report["products"].append(item)
        observed = ledger(decode(uart), manifest, stacks)
        report["ledger"] = observed
        present(not observed["dropped"], "bounded event journal dropped records")
        present(observed["events"] and observed["events"][-1]["dst"] == 4, "missing terminal decision")
        report["status"] = "complete"
    except (FileNotFoundError, Incomplete) as exc:
        report.update(status="incomplete", detail=str(exc))
    except (ValueError, KeyError, OSError, IndexError, TypeError) as exc:
        report.update(status="mismatch", detail=str(exc))
    print(json.dumps(report, indent=2))
    return {"complete": 0, "mismatch": 1, "incomplete": 2}[report["status"]]


if __name__ == "__main__":
    sys.exit(main())
