"""Associate read-only host callbacks with the ordinary guest and exception ledgers."""
import argparse
import json
from pathlib import Path
import re
import struct

from ordinary_mutex_host_capture import ELFS, SITES, digest


def require(ok, message):
    if not ok:
        raise ValueError(message)


def guest(directory, app):
    lines = (directory / "uart").read_text().splitlines()
    require(lines[:3] == ["JNOX M1 BOOT", "JNOX M1 READY", "MUTEX RAW 1"]
            and lines[-1] == "MUTEX END", "UART framing")
    rows = {}
    for line in lines[3:-1]:
        tag, *fields = line.split()
        rows.setdefault(tag, []).append([int(x, 16) for x in fields])
    events, returns = rows["E"], rows["R"]
    (ce, reason, ne, nr, nc), = rows["H"]
    require(ce == int(app == "ceiling") and ne == len(events) == nr == len(returns)
            and nc == len(rows.get("C", [])), "ledger counts/mode")
    trace = (directory / "external.log").read_text().splitlines()
    require(len(trace) == 6 * ne, "unpaired trace record")
    for i, (r, e) in enumerate(zip(returns, events)):
        kind, vector = ("5 [IRQ]", "40201280") if e[2] == 5 else ("2 [SVC]", "40201200")
        ret, taking, level, syndrome, saved, target = trace[6*i:6*i+6]
        require(ret == f"Exception return from AArch64 EL1 to AArch64 EL1 PC 0x{r[1]:x}"
                and taking == "Taking exception " + kind and level == "...from EL1 to EL1"
                and re.fullmatch(r"\.\.\.with ESR 0x[0-9a-f]+/0x[0-9a-f]+", syndrome)
                and saved == f"...with ELR 0x{e[7]:x}"
                and target == f"...to EL1 PC 0x{vector} PSTATE 0x3c5", "exception/return mismatch")
        if i:
            require(r[:2] == events[i-1][10:12] and events[i-1][1] <= r[2], "return ordinal/frame")
    (n,) = [i for i, e in enumerate(events) if e[2:6] == [1, ce, 0, 0] and e[19] == 0]
    (k,) = [i for i, e in enumerate(events) if e[2] == 5]
    e, irq = events[n], events[k]
    (freq, start, target, readback, ctl, iar, irqctl, icv), = rows["T"]
    require(freq == 62500000 and target == readback == icv == start+6250
            and ctl & 3 == 1 and iar == 30 and irqctl & 7 == 5, "physical timer")
    low, priority, lo, hi, size = rows["S"][0]
    dest = int(SITES[app][2].split(":")[0], 16)
    require(k == n+1 and low == 0 and priority == 2 and size == 832
            and lo <= e[6] <= hi-size and e[6] % 16 == 0 and e[6:8] == [e[10], dest]
            and e[11] == dest and e[23:25] == [1, 1] and e[14+ce] == 0
            and irq[4] == 0 and irq[23] == 1 and lo <= irq[6] <= hi-size
            and irq[8] & 128 == 0 and irq[0] >= max(target, returns[k][2]), "LOW acquire to IRQ")
    return k, nr, {"guest_reason": reason, "return_ordinal": k+1, "guest_task": "LOW",
                   "return_frame": hex(e[10]), "return_pc": hex(dest), "irq_elr": hex(irq[7]),
                   "irq_chunks": irq[20], "irq_active": irq[21], "final_chunks": nc}


def host(path, k, nr, result):
    data = path.read_bytes()
    require(len(data) == 32+48*2048, "marker size")
    count, invalid, rw, rc = struct.unpack_from("<4Q", data)
    require(not invalid and 0 < count <= 2048 and rw and rc, "marker header")
    require(not any(data[32+48*count:]), "uncommitted marker tail")
    rows = list(struct.iter_unpack("<6Q", data[32:32+48*count]))
    tid = rows[0][2]
    wall = cpu = 0
    for s, vcpu, thread, a, c, b in rows:
        require(s < 6 and vcpu == 0 and tid == thread and tid > 0 and wall <= a <= b
                and cpu <= c, "marker identity or clock order")
        wall, cpu = b, c
    sites = [r[0] for r in rows]
    starts = [i for i, s in enumerate(sites) if s == 0]
    require(len(starts) == nr and [s for s in sites if s < 2] == [0, 1]*nr
            and all(sites[i:i+2] == [0, 1] for i in starts), "ERET epoch ambiguity")
    first = starts[k]+1
    (last,) = [i for i, s in enumerate(sites) if s == 5]
    require(first < last < (starts[k+1] if k+1 < nr else count), "IRQ epoch ambiguity")
    mid = sites[first+1:last]
    require(mid in ([], [2], [2, 3], [2, 3, 4]), "ambiguous return/work sequence")
    if not mid:
        require(result["irq_elr"] == result["return_pc"], "absent destination with advanced ELR")
    require((4 not in mid or result["irq_active"] == 1)
            and (3 in mid or result["irq_active"] == 0), "work publication disagreement")
    require(result["irq_chunks"] == 0 or 4 in mid, "chunk before protected work")
    intervals = {}
    for j in range(first+1, last+1):
        a, b = rows[first], rows[j]
        w = [max(0, b[3]-a[5]-2*rw), b[5]-a[3]+2*rw]
        c = [max(0, b[4]-a[4]-2*rc), b[4]-a[4]+2*rc]
        intervals[sites[j]] = {"wall_ns": w, "thread_cpu_ns": c,
                               "wall_minus_cpu_ns": [w[0]-c[1], w[1]-c[0]]}
    return {"host_tid": tid, "vcpu": 0, "wall_cpu_resolution_ns": [rw, rc],
            "rows_site_vcpu_tid_wall0_cpu_wall1": rows[first-1:last+1],
            "from_pre_eret_by_site": intervals}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    root = parser.parse_args().directory
    reports = []
    for i, (app, observed) in enumerate((a, o) for a in ELFS for o in (False, True)):
        label = "observed" if observed else "unobserved"
        directory = root / f"0{i}-{app}-{label}"
        report = {"position": directory.name}
        try:
            require(digest((root / (app+".elf")).read_bytes()) == ELFS[app], "ELF digest")
            require((directory / "rc").read_text() == "124", "cold launch status")
            k, nr, result = guest(directory, app)
            report.update(result)
            if observed:
                report.update(host(directory / "marker.bin", k, nr, result))
        except (KeyError, IndexError, ValueError, OSError, struct.error) as error:
            report["omitted"] = str(error)
        reports.append(report)
    print(json.dumps(reports, indent=2))
    return int(any("omitted" in r for r in reports))


if __name__ == "__main__":
    raise SystemExit(main())
