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
            data[key] = value if key == "table" else int(value, 10)
            if key != "table":
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


def elf_identity(path, expected_hash):
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
    return Identity(functions, base, digest)


def exception_events(lines):
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
    expected_events = []
    accumulator = list(STEPS)
    replayed = 0
    interrupted_counts = [0, 0, 0]
    stream = iter(records(lines))

    def take(kind):
        item = next(stream, None)
        require(item is not None and item[0] == kind, f"expected {kind}")
        return item[1]

    h = take("HEADER")
    epoch, freq, q, count = (h[k] for k in ("epoch", "freq", "q", "n"))
    require(h["table"] == table and freq > 0 and freq % 1000 == 0
            and q == freq // 20 and 1 <= count <= 16, "invalid header")
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
        return epoch + q * tick

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
            else:
                require(late <= freq//1000, "unplanned late dispatch")
        previous = edge, obs, timer
        expected_due = edge["next"]

    stop = read("STOP")
    require(index == len(pending), "trailing records")
    require(at(previous[0]["live"])[0] == 3*period and stop["due"] == expected_due, "incomplete horizon")
    close_previous(stop)
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
    return dict(origin=origin, table=table, case=case, epoch=epoch, frequency=freq,
                edges=edges, first_a_close=recovery, missed=missed, intervals=intervals,
                b_release_nominal=absolute(3 if table == "P" else 1),
                b_first_selection=first_b["selected_at"], b_first_worker_observation=first_b["x"],
                ticks=dict(totals), measured_service_loss_ticks=dict(losses),
                timing_envelope_met=timing, recurrence_iterations_checked=replayed,
                elf_sha256=identity.digest, independent_return_trace_checked=exceptions is not None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart", type=Path)
    parser.add_argument("--table", choices=LAYOUT, required=True)
    parser.add_argument("--case", choices=("healthy","near","long"), required=True)
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
            identity = elf_identity(args.elf, args.elf_sha256)
            exceptions = exception_events(args.exceptions.read_text().splitlines())
        else:
            require(not any((args.elf, args.elf_sha256, args.exceptions)), "unexpected synthetic evidence arguments")
        result = reconstruct(args.uart.read_text().splitlines(), args.table, args.case, args.origin,
                             identity, exceptions)
    except (KeyError, ValueError, OSError, subprocess.CalledProcessError) as exc:
        parser.exit(1, f"invalid trace: {exc}\n")
    print(json.dumps(result, indent=2))
    if not result["timing_envelope_met"] and args.case == "healthy":
        raise SystemExit(2)


if __name__ == "__main__":
    main()
