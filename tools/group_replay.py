"""Replay the ordinary-groups sample against its declared quotas and frame handoffs."""
import argparse
import struct
from pathlib import Path

ROW = struct.Struct("<15Q")
# Source, previous, next, ready mask. Idle has index 3.
FLOW = [(0,3,0,7), (2,0,1,6), (3,1,0,7), (1,0,2,7),
        (4,2,3,3), (1,3,0,3), (4,0,1,2), (4,1,3,0)]
RECURRING_FLOW = FLOW[:3] + [(1,0,2,7), (1,2,3,7), (1,3,0,7)] * 2 + FLOW[3:]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def replay(data, late=False, recurring=False):
    require(len(data) == 65 * ROW.size, "journal size")
    header, *rows = list(ROW.iter_unpack(data))
    count, period = header[:2]
    flow = RECURRING_FLOW if recurring else FLOW
    require(count == len(flow) and period >= 4, "header")
    origin = rows[0][0]
    previous_time = origin
    previous_due = 0
    frames = {}
    owners = [(0,2,5), (0,3,6), (1,4,7)]
    limits = [period//4, period//2] + [period]*6
    left = limits.copy()
    parent_stops = {}
    for idx, expected in enumerate(flow):
        now, source, prev, charged, arg, nxt, due, mask, restored, saved, old_due, overrun, *debts = rows[idx]
        require((source,prev,nxt,mask) == expected and arg == 0, f"flow {idx}")
        require(now >= previous_time, "clock regressed")
        require(source != 1 or now >= previous_due, "early interrupt")
        if source == 1 and (prev == 3 or not late):
            require(now-previous_due <= period//8, "timer reaction exceeds bench bound")
        elapsed = now-previous_time if prev != 3 else 0
        require(old_due == previous_due, "previous compare")
        require(overrun == (max(0, now-old_due) if prev != 3 else 0), "lost overrun")
        require(charged == elapsed, "elapsed service")
        # Slow independent reference, bounded to eight periods per observed handoff.
        # Debit only the actual owner, split exactly at original grid boundaries.
        require(now - previous_time <= 8*period, "workload response bound")
        cursor = previous_time
        path = owners[prev] if prev != 3 else ()
        while cursor < now:
            boundary = origin + ((cursor-origin)//period + 1)*period
            end = min(now, boundary)
            for node in path:
                left[node] -= end - cursor
            if end == boundary:
                left = [min(limit, bal+limit) for limit,bal in zip(limits,left)]
            cursor = end
        require(debts == ([max(0,-left[n]) for n in path] if path else [0]*3), "level debt")
        window = (now-origin)//period
        boundary = origin + (window+1)*period
        eligible = [i for i,path in enumerate(owners) if mask & (1<<i) and all(left[n] > 0 for n in path)]
        require(nxt == (eligible[0] if eligible else 3), "eligible path")
        end = min(boundary, now+min(left[n] for n in owners[nxt])) if nxt != 3 else (boundary if mask else 0)
        if nxt == 3 and mask:
            for wait in range(1,9):
                if any(mask & (1<<i) and all(left[n]+wait*limits[n] > 0 for n in path)
                       for i,path in enumerate(owners)):
                    end = boundary + (wait-1)*period
                    break
            else:
                raise ValueError("bounded recovery")
        require(due == end, "absolute compare")
        if source == 1 and prev == 0:
            require(left[0] <= 0 and all(left[n] > 0 for n in (2,3,5,6)), "ancestor exhaustion")
            parent_stops[window] = debts[0]
            if late:
                require(period <= now-origin < 2*period and now-previous_time >= period
                        and debts[0] >= period//2, "controlled delayed IRQ")
        if idx:
            require(saved and saved % 16 == 0, "incoming frame")
            frames[prev] = saved
        require(restored and restored % 16 == 0 and restored == frames.get(nxt,restored), "restore")
        previous_time, previous_due = now, due
    if recurring:
        require(len(parent_stops) == 3 and sum(debt > 0 for debt in parent_stops.values()) >= 2,
                "three distinct exhaustion windows and at least two positive parent debts")


def fixture(late):
    if late:
        times = [100,102,105,225,226,600,601,602]
        ends = [125,125,125,275,600,625,625,0]
        charges = [0,2,3,120,1,0,1,1]
        over, debt = 100, 75
    else:
        times = [100,102,105,128,129,200,201,202]
        ends = [125,125,125,178,200,222,222,0]
        charges = [0,2,3,23,1,0,1,1]
        over, debt = 3, 3
    rows = [(len(FLOW),100,*([0]*13))]
    for i,(src,prev,nxt,mask) in enumerate(FLOW):
        rows.append((times[i],src,prev,charges[i],0,nxt,ends[i],mask,16*(nxt+1),
                     16*(prev+1) if i else 0, ends[i-1] if i else 0,
                     over if i==3 else 0, debt if i==3 else 0, 0, 0))
    return b"".join(ROW.pack(*r) for r in rows).ljust(65*ROW.size,b"\0")


def recurring_fixture():
    times = [100,102,105,128,180,200,225,275,300,325,326,400,401,402]
    ends = [125,125,125,178,200,222,273,300,322,373,400,422,422,0]
    debts = [0,0,0,3,2,0,3,2,0,3,0,0,0,0]
    rows = [(len(RECURRING_FLOW), 100, *([0] * 13))]
    for i, (source, prev, nxt, mask) in enumerate(RECURRING_FLOW):
        charged = times[i] - times[i-1] if prev != 3 else 0
        rows.append((times[i], source, prev, charged, 0, nxt, ends[i], mask, 16*(nxt+1),
                     16*(prev+1) if i else 0, ends[i-1] if i else 0, debts[i], debts[i], 0, 0))
    return b"".join(ROW.pack(*r) for r in rows).ljust(65*ROW.size, b"\0")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--late", action="store_true", help="require crossed-window delayed IRQ")
    mode.add_argument("--recurring", action="store_true", help="require three real parent recoveries")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--audit", type=Path)
    group.add_argument("--fixture", action="store_true")
    args = parser.parse_args()
    if args.fixture:
        repeated = recurring_fixture()
        replay(repeated, recurring=True)
        for row, column in [(7, 3), (8, 12), (9, 6), (10, 5)]:
            bad = bytearray(repeated)
            bad[ROW.size*row + 8*column] ^= 1
            try:
                replay(bad, recurring=True)
            except ValueError:
                continue
            raise ValueError("accepted corrupted recurring row")
        for late in (False, True):
            good = fixture(late)
            replay(good, late)
            for row,column in [(2,3),(4,5),(5,6),(6,8),(4,10),(4,11),(4,12),(4,13),(4,14)]:
                bad = bytearray(good)
                bad[ROW.size*row + 8*column] ^= 1
                try:
                    replay(bad, late)
                except ValueError:
                    continue
                raise ValueError("accepted corrupted synthetic row")
        print("synthetic normal, late and recurring fixtures accepted with rejection controls")
    else:
        replay(args.audit.read_bytes(), args.late, args.recurring)
        print("raw group journal accepted")
