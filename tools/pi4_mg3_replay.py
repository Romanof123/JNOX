"""Verify MG3 register witnesses against the running activation and real checkpoint prefix."""
from functools import lru_cache

from pi4_smp_replay import need

ACTIVATIONS = {
    (0, 0): ("M", 0x10601),
    (1, 0): ("M", 0x10601),
    (1, 1): ("P0", 0x10602),
    (0, 2): ("P1_OPEN", 0x10603),
    (1, 2): ("P1_REPLACEMENT", 0x10604),
}


@lru_cache(maxsize=8)
def prefixes(task, epoch, stream):
    from pi4_migration_replay import stream_reference
    if stream and task == 0:
        values = stream_reference()[0].items()
    else:
        start = 16 if (epoch, task) == (1, 2) else 0
        stop = 65536 if task == 0 else 16 if (epoch, task) == (0, 2) else 32
        initial = start * (start + 1) // 2
        values = ((i, i * (i + 1) // 2 - initial) for i in range(start + 16, stop + 1, 16))
    result = {}
    h = 0
    mask = (1 << 64) - 1
    for i, value in values:
        h = ((((h << 7 | h >> 57) & mask) ^ i) + value) & mask
        result[i] = h
    return result


def witnesses(sections, symbols, case, stream, handoff=False):
    saved, live = {}, {}
    expected_cpu = {(0, 0): 0, (0, 2): 1, (1, 1): 0,
                    (1, 0): 0 if case else 1, (1, 2): 1}
    for cpu, rows in sections.items():
        base = symbols[f"PI4_MIG_CPU{cpu}"]["address"]
        current = None
        trapped = False
        for seq, (kind, d) in enumerate(rows):
            if kind == 11:
                need(current is None, "dispatch before local retirement")
                ep, task, lease = d[:3]
                if task == 3:
                    need(lease == 0, "idle lease")
                    continue
                key = ep, task
                need(expected_cpu.get(key) == cpu, "unexpected activation placement")
                name, token = ACTIVATIONS[key]
                ptr = symbols["PI4_MIG_TLS_" + name]["address"]
                current = ep, task, lease
                trapped = False
            elif kind == 4:
                if d[1] == 3:
                    need(current is None and d[6] != 65535, "false idle trap")
                    continue
                need(current is not None and current[:2] == d[:2] and not trapped,
                     "trap outside dispatched activation")
                trapped = True
                interrupted = current[:2] == (0, 0)
                need((d[6] != 65535) == interrupted, "trap cause differs from activation")
                expected = (task, token, lease, d[2], ptr, token, base, lease)
                need(rows[seq+1:seq+2] == [(14, expected)], "missing or incorrect saved-state witness")
                follow = ([17, 3] if handoff else [3]) if interrupted else [13, 9, 3]
                need([k for k, _ in rows[seq+2:seq+2+len(follow)]] == follow,
                     "saved-state trap/summary/retirement chronology")
                need(lease not in saved, "duplicate saved witness")
                saved[lease] = seq + 1
            elif kind == 5:
                need(current is not None and not trapped and (task, lease) == (d[0], d[2]),
                     "checkpoint outside executing activation")
                if lease not in live:
                    index = d[3]
                    digest = prefixes(task, ep, stream).get(index)
                    need(digest is not None, "unknown activation prefix")
                    expected = (task, token, lease, base, ptr, token, index, digest)
                    need(rows[seq+1:seq+2] == [(15, expected)], "missing or incorrect live TLS prefix")
                    live[lease] = (cpu, seq + 1, index, digest)
            elif kind == 14:
                need(current is not None and saved.get(lease) == seq, "orphan saved-state row")
            elif kind == 15:
                need(current is not None and lease in live and live[lease][:2] == (cpu, seq),
                     "orphan live-state row")
            elif kind == 3:
                need(current == d[:3] and trapped and lease in live, "retirement without owned witnesses")
                need(d[7] == int((ep, task) == (0, 0)), "wrong retirement disposition")
                current = None
            elif kind == 13:
                need(current is not None and trapped and (task, ep, lease) == d[:3],
                     "summary does not belong to trapped worker")
        need(current is None, "unclosed local execution")
    need(len(saved) == len(live) == 5 and saved.keys() == live.keys(), "incomplete MG3 lease witnesses")
    return [{"lease": lease, "cpu": c, "sequence": n, "index": i, "digest": h}
            for lease, (c, n, i, h) in sorted(live.items())]
