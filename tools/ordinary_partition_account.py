"""Reconstruct an ordinary partition UART capture or an explicitly synthetic stream."""
import argparse
import hashlib
import re
import shutil
import subprocess
from dataclasses import dataclass
import json
from collections import Counter, defaultdict
from pathlib import Path

LAYOUT = {"P": ((0, 2), (1, 1), (2, 1)), "Q": ((1, 1), (2, 1), (0, 2), (1, 1))}
NAMES = ("A", "B", "idle")


def require(ok, error):
    if not ok:
        raise ValueError(error)


def slot(table, tick):
    layout = LAYOUT[table]
    period = sum(size for _, size in layout)
    start = tick // period * period
    for owner, size in layout:
        end = start + size
        if tick < end:
            runnable = 2 if owner == 1 and tick < (3 if table == "P" else 1) else owner
            return start, end, owner, runnable
        start = end
    raise AssertionError("unreachable")


def records(lines):
    result = []
    for line in lines:
        if not line.startswith("OP "):
            continue
        _, kind, *fields = line.split()
        data = {}
        for field in fields:
            key, value = field.split("=", 1)
            require(key not in data, "duplicate field")
            data[key] = value if key in ("table", "mode") else int(value, 10)
            if key not in ("table", "mode"):
                require(0 <= data[key] < 2**64, "invalid counter")
        result.append((kind, data))
    return result


MASK = (1 << 64) - 1
MAX_REPLAY = 64_000_000
STEPS = (17, 31, 1)


@dataclass(frozen=True)
class Identity:
    functions: tuple
    stack_base: int
    digest: str | None
    irq_vector: int | None = None
    notice: tuple | None = None


def elf_identity(path, expected_hash, controlled=False):
    data = path.read_bytes()
    require(data[:6] == b"\x7fELF\x02\x01" and data[16:20] == b"\x02\x00\xb7\x00",
            "expected a little-endian AArch64 executable")
    digest = hashlib.sha256(data).hexdigest()
    require(digest == expected_hash.lower(), "ELF hash differs from cold-boot provenance")
    nm = shutil.which("llvm-nm") or shutil.which("aarch64-linux-gnu-nm")
    require(nm, "llvm-nm or aarch64-linux-gnu-nm is required")
    output = subprocess.check_output([nm, "-S", "--defined-only", str(path)], text=True)
    symbols = [line.split() for line in output.splitlines() if len(line.split()) == 4]

    def symbol(pattern, kind):
        matches = [(int(a,16), int(z,16)) for a,z,k,name in symbols
                   if k.lower() == kind and re.fullmatch(pattern, name)]
        require(len(matches) == 1 and matches[0][1] > 0, "missing or ambiguous ELF symbol " + pattern)
        return matches[0]

    functions = tuple(symbol(name, "t") for name in ("ordinary_a", "ordinary_b", "ordinary_idle"))
    base, length = symbol(r"_ZN11jnox_kernel9partition6STACKS17h[0-9a-f]+E", "b")
    require(length == 3 * 16384, "ELF stack array size")
    vector = symbol("sched_vectors", "t")[0] + 0x280 if controlled else None
    notice = None
    if controlled:
        selectors = [m.groups() for _,size,kind,name in symbols
                     if kind.lower() == "t" and int(size, 16) > 0
                     and (m := re.fullmatch(r"ordinary_partition_([pq])_(short|long)", name))]
        require(len(selectors) == 1, "expected exactly one linked controlled notice selector")
        notice = selectors[0]
    return Identity(functions, base, digest, vector, notice)


def exception_events(lines, irq_vector=None):
    if irq_vector is not None:
        # Each real current-EL/SPx IRQ block, including the final IRQ which
        # parks in the reporter, must reach the vector in this exact ELF.
        stream = iter(lines)
        events = []
        for line in stream:
            if line == "Taking exception 5 [IRQ]":
                require(next(stream, None) == "...from EL1 to EL1", "incomplete EL1 IRQ")
                require(re.fullmatch(r"\.\.\.with ESR 0x[\da-fA-F]+/0x[\da-fA-F]+", next(stream, "")),
                        "missing IRQ syndrome record")
                elr = re.fullmatch(r"\.\.\.with ELR (0x[\da-fA-F]+)", next(stream, ""))
                target = re.fullmatch(r"\.\.\.to EL1 PC (0x[\da-fA-F]+) PSTATE (0x[\da-fA-F]+)", next(stream, ""))
                require(elr and target and int(target[1], 16) == irq_vector
                        and int(target[2], 16) & 0x8f == 0x85, "wrong IRQ vector or entry state")
                events.append(("irq", int(elr[1], 16)))
            else:
                target = re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC (0x[\da-fA-F]+)", line)
                require(target, "unexpected controlled exception trace record")
                events.append(("return", int(target[1], 16)))
        return events
    events = []
    pending_irq = False
    for line in lines:
        if line.startswith("Taking exception"):
            require(line == "Taking exception 5 [IRQ]" and not pending_irq, "unexpected exception")
            pending_irq = True
        elif line.startswith("...with ELR "):
            require(pending_irq, "ELR without IRQ")
            events.append(("irq", int(line.split()[-1], 16)))
            pending_irq = False
        elif line.startswith("Exception return"):
            match = re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC (0x[0-9a-fA-F]+)", line)
            require(match and not pending_irq, "invalid exception return")
            events.append(("return", int(match[1], 16)))
    require(not pending_irq, "truncated IRQ")
    return events


def reconstruct(lines, table, case, origin, identity=None, exceptions=None):
    require(origin in ("guest", "synthetic"), "invalid evidence origin")
    if origin == "guest":
        require(identity is not None and identity.digest and exceptions is not None,
                "guest evidence requires a provenance-matched ELF and exception trace")
    else:
        require(identity is None and exceptions is None, "synthetic inputs cannot claim ELF identity")
        identity = Identity(tuple((0x4000 + i*256, 128) for i in range(3)), 0x10000, None)
    controlled = origin == "guest" and case != "healthy"
    if controlled:
        require(identity.irq_vector is not None, "controlled guest needs the linked IRQ vector")
        lines = list(lines)
        require(lines[:2] == ["JNOX M1 BOOT", "JNOX M1 READY"] and
                all(line.startswith("OP ") for line in lines[2:]), "incomplete controlled boot record")
    expected_events = []
    accumulator = list(STEPS)
    replayed = 0
    interrupted_counts = [0, 0, 0]
    stream = iter(records(lines))

    def take(kind):
        item = next(stream, None)
        require(item is not None and item[0] == kind, f"expected {kind}")
        return item[1]

    if controlled:
        mode = "long" if case == "long" else "short"
        require(identity.notice == (table.lower(), mode), "ELF does not contain the selected controlled case")
        require(take("NOTICE") == dict(table=table, mode=mode, frame=0, slot=0 if table == "P" else 2),
                "wrong cold-boot notice selector")
    h = take("HEADER")
    epoch, freq, q, count = (h[k] for k in ("epoch", "freq", "q", "n"))
    require(h["table"] == table and freq > 0 and freq % 1000 == 0
            and q == freq // 20 and 1 <= count <= 16, "invalid header")
    release = take("RELEASE") if controlled else None
    stacks = []
    for owner in range(3):
        r = take("STACK")
        require(r["id"] == owner and r["hi"] - r["lo"] == 16384, "stack shape")
        require(all(r["hi"] <= lo or hi <= r["lo"] for lo, hi in stacks), "overlapping stacks")
        require(r["lo"] == identity.stack_base + owner*16384, "stack differs from image")
        stacks.append((r["lo"], r["hi"]))

    def at(time):
        require(time >= epoch, "timestamp before epoch")
        return slot(table, (time - epoch) // q)

    def check_sp(owner, sp):
        lo, hi = stacks[owner]
        require(lo < sp <= hi and sp % 16 == 0, "incorrect private stack")

    def absolute(tick):
        result = epoch + q * tick
        require(result <= MASK, "absolute boundary overflow")
        return result

    intervals = []
    totals = Counter()
    losses = Counter()
    missed = []
    saved = {}
    counts = defaultdict(int)
    edges = []
    delay_due = absolute(2 if table == "P" else 4)
    period = 4 if table == "P" else 5
    expected_due = epoch
    previous = None
    recovery = None
    release_due = absolute(3 if table == "P" else 1)
    release_seen = False
    affected = []

    def check_irq(irq, n, out):
        require(irq["n"] == n and irq["iar"] & 1023 == 30 and irq["ctl"] & 7 == 5
                and irq["spsr"] & 0x8f == 5, "expected asserted physical timer and IRQ-enabled EL1h worker")
        require(irq["c"] == previous[2]["c"] and irq["end"] == at(out["entry"])[1],
                "active compare or IRQ-entry window mismatch")

    def charge(start, end, actual):
        require(start <= end, "negative interval")
        while start < end:
            s, t, owner, runnable = at(start)
            stop = min(end, absolute(t))
            ticks = stop - start
            totals[actual] += ticks
            if actual in NAMES and actual != NAMES[runnable]:
                losses[NAMES[runnable]] += ticks
            intervals.append(dict(start=start, end=stop, nominal_start=absolute(s),
                                  nominal_owner=NAMES[owner], eligible=NAMES[runnable], actual=actual))
            start = stop

    def close_previous(out):
        prior, obs, timer = previous
        task = prior["chosen"]
        check_sp(task, out["out_sp"])
        base, size = identity.functions[task]
        require(base <= out["out_pc"] < base + size and out["out_pc"] % 4 == 0,
                "interrupted PC outside task function")
        interrupted_counts[task] = out["out_count"]
        require(out["out_count"] <= MAX_REPLAY, "outgoing counter exceeds replay limit")
        expected_events.append(("irq", out["out_pc"]))
        require(out["from"] == task and out["out_pc"] != 0 and
                obs["x"] <= out["last"] <= out["entry"] and out["out_count"] > obs["count"],
                "invalid outgoing task progress")
        require(out["entry"] >= timer["c"] and out["due"] == timer["d"], "timer delivery contradiction")
        saved[task] = (out["out_pc"], out["out_sp"], out["out_hash"])
        charge(obs["x"], out["last"], NAMES[task])
        charge(out["last"], out["entry"], "uncertainty")

    # BOUND records precede each noninitial EDGE. Derive them independently from
    # the verified due and current live ticks and compare every reported field.
    pending = list(stream)
    index = 0

    def read(kind):
        nonlocal index
        require(index < len(pending) and pending[index][0] == kind, f"expected {kind}")
        index += 1
        return pending[index - 1][1]

    for n in range(count):
        bounds = []
        while index < len(pending) and pending[index][0] == "BOUND":
            bounds.append(read("BOUND"))
        irq = read("IRQ") if n and controlled else None
        timer, edge, obs = read("TIMER"), read("EDGE"), read("OBS")
        require(all(r["n"] == n for r in (timer, edge, obs)), "generation mismatch")
        live = edge["live"]
        require(epoch <= live < absolute(4 * period), "outside capture horizon")
        s, t, owner, task = at(live)
        require(edge["due"] == expected_due and edge["owner"] == owner and edge["chosen"] == task
                and edge["next"] == absolute(t) and timer["d"] == absolute(t), "wrong dispatch or epoch")
        require(obs["task"] == task and obs["count"] > counts[task], "wrong in-context observation")
        delta = obs["count"] - counts[task]
        replayed += delta
        require(replayed <= MAX_REPLAY, "worker recurrence exceeds bounded replay")
        value = accumulator[task]
        for _ in range(delta):
            value = (((value << 7) | (value >> 57)) + STEPS[task]) & MASK
        require(obs["value"] == value, "worker accumulator mismatch")
        counts[task] = obs["count"]
        accumulator[task] = value
        check_sp(task, obs["sp"])
        check_sp(task, edge["in_sp"])
        require(edge["in_pc"] != 0, "missing return PC")
        require(live <= timer["p"] <= timer["after"] <= edge["ret"] <= obs["x"] < edge["next"],
                "invalid programming or return bracket")
        require(timer["after"] < timer["d"], "late timer write")
        offset = 0
        if timer["d"] == delay_due and case != "healthy":
            offset = q + q // 4 if case == "long" else freq // 1000 + 1
            require(case != "long" or q % 4 == 0, "fractional long offset")
        require(timer["c"] == timer["d"] + offset, "compare readback mismatch")
        expected_bounds = []
        if previous:
            close_previous(edge)
            if controlled:
                check_irq(irq, n, edge)
                if not release_seen and edge["entry"] >= release_due:
                    require(release["n"] == n and release["due"] == release_due and
                            edge["entry"] <= release["before"] <= release["after"] <= live,
                            "deferred B release not bracketed at first eligible IRQ")
                    release_seen = True
            require(edge["entry"] <= live, "entry after selection")
            require(at(edge["entry"]) == at(live), "unexpected boundary in dispatcher")
            tick = (expected_due - epoch) // q
            while tick <= (live - epoch) // q:
                _, end, nominal, ready = slot(table, tick)
                skipped = absolute(end) <= live
                expected_bounds.append(dict(n=n,tick=tick,end=end,owner=nominal,runnable=ready,skipped=int(skipped)))
                if skipped:
                    missed.append(dict(start=absolute(tick), end=absolute(end), owner=NAMES[nominal], eligible=NAMES[ready]))
                tick = end
            programmed = previous[2]["c"]
            metrics = dict(d=expected_due, c=programmed, i=edge["entry"], x=obs["x"],
                           c_minus_d=programmed-expected_due, i_minus_c=edge["entry"]-programmed,
                           x_minus_i=obs["x"]-edge["entry"])
            if expected_due == delay_due:
                recovery = metrics
                if controlled:
                    prior, a_obs, a_timer = previous
                    require(prior["chosen"] == edge["from"] == 0 and
                            a_timer["p"] <= a_timer["after"] < prior["ret"] <= a_obs["x"] < delay_due
                            < programmed <= edge["entry"] <= obs["x"], "unobserved first A close")
                    require(at(programmed) == at(edge["entry"]) == at(obs["x"]),
                            "additional boundary crossed during controlled recovery")
                    # Only x..last is attributed to the outgoing task. The gap from
                    # last to entry is uncertainty, even with an asserted timer.
                    require(absolute(at(programmed)[0]) < irq["prior_a"] < edge["last"],
                            "missing two distinct A iterations past the recovery-window boundary")
                    for span in intervals:
                        if span["end"] > delay_due and span["actual"] == "A":
                            affected.append(dict(start=span["start"], end=span["end"],
                                                 ticks=span["end"] - span["start"], execution_by="A",
                                                 nominal_start=span["nominal_start"],
                                                 nominal_end=absolute(at(span["start"])[1]),
                                                 nominal_owner=span["nominal_owner"], eligible=span["eligible"]))
                    expected = (["B", "idle"] if table == "P" else ["B", "B"]) if case == "long" else ["B"]
                    lost_to = "idle" if table == "P" else "B"
                    require([r["nominal_owner"] for r in affected] == expected and
                            all(r["eligible"] == lost_to for r in affected) and
                            affected[0]["start"] == delay_due and affected[-1]["end"] == edge["last"],
                            "missing measured lost service by nominal window")
                    recovery.update(p=a_timer["p"], after=a_timer["after"], a_prior=irq["prior_a"],
                                    a_last=edge["last"], x_minus_d=obs["x"] - delay_due,
                                    a_interrupted_pc=edge["out_pc"], recovery_owner=NAMES[owner],
                                    recovery_task=NAMES[task], next_original_boundary=edge["next"],
                                    measured_a_spill_by_window=affected)
            charge(edge["entry"], edge["ret"], "kernel")
        else:
            require(edge["entry"] == 0 and edge["from"] == 3 and s == 0, "incorrect first dispatch")
            require(all(edge[k] == 0 for k in ("last","out_count","out_pc","out_sp","out_hash")),
                    "nonzero boot sentinel")
            charge(epoch, edge["ret"], "kernel")
        require(bounds == expected_bounds, "crossed boundary mismatch")
        require(obs["count"] >= interrupted_counts[task], "worker progress rolled back")
        if task not in saved:
            # Initial frame has 31 zero GPRs and SPSR=0x345. Its hash rotates
            # SPSR by nine bits per GPR (279 modulo 64 equals 23).
            require(edge["in_hash"] == 0x345 << 23, "incorrect initial register fingerprint")
            require(edge["in_pc"] == identity.functions[task][0] and edge["in_sp"] == stacks[task][1],
                    "initial task PC or SP differs from image")
        expected_events.append(("return", edge["in_pc"]))
        if task in saved:
            require(tuple(edge[k] for k in ("in_pc","in_sp","in_hash")) == saved[task], "false resume")
        charge(edge["ret"], obs["x"], "uncertainty")
        late = obs["x"] - expected_due
        edges.append(dict(n=n, nominal_close=expected_due, entry=edge["entry"], selected_at=live,
                          return_at=edge["ret"], x=obs["x"], latency=late, within_bound=late <= freq//1000,
                          nominal_owner=NAMES[owner], actual=NAMES[task]))
        if case != "healthy":
            if expected_due == delay_due:
                require(late > freq//1000, "expected envelope failure absent")
            elif not controlled:
                require(late <= freq//1000, "unplanned late dispatch")
        previous = edge, obs, timer
        expected_due = edge["next"]

    terminal_irq = read("IRQ") if controlled else None
    stop = read("STOP")
    require(index == len(pending), "trailing records")
    require(at(previous[0]["live"])[0] == 3*period and stop["due"] == expected_due, "incomplete horizon")
    close_previous(stop)
    if controlled:
        check_irq(terminal_irq, count, stop)
        require(at(stop["due"]) == at(stop["entry"]), "unexpected terminal boundary crossing")
        require(release_seen, "B release was not observed")
    require(recovery is not None, "missing first A close")
    require(sum(totals.values()) == stop["entry"] - epoch, "incomplete elapsed accounting")
    expected_missed = [delay_due] if case == "long" else []
    if case != "healthy":
        require([r["start"] for r in missed] == expected_missed, "unexpected skipped windows")
    if exceptions is not None:
        require(exceptions == expected_events, "independent IRQ/return sequence differs")
    first_b = next((r for r in edges if r["actual"] == "B"), None)
    require(first_b is not None, "B never observed")
    timing = all(r["within_bound"] for r in edges) and not missed
    result = dict(origin=origin, table=table, case=case, epoch=epoch, frequency=freq,
                edges=edges, first_a_close=recovery, missed=missed, intervals=intervals,
                b_release_nominal=absolute(3 if table == "P" else 1),
                b_first_selection=first_b["selected_at"], b_first_worker_observation=first_b["x"],
                ticks=dict(totals), measured_service_loss_ticks=dict(losses),
                timing_envelope_met=timing, recurrence_iterations_checked=replayed,
                elf_sha256=identity.digest, independent_return_trace_checked=exceptions is not None)
    if controlled:
        # Require two subsequent complete nominal windows on their original
        # boundaries, in addition to positive work in the recovered remainder.
        def observed_ticks(edge, end, task):
            return sum(min(span["end"], end) - max(span["start"], edge["x"]) for span in intervals
                       if span["actual"] == task and span["end"] > edge["x"] and span["start"] < end)

        recovered = next(edge for edge in edges if edge["nominal_close"] == delay_due)
        start = at(recovery["x"])[1]
        recovery["work_ticks_in_remainder"] = observed_ticks(recovered, absolute(start), recovered["actual"])
        require(recovery["work_ticks_in_remainder"] > 0, "missing work in recovered remainder")
        protected = []
        for _ in range(2):
            _, end, _, runnable = slot(table, start)
            a, b = absolute(start), absolute(end)
            events = [edge for edge in edges if edge["nominal_close"] == a]
            require(len(events) == 1 and events[0]["actual"] == NAMES[runnable], "missing protected window")
            ticks = observed_ticks(events[0], b, NAMES[runnable])
            require(ticks > 0, "unobserved service in protected window")
            protected.append(dict(start=a, end=b, task=NAMES[runnable], measured_ticks=ticks))
            start = end
        unrelated = [edge["n"] for edge in edges if edge["nominal_close"] != delay_due and not edge["within_bound"]]
        require(not timing and release["after"] <= first_b["selected_at"], "controlled timing or release contradiction")
        result.update(b_release_processing=release, protected_windows=protected,
                      unrelated_late_generations=unrelated, controlled_case_met=not unrelated)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart", type=Path)
    parser.add_argument("--table", choices=LAYOUT, required=True)
    parser.add_argument("--case", choices=("healthy","near","short","long"), required=True)
    parser.add_argument("--origin", choices=("guest","synthetic"), required=True)
    parser.add_argument("--elf", type=Path)
    parser.add_argument("--elf-sha256", help="expected digest from the cold-boot record")
    parser.add_argument("--exceptions", type=Path, help="raw QEMU -d int trace")
    args = parser.parse_args()
    try:
        identity = None
        exceptions = None
        if args.origin == "guest":
            require(args.elf and args.elf_sha256 and args.exceptions, "guest requires --elf, --elf-sha256 and --exceptions")
            controlled = args.case != "healthy"
            identity = elf_identity(args.elf, args.elf_sha256, controlled)
            trace = args.exceptions.read_bytes()
            if controlled:
                require(trace.endswith(b"\n"), "partial final exception record")
            exceptions = exception_events(trace.decode("ascii").splitlines(), identity.irq_vector)
        else:
            require(not any((args.elf, args.elf_sha256, args.exceptions)), "unexpected synthetic evidence arguments")
        uart = args.uart.read_bytes()
        if args.origin == "guest" and args.case != "healthy":
            require(uart.endswith(b"\n"), "partial final UART record")
        result = reconstruct(uart.decode("ascii").splitlines(), args.table, args.case, args.origin,
                             identity, exceptions)
    except (KeyError, ValueError, OSError, subprocess.CalledProcessError) as exc:
        parser.exit(1, f"invalid trace: {exc}\n")
    print(json.dumps(result, indent=2))
    if (not result["timing_envelope_met"] and args.case == "healthy") or result.get("controlled_case_met") is False:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
