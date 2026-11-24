"""Decode the sealed control-marker journal and reconcile the unchanged stock UART debits."""
import argparse
import json
import re
from pathlib import Path
import struct


def require(ok, message):
    if not ok:
        raise ValueError(message)


def decode(path):
    raw = path.read_bytes()
    require(len(raw) == (3 + 7 * 1024) * 8, "wrong journal extent")
    used, overflow, sealed = struct.unpack_from("<3Q", raw)
    require(sealed == 1 and overflow == 0 and 0 < used <= 1024, "unsealed/overflowed journal")
    rows = []
    for i in range(used):
        kind,t,a,b,start,end,commit = struct.unpack_from("<7Q", raw, 24 + i*56)
        require(commit == i+1 and kind in range(1,11) and start <= end, "invalid marker row")
        rows.append(dict(n=i,kind=kind,t=t,a=a,b=b,start=start,end=end))
    return rows


def check_external(path, expected):
    events = []
    pending = None
    for line in path.read_text().splitlines():
        if line.startswith("Exception return"):
            m = re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC (0x[0-9A-Fa-f]+)", line)
            require(m and pending is None, "invalid exception return")
            events.append(("return", int(m[1], 16)))
        elif line.startswith("Taking exception"):
            m = re.fullmatch(r"Taking exception (5 \[IRQ\]|2 \[SVC\])", line)
            require(m and pending is None, "unexpected exception")
            pending = "IRQ" if m[1].startswith("5") else "SVC"
        elif line.startswith("...with ELR"):
            require(pending is not None, "orphan ELR")
            events.append((pending, int(line.split()[-1], 16)))
            pending = None
    require(pending is None and events == expected, "external return/vector sequence or terminal PC differs")


def inspect(journal, uart, exceptions=None):
    lines = uart.read_text().splitlines()
    first = lines.index("CONTROL RAW v1")
    require(lines[-1] == "CONTROL END", "truncated UART")
    raw = [tuple(map(int,s.split())) for s in lines[first+1:-1]]
    require(all(len(r) == 4 and all(0 <= x < 2**64 for x in r) for r in raw), "malformed unsigned UART")
    def events(kind):
        return [r[1:] for r in raw if r[0] == kind]
    epochs = events(1)
    require(len(epochs) == 1, "invalid epoch record")
    epoch, freq, final_phase = epochs[0]
    require(freq > 0 and freq % 400 == 0 and final_phase == 25, "invalid healthy manifest")
    step = freq // 400
    rows = decode(journal)
    slices = []
    selection = restore = None
    expected = []
    for r in rows:
        k = r["kind"]
        if k == 1:
            require(selection is None and r["a"] in (0,1,2), "invalid selection or lost vector")
            selection = r
        elif k == 4:
            require(restore is None, "duplicated restore")
            restore = r
        elif k in (2,3):
            require(restore is not None and restore["t"] <= restore["a"] <= r["t"] <= r["b"],
                    "invalid restore/vector order")
            expected.extend([("return",restore["b"]), ("IRQ" if k==2 else "SVC",r["a"])])
            if selection:
                require(selection["t"] <= restore["t"] and selection["b"] == restore["b"],
                        "selected and restored ELR disagree")
                if selection["a"] < 2:
                    slices.append(dict(task=selection["a"],start=selection["t"],end=r["t"],
                        restore_start=restore["t"],restore_end=restore["a"],restored_pc=restore["b"],
                        interrupted_pc=r["a"],exit="IRQ" if k==2 else "SVC"))
            elif len(expected) != 2:
                raise ValueError("missing selection")
            selection = restore = None
    require(selection is not None and selection["a"] == 2 and restore is None,
            "expected final idle selection before terminal seal")
    expected.append(("return", selection["b"]))
    if exceptions is not None:
        check_external(exceptions, expected)
    accounting = [(r[1],r[2],r[3]) for r in raw if r[0]==6]
    require(accounting == [(s["end"],s["task"],s["end"]-s["start"]) for s in slices],
            "unchanged row 6 slice reconciliation failed")
    debits = events(8)
    require([(t,a) for t,a,_ in debits] == [(s["end"],s["task"]) for s in slices if s["exit"]=="SVC"],
            "missing or duplicate SVC debit")
    balance = [0,0]
    for k,t,a,b in raw:
        if k == 4:
            require(a < 2 and balance[a] == 0, "unsettled prior release")
        elif k == 6:
            balance[a] += b
        elif k == 8:
            require(balance[a] == b, "unchanged row 8 service reconciliation failed")
            balance[a] = 0
    checks = [(r["a"],r["b"],r["t"]) for r in rows if r["kind"]==8]
    require(checks == [(t,i,d) for k,t,i,d in raw if k==21], "decision marker differs from row 21")
    phases = {}
    finishes = events(10)
    control_debits = [(t,b) for t,a,b in debits if a==0]
    require(len(checks) == len(finishes) == len(control_debits) == final_phase+1, "missing or duplicate phase")
    for index, ((done, phase, begin), (finish, completed, target), (trap,debit)) in enumerate(zip(checks, finishes, control_debits)):
        require(phase == completed == index and target == epoch + index*step, "phase identity or target mismatch")
        require(target <= begin <= done <= finish <= trap, "decision, phase finish or SVC order")
        phases[index] = dict(phase=index, decision_start=begin, decision_end=done, check_ticks=done-begin,
                             finish=finish, svc_entry=trap, debit=debit)
    matched = set()
    masks = []
    open_masks = []
    for r in rows:
        if r["kind"] == 6:
            require(r["t"] <= r["a"], "negative mask entry bracket")
            open_masks.append(r)
        elif r["kind"] == 7:
            require(open_masks and r["t"] <= r["a"], "unpaired mask exit")
            enter = open_masks.pop()
            require(enter["b"] == r["b"] and enter["a"] <= r["t"], "mask state/order mismatch")
            masks.append(dict(before=enter["t"], inside=enter["a"], restore=r["t"], after=r["a"],
                              prior_daif=r["b"], guaranteed_ticks=r["t"]-enter["a"], outer_ticks=r["a"]-enter["t"]))
    require(not open_masks, "incomplete control masking bracket")
    for s in slices:
        start, end = s["start"], s["end"]
        ranges = [(start,s["restore_start"],"selection_to_restore"),
                  (s["restore_start"],s["restore_end"],"restore_body")]
        s["observations"] = []
        for r in rows:
            if start <= r["start"] <= r["end"] <= end:
                ranges.append((r["start"],r["end"],"marker_write"))
            if start <= r["t"] <= end and r["kind"] in (5,6,7,8,9,10):
                s["observations"].append(r["n"])
                if r["kind"] in (5,10) and s["restore_end"] <= r["t"]:
                    ranges.append((s["restore_end"],r["t"],"restore_tail_to_worker_sample"))
                if r["kind"] == 8:
                    require(s["task"] == 0 and start <= r["t"] <= r["a"] <= end, "decision crosses control slice")
                    require(r["b"] not in matched and "phase" not in s, "duplicate decision ownership")
                    matched.add(r["b"])
                    ranges.append((r["t"],r["a"],"actual_check"))
                    phase = r["b"]
                    s["phase"] = phase
                    s["phase_class"] = ("first" if phase==0 else
                                        "same_job_continuation" if phase%2 else "later_job_first")
        if s["task"] == 0 and s["exit"] == "SVC":
            phase = next((v for v in phases.values() if v["svc_entry"] == end), None)
            require(phase is not None, "control SVC has no phase")
            calls = [r for r in rows if r["kind"] == 9 and phase["finish"] <= r["t"] <= end]
            require(len(calls) == 1, "missing or duplicate pre-SVC marker")
            call = calls[0]
            index = phase["phase"]
            require(call["a"] == (1 if index%2 == 0 else 2)
                    and call["b"] == (index if index%2 == 0 else 0), "SVC disposition differs from phase")
            # Wall-time overlays can span a preemption; keep them outside the disjoint debit.
            phase.update(pre_svc=call["t"], finish_to_pre_svc=call["t"]-phase["finish"],
                         pre_svc_to_vector=end-call["t"])
        require(all(start <= lo <= hi <= end for lo,hi,_ in ranges), "invalid unsigned attribution interval")
        cuts = sorted({start,end,*(t for lo,hi,_ in ranges for t in (lo,hi))})
        totals = {}
        for lo,hi in zip(cuts,cuts[1:]):
            names = {name for a,b,name in ranges if a <= lo and hi <= b}
            name = "marker_write" if "marker_write" in names else next(iter(names),"remaining_charged")
            require(len(names-{"marker_write"}) <= 1, "overlapping attribution intervals")
            totals[name] = totals.get(name,0)+hi-lo
        require(sum(totals.values()) == end-start, "attribution does not conserve ticks")
        s["disjoint_ticks"] = totals
    require(matched == set(phases), "unattributed decision")
    require(len([r for r in rows if r["kind"] == 9]) == len(phases), "unpaired SVC marker")
    return dict(phases=list(phases.values()), external_trace_checked=exceptions is not None,
                final_selected_idle_pc=selection["b"], markers=rows,slices=slices,control_mask_brackets=masks,stock_debits=[r for r in raw if r[0]==8],
                absolute_releases=[r for r in raw if r[0] in (2,3,4)],
                phase_completions=[r for r in raw if r[0] in (10,21)])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("journal",type=Path)
    parser.add_argument("uart",type=Path)
    parser.add_argument("--exceptions",type=Path,help="ordered QEMU -d int output for this exact boot")
    args = parser.parse_args()
    try:
        print(json.dumps(inspect(args.journal,args.uart,args.exceptions),indent=2))
    except (ValueError, IndexError, OSError) as exc:
        parser.exit(1, f"invalid capture: {exc}\n")
