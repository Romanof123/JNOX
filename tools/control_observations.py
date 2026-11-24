"""Score an ordinary control UART capture. Per-run observations only."""
import argparse
import json
from pathlib import Path


def intervals(fault, detect, safe, hz):
    return {
        name: {
            "ticks": ticks,
            "limit_ms": limit,
            "within": 0 <= ticks * 1000 <= limit * hz,
        }
        for name, ticks, limit in (
            ("detection", detect - fault, 25),
            ("inhibition", safe - detect, 5),
            ("total", safe - fault, 30),
        )
    }


def ordinary(rows, case):
    def events(code):
        return [(t, a, b) for tag, t, a, b in rows if tag == code]

    selected_case = case
    handoff = case.startswith("handoff-")
    if handoff:
        case = case.removeprefix("handoff-")
    failures = []

    def require(condition, name):
        if not condition:
            failures.append(name)

    [(epoch, hz, last)] = events(1)
    require(last == 25 and hz > 0 and hz % 400 == 0, "clock/horizon")
    step = hz // 400
    targets = [epoch + i * step for i in range(26)]
    irqs = events(3)
    checks = events(10)
    decisions = events(21)
    require(
        [(i, t) for _, i, t in events(2)] == list(enumerate(targets)),
        "programmed targets",
    )
    require([a for _, a, _ in irqs] == list(range(26)), "timer coverage")
    require(all(i == j for _, i, j in irqs), "skipped phases")
    late = [(t - targets[i]) * 2000 > hz for t, i, _ in irqs]
    require(all(t >= targets[i] for t, i, _ in irqs), "early IRQ")
    require([i for _, i, _ in checks] == list(range(26)), "worker coverage")
    require([i for _, i, _ in decisions] == list(range(26)), "decision coverage")
    require(
        all(
            target == targets[i] and 0 <= (t - target) * 1000 <= hz
            for t, i, target in checks
        ),
        "phase envelope",
    )
    require(
        all(
            targets[i] <= decision <= t <= checks[i][0] for t, i, decision in decisions
        ),
        "decision chronology",
    )
    require(
        all(
            0 <= (b[0] - a[0]) * 1000 <= 5 * hz
            for a, b in zip(decisions, decisions[1:])
        ),
        "decision spacing",
    )
    for id, stride in ((0, 2), (1, 4)):
        expected = targets[::stride]
        require(
            [r for _, task, r in events(4) if task == id] == expected,
            f"worker {id} releases",
        )
        ends = [(t, r) for t, task, r in events(9) if task == id]
        require([r for _, r in ends] == expected, f"worker {id} finishes")
        require(
            all(r <= t <= r + stride * step for t, r in ends), f"worker {id} deadline"
        )
    require([t for _, _, t in events(7)] == targets[1::2], "continuation releases")
    service = events(8)
    bad_demand = [
        (t, task, d)
        for t, task, d in service
        if d <= 0 or d * 1_000_000 > hz * (250 if task == 0 else 750)
    ]
    require(
        sum(task == 0 for _, task, _ in service) == 26
        and sum(task == 1 for _, task, _ in service) == 7,
        "service coverage",
    )
    slices = [0, 0]
    for tag, _, task, delta in rows:
        if tag == 6:
            slices[task] += delta
        if tag == 8:
            require(slices[task] == delta, "worker slice accounting")
            slices[task] = 0
    issues, delivered, commands = events(11), events(12), events(17)
    if handoff:
        injector = events(41)
        injector_deliveries = events(42)
        transfers = events(39)
        attempts = events(40)
        if events(38) or not transfers:
            return {
                "case": selected_case,
                "scope": "exact injector setup",
                "failures": failures + ["initial condition unqualified"],
                "attempts": attempts,
                "injector_issues": injector,
                "injector_deliveries": injector_deliveries,
                "disqualifications": events(38),
                "transfers": transfers,
                "ordinary_issues": issues,
                "final_device": events(18),
            }
        expected = [epoch + step // 5 + n * step for n in (0, 4, 6)]
        require(
            attempts == [(t, i, t) for i, t in enumerate(expected, 1)],
            "actual injector targets",
        )
        require(
            injector == [(t, i, 1) for i, t in enumerate(expected[:2], 1)],
            "exact injector issues",
        )
        require(
            injector_deliveries == [(t, i, t) for i, t in enumerate(expected[:2], 1)],
            "exact injector deliveries",
        )
        require(
            len(transfers) == 1 and transfers[0][1:] == (1, 3), "single owner handoff"
        )
        require(len(events(37)) == 2, "sender withheld before handoff")
        require(
            len(events(14)) >= 1
            and events(14)[0][1:] == (0, 1)
            and issues
            and events(14)[0][0] <= transfers[0][0] <= issues[0][0],
            "readback before ordinary owner",
        )
        if transfers and events(14) and issues:
            require(
                rows.index([14, *events(14)[0]])
                < rows.index([39, *transfers[0]])
                < rows.index([11, *issues[0]]),
                "ordered device, handoff and first ordinary issue",
            )
        require(
            all(
                generation == 1 and seq == n
                for n, (_, seq, generation) in enumerate(issues, 3)
            ),
            "ordinary generation and next sequence",
        )
        require(
            len(issues) == (2 if case == "loss" else 5),
            "post-handoff ordinary issue count",
        )
        ordinary_issues = issues
        issues = injector + issues
        delivered = injector_deliveries + delivered
    require(len(issues) == (4 if case == "loss" else 7), "total issue count")
    require(
        len(issues) == len(delivered)
        and all(
            n == i == j and echoed == issued and delivered_at >= issued
            for n, ((issued, i, _), (delivered_at, j, echoed)) in enumerate(
                zip(issues, delivered), 1
            )
        ),
        "actual delivery",
    )
    require(
        len(issues) >= 2 and (issues[1][0] - issues[0][0]) * 1000 == 10 * hz,
        "rearm issue spacing",
    )
    require(
        len(commands) == 1 and (commands[0][0] - issues[0][0]) * 1000 == 15 * hz,
        "rearm command time",
    )
    require(len(events(20)) == 1 and events(20)[0][0] >= commands[0][0], "actual rearm")
    edges = events(14)
    require(len(events(19)) == 1 and events(19)[0][1] == 0, "boot off")
    require(
        [(before, after) for _, before, after in edges]
        == ([(0, 1)] if case == "healthy" else [(0, 1), (1, 0)]),
        "device transitions",
    )
    require(
        len(events(18)) == 1 and events(18)[0][1] == int(case == "healthy"),
        "final device",
    )
    groups = []
    for index in range(0, 24, 4):
        left = targets[index]
        right = left + 4 * step
        demand = sum(ticks for t, task, ticks in service if left <= t < right)
        groups.append({"first_phase": index, "service_ticks": demand})
        if case != "demand":
            require(
                demand * 1_000_000 <= hz * (4 * 250 + 750), "10ms recurring service"
            )
        require(sum(left <= t < right for t, _, _ in irqs) == 4, "10ms IRQ count")
    require(len(events(34)) == len(irqs), "IRQ handler observation coverage")
    require(len(events(33)) == 1, "masked interval observation")
    faults = events(15)
    require(
        any(late) == (case == "late") and bool(bad_demand) == (case == "demand"),
        "negative control isolation",
    )
    require(
        {why for _, why, _ in faults}
        == ({"late": {1}, "demand": {3}}.get(case, set())),
        "timing failure causes",
    )
    budget = {}
    if case == "loss":
        [(fault, seq, issue)] = events(16)
        [(detect, cause, last_issue)] = events(13)
        safe = edges[-1][0]
        require(
            cause == 0
            and issue == last_issue == issues[-1][0]
            and seq == delivered[-1][1]
            and delivered[-1][0] <= fault,
            "expiry cause and gate",
        )
        require((detect - last_issue) * 1000 >= 20 * hz, "premature expiry")
        require(
            rows.index([16, fault, seq, issue])
            < rows.index([13, detect, cause, last_issue])
            < rows.index([14, safe, 1, 0]),
            "recorded fault order",
        )
        [(fault_lo, fault_hi, _)] = events(29)
        [(detect_lo, detect_hi, _)] = events(28)
        [(safe_lo, safe_hi, _)] = [e for e in events(27) if e[2] == 0]
        require(
            fault_lo
            <= fault_hi
            == fault
            <= detect_lo
            <= detect_hi
            == detect
            <= safe_lo
            <= safe_hi
            == safe,
            "counter brackets",
        )
        budget = intervals(fault, detect, safe, hz)
        for name, worst in (
            ("detection", detect_hi - fault_lo),
            ("inhibition", safe_hi - detect_lo),
            ("total", safe_hi - fault_lo),
        ):
            budget[name]["upper_ticks"] = worst
            budget[name]["within"] &= worst * 1000 <= budget[name]["limit_ms"] * hz
        require(all(v["within"] for v in budget.values()), "fault response budgets")
    return {
        "case": selected_case,
        "issue_ownership": {
            "injector": injector if handoff else [],
            "ordinary": ordinary_issues if handoff else issues,
        },
        "failures": failures,
        "fault_budgets": budget,
        "recurring_windows": groups,
        "late_irqs": sum(late),
        "over_budget_activations": bad_demand,
    }


def boundary_controls(hz):
    """Integer-tick verdict fixtures. These rows never stand in for guest time."""
    if hz <= 0 or hz % 1000:
        raise ValueError("fixture frequency must support whole milliseconds")
    ms = hz // 1000
    result = {}
    for label, detect, safe_at_limit in (
        ("detection", 25 * ms, 25 * ms),
        ("inhibition", 0, 5 * ms),
        ("total", 25 * ms, 30 * ms),
    ):
        at = intervals(0, detect, safe_at_limit, hz)
        over = intervals(0, detect + (label == "detection"), safe_at_limit + 1, hz)
        result[label] = {"at_limit": at[label], "one_tick_over": over[label]}
    return result


def script_observations(rows, case):
    def events(tag):
        return [(t, a, b) for k, t, a, b in rows if k == tag]

    errors = []

    def require(value, name):
        if not value:
            errors.append(name)

    [(origin, hz, _)] = events(25)
    step = hz // 400
    edges = events(14)
    final = events(18)
    require(len(final) == 1, "final observation")
    require(len(events(19)) == 1 and events(19)[0][1] == 0, "boot off")
    expected_on = case == "recovery"
    require(bool(final[0][1]) == expected_on, "final state")
    commands = events(17)
    issued = events(11)
    accepted = events(12)
    if case in ("worst", "tie"):
        require(len(issued) == (3 if case == "worst" else 4), "issuance count")
        require(
            (issued[1][0] - issued[0][0]) == 4 * step
            and commands[0][0] - issued[0][0] == 6 * step,
            "initial pair and command",
        )
        require(
            [(a, b) for _, a, b in edges] == [(0, 1), (1, 0)], "actual device lifecycle"
        )
        anchor = issued[2][0]
        require(
            accepted[2][0] == anchor,
            "boundary issue and delivery on same measured tick",
        )
        checks = events(21)
        if case == "worst":
            require(
                checks[-2][0] == anchor + 8 * step - 1
                and checks[-1][0] == checks[-2][0] + 2 * step,
                "exact consecutive completed decisions",
            )
            require(
                len(events(16)) == 1 and len(events(13)) == 1, "actual gate and expiry"
            )
            if events(16) and events(13) and edges:
                fault = events(29)[0]
                latch = events(28)[0]
                edge = events(27)[-1]
                require(
                    fault[1] <= latch[0] <= latch[1] <= edge[0]
                    and latch[1] - fault[0] <= 10 * step
                    and edge[1] - latch[0] <= 2 * step
                    and edge[1] - fault[0] <= 12 * step,
                    "ordered bracketed 25/5/30ms",
                )
        else:
            require(not events(16) and len(accepted) == 4, "open gate and arrival")
            target = anchor + 8 * step
            require(
                checks[-1][0]
                == checks[-1][2]
                == issued[3][0]
                == accepted[-1][0]
                == target,
                "exact expiry/arrival tick",
            )
            require(
                len(events(13)) == 1 and events(13)[0][1] == 0, "expiry-first latch"
            )
            require(
                events(30)[-1][:2] == (target, target), "unambiguous arrival bracket"
            )
    elif case in (
        "recovery",
        "old-generation",
        "early-command",
        "expired-command",
        "missing-permit",
        "no-command",
    ):
        require(
            [(a, b) for _, a, b in edges]
            == ([(0, 1), (1, 0), (0, 1)] if expected_on else [(0, 1), (1, 0)]),
            "recovery prerequisites",
        )
        require(
            len(events(31)) == 1
            and events(31)[0][1] == (1 if case == "old-generation" else 2),
            "generation",
        )
        restart = events(31)[0][0]
        pair = [row for row in issued if row[0] > restart]
        require(
            len(pair) == (1 if case == "missing-permit" else 2), "recovery issue count"
        )
        if len(pair) == 2:
            require(pair[1][0] - pair[0][0] == 4 * step, "10ms actual recovery spacing")
        new_commands = [t for t, _, _ in commands if t > restart]
        offset = {"early-command": 2, "expired-command": 8, "no-command": None}.get(
            case, 6
        )
        require(
            new_commands == ([] if offset is None else [pair[0][0] + offset * step]),
            "actual command tick",
        )
    elif case == "silence":
        require(
            len(issued) == 2 and len(events(13)) == 1 and not events(16),
            "complete service silence",
        )
    elif case == "invalid":
        require(len(events(24)) == 6 and len(accepted) == 2, "invalid input rejection")
    elif case == "saturation":
        require(
            len(events(26)) == 1 and len(accepted) == 2 and len(events(16)) == 1,
            "bounded stimulus rejection",
        )
    if case in ("silence", "invalid", "saturation"):
        require(
            [(a, b) for _, a, b in edges] == [(0, 1), (1, 0)],
            "healthy state to observed expiry",
        )
        require(
            len(events(36)) == 1 and events(36)[0][1:] == (2, issued[1][0]),
            "unchanged last receiver state",
        )
    return {"case": case, "scope": "scripted guest fixture", "failures": errors}


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument(
        "case",
        choices=[
            "healthy",
            "loss",
            "handoff-loss",
            "handoff-healthy",
            "late",
            "demand",
            "worst",
            "tie",
            "early-command",
            "expired-command",
            "recovery",
            "old-generation",
            "missing-permit",
            "no-command",
            "invalid",
            "silence",
            "saturation",
            "verdict-boundaries",
        ],
    )
    p.add_argument("uart", type=Path, nargs="?")
    args = p.parse_args()
    if args.case == "verdict-boundaries":
        controls = boundary_controls(62_500_000)
        print(json.dumps(controls, indent=2))
        raise SystemExit(
            any(
                not v["at_limit"]["within"] or v["one_tick_over"]["within"]
                for v in controls.values()
            )
        )
    if args.uart is None:
        p.error("a UART file is required")
    data = args.uart.read_text().splitlines()
    start = data.index("CONTROL RAW v1") + 1
    end = data.index("CONTROL END", start)
    rows = [list(map(int, line.split())) for line in data[start:end]]
    score = (
        ordinary
        if args.case
        in ("healthy", "loss", "late", "demand", "handoff-loss", "handoff-healthy")
        else script_observations
    )
    result = score(rows, args.case)
    [(_, _, dm, case_hash)] = [row for row in rows if row[0] == 32]
    wanted = 0
    for byte in args.case.encode():
        wanted = (31 * wanted + byte) % (1 << 64)
    if case_hash != wanted or dm not in (0, 1):
        result["failures"].append("embedded profile/case")
    result["policy"] = "DM" if dm else "RM"
    print(json.dumps(result, indent=2))
    raise SystemExit(bool(result["failures"]))
