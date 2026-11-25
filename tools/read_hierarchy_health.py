#!/usr/bin/env python3
"""Read and reconcile the terminal native hierarchy idle-refill health report."""
import argparse
from collections import Counter
import json
from pathlib import Path
import re
import sys

from native_hierarchy_arm_replay import Hierarchy

MAX = 2**64 - 1
SUMMARY = "version period limit arms events scored early invalid late max_lag overflow rows_total stored dropped".split()
ARM = "arm_seq kind due arm_before pre_cval pre_ctl arm_after post_cval post_ctl".split()
ENTRY = "earliest_irq entry_cval entry_ctl entry_counter spsr elr iar idle_landing last_before_unmask daif_before_unmask".split()
TIMER = ["decision_index", *ARM, "old_arm_seq", *ENTRY, "outcome"]
FIRST = ["decision_index", "event_number", "outcome", *ARM, *ENTRY, "lag_present", "lag"]
OUTCOMES = ("other", "scored", "late", "early", "invalid", "stale", "non_ppi")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def words(tokens):
    require(all(re.fullmatch(rb"[0-9]{1,20}", t) for t in tokens), "invalid unsigned decimal")
    result = [int(t) for t in tokens]
    require(all(x <= MAX for x in result), "value exceeds u64")
    return result


def fields(keys, row):
    require(len(keys) == len(row), f"expected {len(keys)} fields, got {len(row)}")
    return dict(zip(keys, row))


def eligible(arm, entry, limit):
    due = arm["due"]
    earliest = entry["earliest_irq"]
    if (arm["post_cval"] != due or entry["entry_cval"] != due
            or arm["post_ctl"] & 3 != 1 or entry["entry_ctl"] & 3 != 1
            or not arm["arm_before"] <= arm["arm_after"] <= earliest <= entry["entry_counter"]):
        return 4
    if earliest < due:
        return 3
    return 2 if earliest - due > limit else 1


def irq_boundary(entry, idle, previous=None):
    earliest = entry["earliest_irq"]
    require(not entry["spsr"] & 0x80, "IRQ interrupted an IRQ-masked context")
    landing = entry["idle_landing"]
    unmask = entry["last_before_unmask"]
    daif = entry["daif_before_unmask"]
    if idle:
        require(entry["spsr"] & 15 == 5 and 0 < landing <= unmask <= earliest and daif & 0x80,
                "invalid idle landing or IRQ unmask boundary")
        if previous is not None:
            require(landing == previous[46], "idle landing differs from retained transfer")
    else:
        require((landing, unmask, daif) == (0, 0, 0), "idle samples on a user IRQ")


def decision_boundary(row, timer, previous, prior_arm):
    i, kind, op, entry, settle, settled, select, selected = row[:8]
    require(entry <= settle <= settled <= select <= selected <= timer["arm_before"],
            f"decision phases out of order at {i}")
    target = row[9]
    if target == 4:
        require(not any(row[44:47]), "terminal transfer samples")
    else:
        require(timer["arm_after"] <= row[45], f"transfer before completed arm at {i}")
        require(row[45] <= row[46] if target == 3 else row[46] == 0,
                f"unexpected idle transfer stamp at {i}")
    if previous is not None:
        require(previous[9] != 4 and max(previous[45:47]) <= entry,
                f"entry before preceding transfer at {i}")
        if prior_arm["post_cval"] == prior_arm["due"] and prior_arm["post_ctl"] & 3 == 1:
            require(timer["pre_cval"] == prior_arm["post_cval"] and
                timer["pre_ctl"] & 3 == prior_arm["post_ctl"] & 3,
                f"previous timer programming differs from pre-arm sample at {i}")
    waits = previous[32:35] if previous is not None else [0]*3
    exited = previous[12] if previous is not None else 0
    if kind == 2:
        require(row[8] < 3, "syscall from idle")
        if op == 1:
            exited |= 1 << row[8]
    require(row[12] == exited, f"exit transition at {i}")
    for slot, deadline in enumerate(waits):
        bit = 1 << slot
        if kind == 2 and op == 7 and row[8] == slot and row[27] == 0:
            # The pointed-to absolute timespec is absent from this journal.
            # A newly requested sleep can mature between entry and settlement.
            continue
        woke = bool(deadline and deadline <= settle)
        require(bool(row[11] & bit) == woke and row[32+slot] == (0 if woke else deadline),
                f"sleep or wake transition at {i} slot {slot}")


def read_uart(path, expected=None):
    one = {}
    groups = {tag: [] for tag in ("PARENT", "CHILD", "LEAF")}
    events, timers, exits = [], [], {}
    ended = False
    with path.open("rb") as uart:
        for line in uart:
            if not line.startswith((b"NATIVE HIERARCHY ", b"NATIVE EL0 EXIT ")):
                continue
            require(line.endswith(b"\n"), "truncated UART record")
            require(not ended, "record after HEALTH END")
            tokens = line.split()
            if tokens[:3] == [b"NATIVE", b"EL0", b"EXIT"]:
                if len(tokens) == 3 + 1:
                    slot, status = 0, tokens[3]
                    multi = False
                else:
                    require(len(tokens) == 6 and tokens[3] == b"PID", "invalid process exit")
                    slot, = words(tokens[4:5])
                    status = tokens[5]
                    multi = True
                require(re.fullmatch(rb"-?[0-9]{1,10}", status) and -(2**31) <= int(status) < 2**31
                        and slot not in exits, "invalid or duplicate process exit")
                exits[slot] = (int(status), multi)
                continue
            tag = tokens[2].decode("ascii")
            data = tokens[3:]
            if tag == "HEALTH" and data[:1] == [b"END"]:
                require(data == [b"END", b"1"], "malformed HEALTH END")
                ended = True
            elif tag in groups:
                groups[tag].append(words(data))
            elif tag == "EVENT":
                row = words(data)
                require(len(row) == 47 and row[0] == len(events), "EVENT width or sequence")
                events.append(row)
            elif tag == "TIMER":
                t = fields(TIMER, words(data))
                require(t["decision_index"] == len(timers), "TIMER sequence")
                timers.append(t)
            elif tag in ("CONFIG", "DROPPED", "FIRST", "HEALTH"):
                require(tag not in one, f"duplicate {tag}")
                one[tag] = words(data)
    require(ended and set(one) == {"CONFIG", "DROPPED", "FIRST", "HEALTH"}, "incomplete terminal report")
    h = fields(SUMMARY, one["HEALTH"])
    config = one["CONFIG"]
    require(len(config) == 7 and config[0] == h["version"] == 1 and config[3] == h["period"] > 0
            and h["limit"] == h["period"] // 8, "config/health threshold mismatch")
    slots = config[6]
    require(1 <= slots <= 3 and set(exits) == set(range(slots))
            and all(multi == (slots > 1) for _, multi in exits.values()), "incomplete process exits")
    require(h["overflow"] in (0, 1) and h["stored"] == len(events) == len(timers)
            and one["DROPPED"] == [h["dropped"]]
            and h["rows_total"] == min(MAX, h["stored"] + h["dropped"]), "inconsistent detail counts")
    require(h["late"] <= h["scored"] and bool(h["late"]) == (h["max_lag"] > h["limit"])
            and (h["scored"] or h["max_lag"] == 0), "inconsistent lag maximum")
    if not h["overflow"]:
        require(h["arms"] == h["rows_total"] and h["scored"] + h["early"] + h["invalid"] <= h["events"] <= h["arms"],
                "inconsistent observation counts")
    first = None
    if one["FIRST"] != [0]:
        require(one["FIRST"][:1] == [1], "malformed FIRST")
        first = fields(FIRST, one["FIRST"][1:])
        irq_boundary(first, True)
        outcome = first["outcome"]
        require(outcome in (2, 3, 4) and first["kind"] in (1, 2) and first["iar"] & 1023 == 30
                and eligible(first, first, h["limit"]) == outcome, "invalid first violation")
        require(first["lag_present"] == int(outcome == 2)
                and first["lag"] == (first["earliest_irq"] - first["due"] if outcome == 2 else 0)
                and first["lag"] <= h["max_lag"] and h[OUTCOMES[outcome]] > 0
                and 1 <= first["event_number"] <= h["events"] and first["decision_index"] < h["rows_total"]
                and 0 < first["arm_seq"] <= h["arms"], "inconsistent FIRST operands")
    if not h["overflow"]:
        require(bool(first) == bool(h["late"] or h["early"] or h["invalid"]), "first violation missing or spurious")
    tallies = Counter()
    ledger = Hierarchy(config, groups)
    register_errors = []
    prefix_max = 0
    prefix_first = None
    worst = None
    for i, (row, timer) in enumerate(zip(events, timers)):
        require(timer["kind"] <= 6 and row[1] in (0, 1, 2) and timer["due"] == row[14]
                and timer["arm_seq"] == i + 1, f"arm mismatch at {i}")
        require((i == 0) == (row[1] == 0), f"startup sequence at {i}")
        if i:
            require(row[8] == events[i - 1][9] and row[13] == timers[i - 1]["due"],
                    f"preceding owner or due time mismatch at {i}")
        decision_boundary(row, timer, events[i-1] if i else None, timers[i-1] if i else None)
        ledger.decision(row, timer)
        require(timer["old_arm_seq"] == i and (i or row[13] == 0), f"previous arm identity at {i}")
        require(all(timer[k] < 8 for k in ("pre_ctl", "post_ctl", "entry_ctl")), f"timer control flags at {i}")
        disabled = timer["kind"] == 6
        if (timer["arm_before"] > timer["arm_after"]
                or (timer["post_ctl"] & 3) != (0 if disabled else 1)
                or timer["post_cval"] != (timer["pre_cval"] if disabled else timer["due"])):
            register_errors.append(i)
        if row[1] != 1:
            require(timer["outcome"] == 0 and not any(timer[k] for k in ENTRY), f"non-IRQ evidence at {i}")
            continue
        tallies["events"] += 1
        require(i > 0 and timer["earliest_irq"] == row[3] and timer["iar"] & 1023 == row[2],
                f"IRQ identity at {i}")
        require(timer["elr"] == row[19] and timer["spsr"] & 15 == (5 if row[8] == 3 else 0),
                f"IRQ frame does not match interrupted owner at {i}")
        irq_boundary(timer, row[8] == 3, events[i-1])
        require(timer["entry_counter"] <= row[4], f"IRQ sampling overlaps settlement at {i}")
        arm = timers[i - 1]
        if (timer["earliest_irq"] > timer["entry_counter"] or
                timer["entry_cval"] != arm["due"] or (timer["entry_ctl"] & 3) != 1):
            register_errors.append(i)
        if timer["iar"] & 1023 != 30:
            outcome = 6
        elif timer["old_arm_seq"] != arm["arm_seq"]:
            outcome = 5
        elif row[8] == 3 and arm["kind"] in (1, 2):
            outcome = eligible(arm, timer, h["limit"])
        else:
            outcome = 0
        require(timer["outcome"] == outcome, f"outcome mismatch at {i}")
        if outcome in (1, 2):
            lag = timer["earliest_irq"] - arm["due"]
            tallies["scored"] += 1
            if worst is None or lag > prefix_max:
                prefix_max, worst = lag, i
        if outcome in (2, 3, 4):
            tallies[OUTCOMES[outcome]] += 1
            if prefix_first is None:
                prefix_first = dict(decision_index=i, event_number=tallies["events"], outcome=outcome,
                                    **{k: arm[k] for k in ARM}, **{k: timer[k] for k in ENTRY},
                                    lag_present=int(outcome == 2), lag=lag if outcome == 2 else 0)
    complete = h["dropped"] == 0
    if complete:
        require(events and events[-1][9] == 4, "complete trace missing terminal decision")
    if first and not h["overflow"]:
        decision = first["decision_index"]
        require(first["due"] >= config[1] and (first["due"] - config[1]) % h["period"] == 0,
                "first idle refill is off the configured grid")
        require(first["event_number"] <= decision and first["arm_seq"] == decision,
                "first violation decision/arm/IRQ identity mismatch")
        if decision == len(timers) and decision > 0:
            irq_boundary(first, True, events[-1])
            require(first["event_number"] == tallies["events"] + 1 and events[-1][9] == 3
                    and all(first[k] == timers[-1][k] for k in ARM),
                    "first omitted IRQ contradicts the retained arm")
        if decision >= len(timers):
            require(first["event_number"] <= tallies["events"] + decision - len(timers) + 1,
                    "first omitted IRQ number exceeds missing decisions")
    for k in ("events", "scored", "late", "early", "invalid"):
        require(tallies[k] <= h[k] and (not complete or tallies[k] == h[k]), f"{k} summary mismatch")
    require(prefix_max <= h["max_lag"] and (not complete or prefix_max == h["max_lag"]), "detail maximum mismatch")
    if prefix_first or complete or (first and first["decision_index"] < h["stored"]):
        require(prefix_first == first, "detail first violation mismatch")
    if not h["overflow"]:
        require(h["events"] - tallies["events"] <= h["dropped"], "event count exceeds missing decisions")
        missing = sum(h[k] - tallies[k] for k in ("scored", "early", "invalid"))
        require(missing <= h["events"] - tallies["events"]
                and h["late"] - tallies["late"] <= h["scored"] - tallies["scored"],
                "missing outcomes exceed missing observations")
        if first and not prefix_first:
            require(first["event_number"] > tallies["events"], "first violation IRQ predates dropped rows")
    statuses = [exits[i][0] for i in range(slots)]
    if expected is not None:
        require(len(expected) == slots, "supply one expected status per slot")
    valid = not (h["overflow"] or h["invalid"] or h["early"] or register_errors)
    return {"instrumentation": "valid" if valid else "invalid_or_incomplete",
            "timing": "late" if h["late"] else "unqualified" if not valid else "within_limit" if h["scored"] else "unobserved",
            "register_discrepancy_rows": sorted(set(register_errors)),
            "health": h, "first_violation": first, "maximum_scored_lag_ticks": h["max_lag"] if h["scored"] else None,
            "maximum_detail_row": worst if prefix_max == h["max_lag"] else None,
            "detail": "complete" if complete else "retained_prefix", "detail_counts": dict(tallies),
            "process_exits": statuses, "expected_exits_match": None if expected is None else statuses == expected}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart", type=Path)
    parser.add_argument("--expect-exit", action="append", type=int, help="repeat in package slot order")
    args = parser.parse_args()
    try:
        result = read_uart(args.uart, args.expect_exit)
    except (ValueError, OSError) as exc:
        parser.exit(2, f"incomplete or contradictory UART: {exc}\n")
    print(json.dumps(result, indent=2))
    sys.exit(0 if result["timing"] == "within_limit" and result["instrumentation"] == "valid"
             and result["expected_exits_match"] is not False else 1)
