"""Replay WEIGHTED1 UART using integer half-ticks. Python 3.10+."""
import argparse
from pathlib import Path


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def replay(path, wake, diagnostic):
    lines = path.read_bytes().decode("ascii").splitlines()
    require(lines[-1:] == ["WEIGHTED TERMINAL"], "missing terminal")
    records = [s.split() for s in lines if s.startswith(("WEIGHTED1 ", "W ", "F "))]
    header = records.pop(0)
    mode, hz, q, cap, base_a, base_b, stack, frame = map(int, header[1:])
    require(header[0] == "WEIGHTED1" and mode == wake and hz == 100*q
            and q > 0 and q % 4 == 0 and cap == 64, "header")
    bases = [base_a, base_b]
    require(base_b == base_a+stack and 0 < frame < stack, "stack layout")
    h, g = 5*q//4, q//4
    service, vr = [0, 0], [0, 0]
    ready, returned, saved = {0, 1}, set(), {}
    floor = last = count = timers = cycles = solo = resumed = 0
    sleep_service = accepted = resumed_start = None
    progress = [0, 0]
    while records:
        row = records.pop(0)
        require(row[0] == "W", "closure tag")
        (seq, task, cause, mask, start, due, stop, gap, pre_a, pre_b, charge_a,
         charge_b, place_a, place_b, a, b, f0, f1, f2, stamp, pa, pb, gen,
         nsolo, witness, nr, sel0, sel1) = map(int, row[1:])
        count += 1
        require(seq == count <= cap and mask == sum(1 << i for i in ready), "sequence/mask")
        require(task == min(ready, key=lambda i: (vr[i], i)), "owner")
        require([pre_a, pre_b] == vr and f0 == floor, "pre-state")
        require(last <= sel0 <= sel1 <= start and gap == (start-last if last else 0)
                and gap <= g and due == start+q, "dispatch timing")
        if diagnostic:
            f = records.pop(0)
            lease, restore, pc, save, elr, mark = map(int, f[1:])
            require(f[0] == "F" and lease == seq and start <= mark <= stop, "restore marker")
            require(all(v % 16 == 0 and bases[task] <= v <= bases[task]+stack-frame
                        for v in (restore, save)), "frame extent")
            if task in saved:
                require((restore, pc) == saved[task], "restore frame/ELR")
            saved[task] = save, elr
            print("cost", seq, "select", sel1-sel0, "dispatch-to-restore-marker", mark-start)
        ticks = stop-start
        require(0 < ticks <= h and (cause != 0 or ticks >= q), "elapsed")
        service[task] += ticks
        vr[task] += ticks*(2-task)
        floor = max(floor, min(vr[i] for i in ready))
        require(service == [a,b] and vr == [charge_a,charge_b] and floor == f1, "charge")
        if accepted is not None:
            wake_seq, when = accepted
            if seq == wake_seq+1:
                require(task == 0 and cause == 0, "one A timer grant")
            else:
                require(seq == wake_seq+2 and task == 1 and start-when <= h+2*g, "wake dispatch")
                resumed_start = start, when
                accepted = None
        if nr > resumed:
            require(resumed_start is not None and nr == resumed+1 and task == 1, "resume generation")
            first, when = resumed_start
            require(first <= witness <= min(first+g,stop) and pb > progress[1], "resume marker/progress")
            print("resume", nr, "wake-to-dispatch", first-when, "wake-to-work", witness-when)
            resumed_start = None
            resumed = nr
        if cause == 1:
            require((resumed == 3 if wake else timers >= 36), "early return")
            ready.remove(task)
            returned.add(task)
        elif cause == 2:
            require(wake and task == 1 and cycles == resumed < 3 and sleep_service is None, "block")
            cycles += 1
            solo = 0
            sleep_service = service.copy()
            ready.remove(task)
        elif cause == 3:
            require(wake and task == 0 and ready == {0} and sleep_service is not None, "wake")
            require(3 <= solo <= 4 and b == sleep_service[1] and a-sleep_service[0] >= 3*q,
                    "sleep service")
            require(vr[1] < floor == vr[0] and stop <= stamp, "placement input")
            vr[1] = floor
            ready.add(1)
            sleep_service = None
            accepted = seq, stamp
        elif cause == 0:
            timers += 1
            if sleep_service is not None:
                require(task == 0, "sleep owner")
                solo += 1
            if not wake and timers <= 36:
                d = 2*a-b
                require(ready == {0,1} and -h <= d <= 2*h, "prefix discrepancy")
                if timers == 36:
                    require(a+b >= 36*q and abs(d)*36*q <= 2*h*(a+b), "share bound")
                    print("horizon", seq, "service", a,b, "total", a+b, "D", d,
                          "shares", a/(a+b), b/(a+b), "progress", pa,pb)
        else:
            raise ValueError("unknown cause")
        if ready:
            floor = max(floor, min(vr[i] for i in ready))
        require(vr == [place_a,place_b] and floor == f2 and (cause == 3 or stamp == 0), "post-state")
        require((gen,nsolo,nr) == (cycles,solo,resumed), "workload counters")
        if wake:
            print("B lag", seq, "charged", f1-charge_b, "after event", f2-place_b)
        require(pa >= progress[0] and pb >= progress[1], "progress regression")
        progress = [pa,pb]
        last = stop
    require(returned == {0,1} and min(progress) > 0 and not ready, "terminal ownership")
    require(resumed == 3 if wake else timers >= 36, "horizon")
    print("terminal service", service, "progress", progress, "closures", count)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart", type=Path)
    parser.add_argument("--wake", action="store_true")
    parser.add_argument("--diagnostic", action="store_true")
    args = parser.parse_args()
    replay(args.uart, args.wake, args.diagnostic)
