"""Decode the sealed control-marker journal and reconcile the unchanged stock UART debits."""
import argparse
import json
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


def inspect(journal, uart):
    lines = uart.read_text().splitlines()
    first = lines.index("CONTROL RAW v1")
    require(lines[-1] == "CONTROL END", "truncated UART")
    raw = [tuple(map(int,s.split())) for s in lines[first+1:-1]]
    require(all(len(r) == 4 for r in raw), "malformed UART")
    rows = decode(journal)
    slices = []
    selection = restore = None
    for r in rows:
        k = r["kind"]
        if k == 1:
            require(selection is None, "lost vector")
            selection = r
        elif k == 4:
            require(restore is None, "duplicated restore")
            restore = r
        elif k in (2,3):
            require(restore is not None and restore["t"] <= restore["a"] <= r["t"] <= r["b"],
                    "invalid restore/vector order")
            if selection:
                require(selection["t"] <= restore["t"] and selection["b"] == restore["b"],
                        "selected and restored ELR disagree")
                if selection["a"] < 2:
                    slices.append(dict(task=selection["a"],start=selection["t"],end=r["t"],
                        restore_start=restore["t"],restore_end=restore["a"],restored_pc=restore["b"],
                        interrupted_pc=r["a"],exit="IRQ" if k==2 else "SVC"))
            elif slices:
                raise ValueError("missing selection")
            selection = restore = None
    require(selection is not None and selection["a"] == 2 and restore is None,
            "expected final idle selection before terminal seal")
    accounting = [(r[1],r[2],r[3]) for r in raw if r[0]==6]
    require(accounting == [(s["end"],s["task"],s["end"]-s["start"]) for s in slices],
            "unchanged row 6 slice reconciliation failed")
    balance = [0,0]
    for k,t,a,b in raw:
        if k == 4:
            balance[a] = 0
        elif k == 6:
            balance[a] += b
        elif k == 8:
            require(balance[a] == b, "unchanged row 8 service reconciliation failed")
            balance[a] = 0
    checks = [(r["a"],r["b"],r["t"]) for r in rows if r["kind"]==8]
    require(checks == [(t,i,d) for k,t,i,d in raw if k==21], "decision marker differs from row 21")
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
                              prior_daif=r["b"], guaranteed_ticks=r["t"]-enter["a"]))
    require(not open_masks, "incomplete control masking bracket")
    for s in slices:
        start, end = s["start"], s["end"]
        ranges = [(start,s["restore_start"],"selection_to_restore"),
                  (s["restore_start"],s["restore_end"],"restore_body")]
        s["observations"] = []
        for r in rows:
            if start < r["start"] <= r["end"] < end:
                ranges.append((r["start"],r["end"],"marker_write"))
            if start <= r["t"] <= end and r["kind"] in (5,6,7,8,9,10):
                s["observations"].append(r["n"])
                if r["kind"] in (5,10) and s["restore_end"] <= r["t"]:
                    ranges.append((s["restore_end"],r["t"],"restore_tail_to_worker_sample"))
                if r["kind"] == 8:
                    require(start <= r["a"] <= end,"decision crosses slice")
                    ranges.append((r["t"],r["a"],"actual_check"))
                    phase = r["b"]
                    s["phase"] = phase
                    s["phase_class"] = ("first" if phase==0 else
                                        "same_job_continuation" if phase%2 else "later_job_first")
        cuts = sorted({start,end,*(t for lo,hi,_ in ranges for t in (lo,hi))})
        totals = {}
        for lo,hi in zip(cuts,cuts[1:]):
            names = {name for a,b,name in ranges if a <= lo and hi <= b}
            name = "marker_write" if "marker_write" in names else next(iter(names),"remaining_charged")
            require(len(names-{"marker_write"}) <= 1, "overlapping attribution intervals")
            totals[name] = totals.get(name,0)+hi-lo
        require(sum(totals.values()) == end-start, "attribution does not conserve ticks")
        s["disjoint_ticks"] = totals
    return dict(markers=rows,slices=slices,control_mask_brackets=masks,stock_debits=[r for r in raw if r[0]==8],
                absolute_releases=[r for r in raw if r[0] in (2,3,4)],
                phase_completions=[r for r in raw if r[0] in (10,21)])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("journal",type=Path)
    parser.add_argument("uart",type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(inspect(args.journal,args.uart),indent=2))
    except (ValueError, IndexError, OSError) as exc:
        parser.exit(1, f"invalid capture: {exc}\n")
