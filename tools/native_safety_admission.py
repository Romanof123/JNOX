"""Offline timing screen for one native permit qualification capture (Python 3.9+)."""
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path


def digest(data):
    return hashlib.sha256(data).hexdigest()


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def rows(uart, prefix):
    return [list(map(int, line[len(prefix):].split())) for line in uart.splitlines()
            if line.startswith(prefix)]


def timing_bounds(path, kernel, policy, gaps):
    """Accept externally justified bounds on *response*, never EL0-only WCET."""
    if path is None:
        gaps.append("missing separately justified full-response bounds")
        return {}
    evidence = json.loads(path.read_bytes())
    if evidence["kernel_sha256"] != kernel or evidence["policy"] != policy:
        raise ValueError("bounds target another kernel or policy")
    limits = {}
    for key in ("A", "B", "sender"):
        entry = evidence.get("bounds", {}).get(key)
        if entry is None:
            gaps.append(f"missing justified {key} response bound")
            continue
        if not integer(entry["response_us"], 1) or not str(entry["section"]).strip():
            raise ValueError(f"invalid {key} bound or section")
        artifact = (path.parent / entry["artifact"]).read_bytes()
        if not artifact or digest(artifact) != entry["sha256"]:
            raise ValueError(f"{key} bound reference failed hash verification")
        limits[key] = entry["response_us"]
    return limits


def assess(directory, bounds_path):
    pkg, cap = directory / "pkg", directory / "cap"
    manifest_bytes = (pkg / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    launch_bytes = (cap / "launch.json").read_bytes()
    launch = json.loads(launch_bytes)
    receipt = json.loads((cap / "session.json").read_bytes())["hashes"]
    uart_bytes = (cap / "uart.bin").read_bytes()
    uart = uart_bytes.decode("ascii")
    kernel = digest((pkg / "kernel.elf").read_bytes())
    policy = manifest["policy"]
    if (receipt["uart.bin"] != digest(uart_bytes) or receipt["launch.json"] != digest(launch_bytes)
            or receipt["kernel.elf"] != kernel or launch["argument_package"] != manifest
            or kernel != manifest["kernel_elf_sha256"]
            or kernel != launch["kernel_sha256"] or kernel != digest((cap / "kernel.elf").read_bytes())):
        raise ValueError("kernel or manifest provenance mismatch")
    failures, gaps, segments = [], [], []
    result = dict(policy=policy, commit=manifest["commit"], kernel_sha256=kernel,
                  manifest_sha256=digest(manifest_bytes), uart_sha256=digest(uart_bytes),
                  failures=failures, missing=gaps, segments=segments)
    argv = launch["argv"]
    if (policy not in ("fifo", "rr") or manifest["board"] != "virt"
            or manifest.get("tracked_changes") or "-smp" not in argv
            or argv.count("-smp") != 1 or argv[argv.index("-smp")+1] != "1"):
        raise ValueError("requires a clean single-core native virt FIFO or RR capture")
    qual = manifest.get("safety_qualification", {})
    if (qual.get("controller_phase_offsets_us") != [0, 2500]
            or [qual.get(k) for k in ("syscall", "sender_slot", "controller_slot", "supervisor_slot",
                                       "controller_period_us", "sender_period_us")] != [62, 0, 1, 2, 5000, 10000]):
        raise ValueError("requires the SVC 62 native qualification profile")
    slots = manifest["slots"]
    if [s["slot"] for s in slots] != [0, 1, 2]:
        raise ValueError("requires sender, controller and supervisor slots")
    if not slots[1]["priority"] > slots[0]["priority"] > slots[2]["priority"]:
        failures.append("controller > sender > supervisor rank requirement")
    if policy == "rr" and not integer(manifest.get("quantum_ticks"), 1):
        raise ValueError("invalid RR quantum")
    result["case_argv"] = [s["argv"] for s in slots]
    result["slot_sha256"] = [s["elf_sha256"] for s in slots]
    try:
        limits = timing_bounds(bounds_path, kernel, policy, gaps)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        gaps.append(f"rejected timing evidence: {error}")
        limits = {}
    result["submitted_response_bounds_us"] = limits
    counts = rows(uart, "NATIVE QUAL COUNTS ")
    raw = rows(uart, "NATIVE QUAL ROW ")
    if len(counts) != 1 or len(counts[0]) != 4:
        raise ValueError("missing or ambiguous qualification counter metadata")
    hz, attempts, kept, lost = counts[0]
    if hz <= 0 or kept != len(raw) or attempts != kept + lost:
        raise ValueError("invalid counter metadata")
    if any(len(r) != 13 or r[0] != i for i,r in enumerate(raw)):
        raise ValueError("missing or malformed retained journal rows")
    result.update(counter_hz=hz, retained=kept, lost=lost)
    if rows(uart, "NATIVE SAFETY DROPPED ") != [[0]] or lost:
        gaps.append("incomplete safety or scheduling journal")
    exits = rows(uart, "NATIVE EL0 EXIT PID ")
    if any(len(r)!=2 or r[0] not in (0,1,2) or r[1]!=71+r[0] for r in exits):
        failures.append("unexpected application exit")
    if sorted(exits) != [[0,71],[1,72],[2,73]]:
        gaps.append("incomplete expected process exits")
    # Row numbers below include the append index at [0].
    marks = {}
    for row in raw:
        if row[1] == 2:
            runnable = [s for s in slots if row[7] & (1 << s["slot"])]
            if runnable and row[9] != max(runnable, key=lambda s:s["priority"])["slot"]:
                failures.append(f"dispatch rank at row {row[0]}")
        if row[1] == 1:
            key = tuple(row[2:6])  # pid, job, phase, stage
            if key in marks or row[10] != 0:
                failures.append(f"duplicate or rejected mark {key}")
            marks[key] = row
    origin = marks.get((1,0,1,1))
    if origin is None:
        gaps.append("missing epoch marker")
    for pid,total in ((1,32),(0,8)):
        for index in range(total):
            job,phase = (index//2,1+index%2) if pid==1 else (index,3)
            name = {1:"A",2:"B",3:"sender"}[phase]
            key = (pid,job,phase)
            start, work, end = (marks.get((*key,i)) for i in (1,2,3))
            if any(r is None for r in (start, work, end)):
                gaps.append(f"missing marks {key}")
                continue
            target = start[6]
            if (origin and target != origin[6]+index*(2500 if pid==1 else 10000)*hz//1000000
                    or not target==work[6]==end[6] or not start[0]<work[0]<end[0]
                    or not start[7]<=work[7]<=end[7] or work[7]<target):
                failures.append(f"grid or marker order {key}")
                continue
            # The next stage-1 call blocks this worker. Final jobs park via receive.
            nxt = next((r for r in raw[end[0]+1:] if r[1]==1 and r[2]==pid), None)
            event = None
            if nxt is not None and nxt[5]==1:
                event = next((r for r in raw[nxt[0]+1:] if r[1]==2 and r[2]==62
                              and r[3]==nxt[8] and r[8]==pid),None)
            elif index==total-1 and nxt is None:
                event = next((r for r in raw[end[0]+1:] if r[1]==2 and r[2]==22 and r[8]==pid),None)
            lower = end[7]-target
            upper = None
            if event and event[6] & (1<<pid) and not event[7] & (1<<pid):
                lower=max(lower,event[4]-target)
                upper=event[5]-target
                if upper < lower:
                    raise ValueError(f"inconsistent block bracket {key}")
            else:
                if event and nxt is not None and nxt[4] == 2 and event[7] & (1 << pid):
                    failures.append(f"controller continuation did not block at row {event[0]}")
                gaps.append(f"missing whole-segment blocking boundary {key}")
            ceiling=1000 if pid==1 else 10000
            segments.append(dict(pid=pid,job=job,phase=name,target=target,lower_ticks=lower,
                                 upper_ticks=upper,end_row=end[0],block_row=event[0] if event else None))
            for label,us in (("profile",ceiling),("submitted",limits.get(name))):
                if us is None:
                    continue
                if lower*1000000 > us*hz:
                    failures.append(f"{key} measured lower bound exceeds {label} {us} us")
                elif upper is None or upper*1000000 > us*hz:
                    gaps.append(f"{key} {label} limit unresolved inside bracket")
    gaps.extend(["finite captures do not establish worst-case dispatch, service, blocking or release jitter",
                 "timer reprogramming, other IRQs, supervisor interference and pending carry-in lack a proved envelope",
                 "universal continuation, rank/idle applicability, check gaps and endpoint uncertainty require independent review"])
    result["status"] = "FAIL" if failures else "UNKNOWN/HOLD"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, help="extracted case with pkg/ and cap/")
    parser.add_argument("--bounds", type=Path)
    args=parser.parse_args()
    try:
        result=assess(args.capture,args.bounds)
    except (OSError, ValueError, KeyError, IndexError, TypeError) as error:
        result=dict(status="UNKNOWN/HOLD",input_error=str(error))
    print(json.dumps(result,indent=2))
    return 1 if result["status"]=="FAIL" else 2


if __name__ == "__main__":
    sys.exit(main())
