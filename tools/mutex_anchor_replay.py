"""Check the single CHG-072 timer witness in an existing UART file (offline)."""
import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import re

NA = (1 << 64) - 1
MAX_CAPTURE = 96 * 1024
WIDTH = {"H": 5, "T": 8, "W": 13, "P": 6, "S": 5, "E": 26,
         "R": 3, "C": 2, "B": 3, "HIGH": 3, "A": 2, "M": 3, "K": 9}
MODES = {"rust-prepared": 1, "rust-stages": 2, "selected-return": 3}
APPS = {"ordinary-pi": 0, "ordinary-ceiling": 1}


class Invalid(ValueError):
    pass


def require(condition, reason):
    if not condition:
        raise Invalid(reason)


@dataclass(frozen=True)
class Witness:
    version: int
    mode: int
    phase: int
    valid: int
    generation: int
    ordinal: int
    frame: int
    pc: int
    spsr: int
    tls: int
    x0: int
    resource: int
    ticks: int


@dataclass(frozen=True)
class Samples:
    write_end: int
    read_end: int
    restore_entry: int
    restore_sample: int
    before_eret: int
    elapsed: int


def parse(raw):
    require(len(raw) <= MAX_CAPTURE, "capture size")
    data = raw.replace(b"\r\n", b"\n")
    require(data.endswith(b"\n") and b"\r" not in data, "line endings")
    lines = data[:-1].split(b"\n")
    require(lines[:3] == [b"JNOX M1 BOOT", b"JNOX M1 READY", b"MUTEX RAW 2"]
            and lines[-1] == b"MUTEX END", "capture framing")
    rows = {tag: [] for tag in WIDTH}
    order = []
    for line in lines[3:-1]:
        key, *words = line.split(b" ")
        try:
            tag = key.decode("ascii")
        except UnicodeError as exc:
            raise Invalid("non-ASCII tag") from exc
        require(tag in WIDTH and len(words) == WIDTH[tag], "wire row shape")
        require(all(re.fullmatch(rb"[0-9a-f]{16}", w) for w in words), "wire word")
        order.append(tag)
        rows[tag].append([int(w, 16) for w in words])
    require(len(rows["H"]) == 1, "header count")
    ce, reason, ne, nr, nc = rows["H"][0]
    require(ce in (0, 1) and reason <= 22 and ne <= 32 and nr <= 64 and nc <= 256,
            "ledger bounds")
    require(order == ["H", "T", "W", "P"] + ["S"] * 4 + ["E"] * ne + ["R"] * nr
            + ["C"] * nc + ["B", "HIGH", "A", "M", "K"], "ledger row order/count")
    return rows


def check_anchor(rows, app, mode):
    ce, reason, ne, nr, nc = rows["H"][0]
    w = Witness(*rows["W"][0])
    p = Samples(*rows["P"][0])
    require(ce == APPS[app] and w.version == 1 and w.mode == MODES[mode],
            "wrong protocol or anchor")
    # Report cold failures without translating a missing stage into a zero-tick interval.
    if reason:
        return {"anchor_verified": False, "guest_reason": reason, "phase": w.phase,
                "valid_bits": w.valid, "decisions": ne, "restores": nr, "completed_chunks": nc}
    required = {1: 271, 2: 383, 3: 399}[w.mode]
    require(w.phase == 4 and w.valid == required, "incomplete or extra stages")
    frequency, start, target, rb, ctl, iar, ictl, icv = rows["T"][0]
    require(frequency > 0 and frequency % 10000 == 0 and w.ticks == frequency // 10000,
            "clock/interval")
    require(start + w.ticks == target == rb == icv and ctl & 3 == 1
            and iar & 1023 == 30 and ictl & 7 == 5, "timer values")
    require(1 <= w.generation <= ne and w.ordinal == w.generation + 1 <= nr
            and w.resource == ce and w.tls == 1 and w.x0 == 0
            and w.spsr & 0x8f == 5 and w.pc != 0 and w.pc % 4 == 0, "descriptor")
    matches = [idx for idx, e in enumerate(rows["E"], 1)
               if e[2:6] == [1, ce, 0, 0] and e[19] == 0]
    require(matches == [w.generation], "missing or reused final acquire")
    e = rows["E"][w.generation - 1]
    require(e[6:9] == [w.frame,w.pc,w.spsr] and e[10:12] == [w.frame,w.pc]
            and e[14] == e[14 + ce] == 0 and e[23:26] == [1,1,0], "acquire identity/owner")
    tid, _, bottom, top, size = rows["S"][0]
    require(tid == 0 and size == 832 and w.frame % 16 == 0 and bottom <= w.frame <= top-size,
            "selected LOW stack")
    ret = rows["R"][w.ordinal - 1]
    require(ret == [w.frame,w.pc,p.restore_sample] and e[0] <= e[1] <= ret[2],
            "restore tie")
    if w.mode < 3:
        require(e[1] <= start <= ret[2] and p.before_eret == 0 and p.elapsed == NA,
                "Rust anchor chronology")
    if w.mode != 2:
        require(p.write_end == p.read_end == p.restore_entry == 0, "unexpected stage stamps")
    else:
        require(start <= p.write_end <= p.read_end <= p.restore_entry <= ret[2],
                "characterization chronology")
    irqs = [event for event in rows["E"] if event[2] == 5]
    require(len(irqs) == 1, "IRQ count")
    irq = irqs[0]
    require(irq[0] >= max(target, ret[2]) and irq[0] <= irq[1]
            and irq[4] == 0 and irq[21] == 1 and irq[14+ce] == 0
            and (not ce or irq[20] < 128), "IRQ release")
    if w.mode == 3:
        require(ret[2] <= start <= p.before_eret <= irq[0]
                and p.elapsed == int(p.before_eret >= target), "selected-return chronology")
    require(rows["C"] == [[256*k, 2176*k] for k in range(1,257)]
            and rows["B"] == rows["HIGH"] == [[1,65536,557056]]
            and rows["A"] == [[ce,ce*256]] and rows["M"] == [[ce,ce*256,32896]],
            "ordinary workload data")
    return {"anchor_verified": True, "guest_reason": 0, "anchor": mode,
            "irq_saved_pc": hex(irq[7]), "completed_chunks_at_irq": irq[20],
            "arm_to_irq_entry_ticks": irq[0]-start, "irq_arm_istatus": (ctl >> 2) & 1,
            "elapsed_at_final_sample": None if p.elapsed == NA else bool(p.elapsed),
            "final_sample_to_vector_entry_ticks": irq[0]-p.before_eret if w.mode == 3 else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart", type=Path)
    parser.add_argument("--app", choices=APPS, required=True)
    parser.add_argument("--anchor", choices=MODES, required=True)
    args = parser.parse_args()
    with args.uart.open("rb") as stream:
        raw = stream.read(MAX_CAPTURE + 1)
    try:
        result = check_anchor(parse(raw), args.app, args.anchor)
    except Invalid as exc:
        parser.exit(2, f"Invalid capture: {exc}\n")
    print(json.dumps(result, sort_keys=True))
    return 0 if result["anchor_verified"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
