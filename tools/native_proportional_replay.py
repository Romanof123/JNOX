#!/usr/bin/env python3
"""Replay a packaged native lottery/stride capture. See native_proportional_capture.md."""
import argparse
from collections import defaultdict, namedtuple
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
import runpy
import struct
import sys

from native_el0_replay import Snapshot, PA_MASK, argument_stacks, elf_file, symbol, vread

MAX = (1 << 64) - 1
SCALE = 720720
IDLE, PARK = 3, 4
Event = namedtuple("Event", """seq kind entry decision op arg result src dst selection
    start stop dt prior cval ctl before after woke exited sleep src_pc src_sp src_pstate
    src_root src_frame dst_pc dst_sp dst_pstate dst_root dst_frame new_r new_cval new_ctl
    returned eret idle_x eoi full ordinal""")


class Incomplete(Exception):
    pass


def check(ok, message):
    if not ok:
        raise ValueError(message)


def present(ok, message):
    if not ok:
        raise Incomplete(message)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def draw(state, weights, ready):
    total = sum(w for i, w in enumerate(weights) if ready & (1 << i))
    check(total > 0, "draw on empty ready set")
    before = state
    words = 0
    while True:
        state = (state + 0x9e3779b97f4a7c15) & MAX
        x = state
        x = ((x ^ (x >> 30)) * 0xbf58476d1ce4e5b9) & MAX
        x = ((x ^ (x >> 27)) * 0x94d049bb133111eb) & MAX
        raw = x ^ (x >> 31)
        words += 1
        if raw >= (1 << 64) % total:
            break
    ticket = raw % total
    upper = 0
    for i, w in enumerate(weights):
        if ready & (1 << i):
            upper += w
            if ticket < upper:
                return i, (before, state, words, raw, ticket, total)
    raise ValueError("ticket outside eligible slots")


def change_mask(passes, old, new):
    floor = min((x for i, x in enumerate(passes) if old >> i & 1), default=0)
    if not new:
        return [max(0, x - floor) for x in passes]
    return [max(x, floor) if (new & ~old) >> i & 1 else x for i, x in enumerate(passes)]


def parse(uart):
    widths = dict(CONFIG=8, SLOT=4, EVENT=len(Event._fields), DRAW=7, PASS=6,
                  BASE=2, DROPPED=1, CONDITION=1)
    rows = defaultdict(list)
    for line in uart.splitlines():
        if line.startswith(b"NATIVE PROP "):
            label, *parts = line[12:].decode("ascii").split()
            if label in ("FAIL", "FAULT", "REFUSED"):
                raise ValueError(line.decode("ascii"))
            if label not in widths:
                continue
            check(len(parts) == widths[label] and all(x.isascii() and x.isdecimal() for x in parts),
                  f"invalid {label} record")
            fields = tuple(map(int, parts))
            check(all(x < 1 << (128 if label in ("PASS", "BASE") else 64) for x in fields),
                  f"{label} field overflow")
            rows[label].append(fields)
    for label in ("CONFIG", "CONDITION", "DROPPED"):
        present(rows[label], f"missing {label}")
        check(len(rows[label]) == 1, f"duplicate {label}")
    return rows


def index_rows(rows, keys):
    result = {}
    for row in rows:
        key = row[:keys]
        check(key not in result, f"duplicate auxiliary record {key}")
        result[key] = row[keys:]
    return result


def lottery_windows(winners, width, n):
    windows = [[winners[start:start+width].count(i) for i in range(n)]
               for start in range(len(winners) - width + 1)]
    return dict(counts=[winners.count(i) for i in range(n)], windows=windows,
                minima=[min(column) for column in zip(*windows)])


def replay(manifest, rows, stride_prefix):
    version, mode, hz, quantum, seed, horizon, width, n = rows["CONFIG"][0]
    slots = manifest["slots"]
    lottery = manifest["policy"] == "lottery"
    check(manifest["policy"] in ("lottery", "stride") and mode == (0 if lottery else 1)
          and version == 1 and hz > 0 and quantum == manifest["quantum_ticks"] > 0
          and n == len(slots) and 1 <= n <= 3, "package/config mismatch")
    weights = [s["weight"] for s in slots]
    check(all(1 <= w <= 16 for w in weights), "invalid package weight")
    configured = manifest.get("lottery_window") or [0, 0]
    floors = manifest.get("lottery_floors") or [0] * n
    check([horizon, width] == configured and seed == (manifest.get("lottery_seed") or 0),
          "seed/window mismatch")
    check(len(rows["SLOT"]) == n and len(floors) == n, "missing slot metadata")
    roots = []
    for i, (slot, weight, floor, root) in enumerate(rows["SLOT"]):
        check((slot, weight, floor) == (i, weights[i], floors[i]), "slot parameters")
        roots.append(root)
    check(len(set(root & PA_MASK for root in roots)) == n, "shared process root")
    draws = index_rows(rows["DRAW"], 1)
    passes = index_rows(rows["PASS"], 2)
    bases = index_rows(rows["BASE"], 1)
    full_mask = (1 << n) - 1
    state = seed
    ledger = [0] * n
    service = [0] * n
    sleeps = {}
    winners = []
    leases = []
    scope = horizon if lottery else stride_prefix
    bad_prefix = None
    owner, start, compare, pending = IDLE, 0, 0, None
    exited = 0
    last_entry = 0
    saved_frames = {}
    held_mask = full_mask
    prefix_service = [0] * n
    prefix = []
    max_step = Fraction()
    for i, values in enumerate(rows["EVENT"]):
        e = Event(*values)
        def valid(ok, detail):
            check(ok, f"event {i}: {detail}")
        valid(e.seq == i and last_entry <= e.entry <= e.decision, "sequence/counter order")
        last_entry = e.entry
        valid(e.src == owner and e.dst in (*range(n), IDLE, PARK), "owner continuity")
        valid(e.before == held_mask and not e.after & ~full_mask, "ready mask")
        if e.src < n:
            valid(e.src_root == roots[e.src] and e.src_pc and e.src_sp and e.src_frame,
                  "interrupted frame identity")
            saved_frames[e.src] = (e.src_pc, e.src_sp, e.src_frame)
        valid(e.kind in range(4) and ((e.kind == 0) == (i == 0)), "entry kind")
        if e.kind == 1:
            valid((e.op & 1023) == 30 and e.eoi == e.op and e.cval == e.prior, "timer acknowledgement")
        closed = e.src < n and bool(e.stop or e.dt)
        if closed:
            valid(pending is not None and e.start == start and e.stop >= start
                  and e.stop - start == e.dt and e.prior == compare, "physical lease")
            voluntary = e.kind == 2 and (e.op in (1, 8) or e.sleep)
            valid(e.stop == (e.decision if not voluntary and e.entry < compare else e.entry),
                  "lease stop cutpoint")
            service[owner] += e.dt
            valid(service[owner] <= MAX, "cumulative service overflow")
            if not lottery:
                ledger[owner] += e.dt * (SCALE // weights[owner])
                valid(ledger[owner] < 1 << 128, "stride charge overflow")
            full = e.kind == 1 and e.op & 1023 == 30 and e.entry >= start + quantum and compare == start + quantum
            valid(bool(e.full) == full, "full quantum classification")
            if pending < scope:
                if (not full or e.before != full_mask) and bad_prefix is None:
                    bad_prefix = pending
                leases.append((owner, e.dt))
                if not lottery:
                    prefix_service[owner] += e.dt
                    normalized = [Fraction(s, w) for s, w in zip(prefix_service, weights)]
                    max_step = max(max_step, Fraction(e.dt, weights[owner]))
                    spread = max(normalized) - min(normalized)
                    if bad_prefix is None:
                        valid(spread <= max_step, "stride prefix fairness")
                    total = sum(prefix_service)
                    deviations = [abs(Fraction(s) - Fraction(w * total, sum(weights)))
                                  for s, w in zip(prefix_service, weights)]
                    prefix.append(dict(length=len(prefix)+1, owner=owner, elapsed=e.dt,
                                       cumulative=list(prefix_service), spread=str(spread),
                                       max_step=str(max_step), deviations=list(map(str, deviations))))
            pending = None
        else:
            valid(not e.full and e.dt == 0, "service without closure")
        charged = ledger.copy()
        mask = held_mask
        if e.kind == 2 and e.src < n and (e.op == 1 or (e.op == 7 and e.sleep)):
            mask &= ~(1 << e.src)
            if e.op == 7:
                sleeps[e.src] = e.sleep
            else:
                exited |= 1 << e.src
            if not lottery:
                ledger = change_mask(ledger, held_mask, mask)
        valid(e.exited == exited, "exited mask")
        valid(e.woke == sum(1 << pid for pid, at in sleeps.items() if at <= e.decision), "due wake mask")
        for deadline, pid in sorted((v, k) for k, v in sleeps.items()):
            if e.woke >> pid & 1:
                valid(deadline <= e.decision, "early wake")
                new = mask | (1 << pid)
                if not lottery:
                    ledger = change_mask(ledger, mask, new)
                mask = new
                del sleeps[pid]
        valid(mask == e.after and e.woke & ~mask == 0, "readiness transitions")
        before_select = ledger.copy()
        base = 0
        if e.selection and e.dst < n:
            valid(pending is None and e.ordinal == len(winners), "selection ordinal")
            choices = [pid for pid in range(n) if mask >> pid & 1]
            valid(bool(choices), "selection without eligible process")
            if lottery:
                winner, audit = draw(state, weights, mask)
                valid(draws.pop((i,), None) == audit, "raw RNG transcript")
                state = audit[1]
            else:
                winner = min(choices, key=lambda pid: (ledger[pid], pid))
                base = ledger[winner]
                ledger = [max(0, x-base) for x in ledger]
            valid(e.dst == winner and e.dst_root == roots[winner], "selected slot/root")
            if winner in saved_frames:
                valid(saved_frames[winner] == (e.dst_pc, e.dst_sp, e.dst_frame), "restore differs from save")
            valid(e.decision <= e.new_r <= e.returned and e.eret and e.new_r + quantum <= MAX and e.new_ctl & 3 == 1 and
                  e.new_cval == min([e.new_r + quantum, *sleeps.values()]), "dispatch comparator")
            start, compare, pending = e.new_r, e.new_cval, len(winners)
            winners.append(winner)
        elif e.dst >= n:
            valid(pending is None and mask == 0, "idle/terminal with eligible work")
        else:
            valid(owner == e.dst and pending is not None, "unselected user owner change")
        if not lottery:
            valid(bases.pop((i,), None) == (base,), "stride common offset")
        for pid in range(n):
            valid(passes.pop((i,pid), None) == (charged[pid],before_select[pid],ledger[pid],service[pid]),
                  f"slot {pid} pass or cumulative service")
        owner, held_mask = e.dst, mask
    present(rows["EVENT"], "missing events")
    present(rows["DROPPED"] == [(0,)], f"journal lost {rows['DROPPED'][0][0]} records")
    present(owner == PARK and exited == full_mask, "missing final park or process exits")
    check(not draws and not bases and not passes, "orphan policy records")
    result = dict(events=len(rows["EVENT"]), selections=len(winners), raw_ticks=service, frequency=hz)
    if lottery and horizon:
        check(1 <= width <= horizon <= 64, "invalid window")
        predicted, copied = [], seed
        for _ in range(horizon):
            pid, audit = draw(copied, weights, full_mask)
            predicted.append(pid)
            copied = audit[1]
        predicted_counts = lottery_windows(predicted, width, n)
        check(all(m >= floor for m, floor in zip(predicted_counts["minima"], floors)), "unadmitted configured floor")
        result["configured"] = predicted_counts
        if bad_prefix is None and len(leases) == scope:
            check(winners[:horizon] == predicted, "observed prefix differs from admission")
            result["observed"] = lottery_windows(winners[:horizon], width, n)
        reported, = rows["CONDITION"][0]
        check(reported == (MAX if bad_prefix is None else bad_prefix), "conditional horizon disposition")
    if scope:
        present(bad_prefix is None and len(leases) == scope, f"full continuously-ready prefix incomplete at {bad_prefix if bad_prefix is not None else len(leases)}")
    if prefix:
        result["stride_prefixes"] = prefix
        result["maximum_observed_step"] = str(max_step)
    return result

def products(package, mem, slots, roles, uart):
    expected = runpy.run_path(str(Path(__file__).resolve().parents[1] /
                              "examples/native-weighted-analytics/oracle.py"))["expected_products"]()
    exits = {int(i): int(code) for i, code in
             re.findall(rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?$", uart)}
    if len(slots) == 1:
        matches = re.findall(rb"(?m)^NATIVE EL0 EXIT (-?\d+)\r?$", uart)
        present(len(matches) == 1, "missing solo exit")
        exits = {0: int(matches[0])}
    present(len(exits) == len(slots), "missing process exits")
    check(len(roles) == len(slots), "provide a product role for each slot")
    outputs = []
    for i, (role, slot) in enumerate(zip(roles, slots)):
        root = slot["root"]
        elf = elf_file(package[i])
        def read(name, fmt):
            addr, size = symbol(elf, name)
            check(struct.calcsize(fmt) == size, f"slot {i} {name} size")
            return struct.unpack(fmt, vread(mem, root, addr, size))
        if role == "reporter":
            values = {key: list(read(key, "<3I")) for key in ("round_stage", "round_count", "cumulative_count")}
            requests, observed = read("request_ns", "<3Q"), read("observed_ns", "<3Q")
            check(values == dict(round_stage=[3]*3, round_count=[256]*3, cumulative_count=[256,512,768])
                  and all(0 < req <= obs for req, obs in zip(requests, observed))
                  and read("private_blocks", "<I") == (3,) and read("private_bytes", "<Q") == (12288,)
                  and read("private_result", "<I") == (768,) and exits[i] == 53, "reporter product")
            values.update(request_ns=requests, observed_ns=observed)
        else:
            check(role in ("crc", "histogram", "energy"), "unknown product role")
            oracle = expected["checksum" if role == "crc" else role]
            values = dict(progress=read("private_progress", "<I")[0],
                          result=read("private_result", {"crc":"<I","histogram":"<9I","energy":"<Q"}[role]))
            values["result"] = list(values["result"]) if role == "histogram" else values["result"][0]
            check(dict(values, exit=exits[i]) == oracle, f"slot {i} {role} product")
        outputs.append(dict(slot=i, role=role, exit=exits[i], **values))
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--session-sha256")
    parser.add_argument("--products", help="comma-separated roles in slot order")
    parser.add_argument("--stride-prefix", type=int, default=0)
    args = parser.parse_args()
    report = {}
    try:
        check(args.stride_prefix >= 0, "negative stride prefix")
        session_file = args.capture / "session.json"
        report["session_sha256"] = digest(session_file)
        if args.session_sha256:
            check(report["session_sha256"] == args.session_sha256, "session hash mismatch")
        session = json.loads(session_file.read_bytes())
        for name in ("ram.bin", "uart.bin", "launch.json", "kernel.elf"):
            present(name in session["hashes"], f"capture missing {name}")
            check(digest(args.capture / name) == session["hashes"][name], f"capture hash mismatch {name}")
        launch = json.loads((args.capture / "launch.json").read_bytes())
        manifest = json.loads((args.package / "manifest.json").read_bytes())
        check(launch["argument_package"] == manifest and manifest["kernel_elf_sha256"] ==
              launch["kernel_sha256"] == session["hashes"]["kernel.elf"], "package identity")
        uart = (args.capture / "uart.bin").read_bytes()
        mem = Snapshot(launch["ram_base"], (args.capture / "ram.bin").read_bytes())
        stacks = argument_stacks((args.capture / "kernel.elf").read_bytes(), mem, manifest, uart)
        blobs = []
        for slot in manifest["slots"]:
            filename = slot["elf"]
            check(Path(filename).name == filename, "invalid packaged ELF path")
            blob = (args.package / filename).read_bytes()
            check(hashlib.sha256(blob).hexdigest() == slot["elf_sha256"], "packaged ELF digest")
            blobs.append(blob)
        rows = parse(uart)
        check([r[3] & PA_MASK for r in rows["SLOT"]] == [s["root"] for s in stacks], "slot root mismatch")
        if args.products:
            report["products"] = products(blobs, mem, stacks, args.products.split(","), uart)
        report["ledger"] = replay(manifest, rows, args.stride_prefix)
        report["status"] = "complete"
    except (FileNotFoundError, Incomplete) as exc:
        report.update(status="incomplete", detail=str(exc))
    except (ValueError, KeyError, IndexError, OSError, struct.error) as exc:
        report.update(status="mismatch", detail=str(exc))
    print(json.dumps(report, indent=2))
    return {"complete": 0, "mismatch": 1, "incomplete": 2}[report["status"]]


if __name__ == "__main__":
    sys.exit(main())
