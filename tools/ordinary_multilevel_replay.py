#!/usr/bin/env python3
"""Replay an ordinary MLQ/MLFQ journal without kernel policy code."""
import argparse
from pathlib import Path
import struct


def require(ok, message):
    if not ok:
        raise ValueError(message)


def replay(data, feedback):
    words = 27 + 104  # header, then complete integer/system/SIMD frame
    count, hz = struct.unpack_from("<QQ", data)
    require(len(data) == 16 + 8 * words * 256 and 1 < count <= 256 and hz >= 100, "header")
    unit = hz // 100
    budgets = [4 * unit, 6 * unit, 8 * unit]
    period = 16 * unit
    levels = [0] * 4
    left = [budgets[0] if feedback else unit] * 4
    service = [0] * 4
    queue = [0, 1, 2]
    live = set(queue)
    blocked = set()
    owner = 4
    total = last_stop = 0
    events = []
    saved = {}
    boost_dispatch = False
    carried_yield = carried_block = False
    uninterrupted = set()
    for index in range(count):
        row = struct.unpack_from(f"<{words}Q", data, 16 + 8 * words * index)
        kind, arg, result, outgoing, start, due, stop, boosted, fp, source, selected, packed, reported, next_fp, next_pc = row[:15]
        require(outgoing == owner, f"row {index}: outgoing")
        boost = False
        if index:
            grant = min(left[owner], period - total % period) if feedback else unit
            require(start >= last_stop and due == start + grant and stop >= start, "counter interval")
            last_stop = stop
            require(fp and fp % 16 == 0 and row[27] == arg and row[58] and row[59] & 0x8f == 5, "entry frame")
            saved[owner] = (fp, row[58])
            elapsed = stop - start
            service[owner] += elapsed
            if feedback:
                if owner == 0 and elapsed and not boost:
                    if kind == 0 and elapsed < left[0]:
                        uninterrupted.add("yield")
                    if kind == 2 and elapsed < left[0] and "yield" in uninterrupted:
                        uninterrupted.add("block")
                    if levels[0] == 0 and elapsed >= left[0]:
                        carried_yield |= "yield" in uninterrupted
                        carried_block |= "block" in uninterrupted
                charge = elapsed
                while charge >= left[owner]:
                    charge -= left[owner]
                    levels[owner] = min(2, levels[owner] + 1)
                    left[owner] = budgets[levels[owner]]
                    if levels[owner] == 2:
                        charge %= left[owner]
                left[owner] -= charge
                boost = (total + elapsed) // period != total // period
            total += elapsed
            if kind == 4:
                require(source & 1023 == 30 and source >> 32 & 7 == 5 and stop >= due, "IRQ")
            else:
                require(kind <= 3 and source >> 26 == 0x15 and source & 65535 == kind, "SVC")
            if kind == 3 and arg in blocked:
                blocked.remove(arg)
                queue.append(arg)
                require(result == 1, "wake result")
            else:
                require(result == 0, "spurious success")
            if kind == 1:
                live.remove(owner)
            elif kind == 2:
                blocked.add(owner)
            else:
                queue.append(owner)
            if boost:
                uninterrupted.clear()
                levels = [0] * 4
                left = [budgets[0]] * 4
            events.append((owner, kind, result))
        else:
            require(not any((start, due, stop, fp, source)), "initial event")
        ranks = [2-x for x in levels] if feedback else [0, 2, 2, 0]
        expected = max(queue, key=ranks.__getitem__, default=4)
        code = 0
        for task in queue:
            code = 16 * code + task + 1
        require((boosted, packed, selected, reported) == (boost, code, expected, total), f"row {index}: decision")
        require(list(row[15:27]) == ranks + left + service, f"row {index}: accounting")
        if queue:
            require(next_pc and next_fp % 16 == 0 and next_fp, "selected frame")
            if selected in saved:
                require((next_fp, next_pc) == saved[selected], "resume frame")
            if boost and 2 in queue and len(queue) > 1:
                boost_dispatch = True
            queue.remove(selected)
        else:
            require(index == count - 1 and not live and not next_fp and not next_pc, "shutdown")
        owner = selected
    require(owner == 4 and all(service[:3]), "completion")
    if feedback:
        require(carried_yield and carried_block and boost_dispatch and (0, 2, 0) in events and (2, 3, 1) in events
                and (0, 0, 0) in events and (1, 4, 0) in events, "feedback workload")
    else:
        first_low = next(i for i, event in enumerate(events) if event[0] == 0)
        require(all((peer, 1, 0) in events[:first_low] and (peer, 4, 0) in events[:first_low]
                    for peer in (1, 2)), "strict saturation")
    return count


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", choices=("mlq", "mlfq"))
    parser.add_argument("journal", type=Path)
    args = parser.parse_args()
    print(f"{replay(args.journal.read_bytes(), args.profile == 'mlfq')} decisions replayed")
