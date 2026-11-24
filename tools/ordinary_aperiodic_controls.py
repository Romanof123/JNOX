"""Focused controls. Optional complete and injected UARTs exercise mutation paths."""
import argparse
import copy
from pathlib import Path
from ordinary_aperiodic_check import Invalid, replay, rows_from_uart


def must_reject(case, rows, trace=None):
    try:
        replay(case, rows, trace)
    except Invalid:
        return
    raise RuntimeError("mutated evidence was accepted")


def first_boundary():
    base, epoch, stride, freq = 0x80000, 10000, 16384, 1000
    frame = base + stride - 832
    return {"EVENT": [[0,0,100,freq,epoch,50,base,stride,2,frame],
                      [1,0,epoch+2,epoch,0,frame,0x1004,0,0,0]],
            "STEP": [], "RETURN": [[frame,0x1000,200]], "MASK":[[0,0]], "END":[[1]]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--complete",type=Path,help="complete positive UART")
    parser.add_argument("--case",default="a-ss2")
    parser.add_argument("--injected",type=Path,help="same case, late ELF UART")
    opts = parser.parse_args()
    for name in ("a-ds", "a-ds-late"):
        rows = first_boundary()
        result = replay(name, rows)
        if result["outcome"]!="inhibited":
            raise RuntimeError("one-tick-late first boundary should inhibit")
        # Exactly at the one-tick limit this IRQ alone cannot justify END 1.
        rows["EVENT"][1][2] -= 1
        must_reject(name, rows)
    rows = first_boundary()
    must_reject("a-ds", rows, "")
    rows["END"] = [[0]]
    must_reject("a-ds", rows)
    if opts.complete:
        rows = rows_from_uart(opts.complete.read_text(encoding="ascii"))
        if replay(opts.case,rows)["outcome"] != "completed":
            raise RuntimeError("positive fixture must complete")
        for kind, field in ((1,4),(2,6),(3,3),(3,5),(3,7),(4,4),(5,3),(6,9)):
            altered=copy.deepcopy(rows)
            event=next(row for row in altered["EVENT"] if row[0]==kind)
            event[field]^=1
            must_reject(opts.case,altered)
        if "ss" in opts.case:
            altered=copy.deepcopy(rows)
            next(row for row in altered["EVENT"] if row[0]==8)[3]+=1
            must_reject(opts.case,altered)
        for tag in ("STEP","RETURN","EVENT"):
            altered=copy.deepcopy(rows)
            altered[tag].pop()
            must_reject(opts.case,altered)
    if opts.injected:
        rows=rows_from_uart(opts.injected.read_text(encoding="ascii"))
        if replay(opts.case+"-late",rows)["outcome"] != "prescribed-late-exercised":
            raise RuntimeError("negative fixture must exercise mask")
        rows["MASK"]=[[0,0]]
        result=replay(opts.case+"-late",rows)
        if result["prescribed_fault"] != "unexercised":
            raise RuntimeError("missing mask cannot prove injected fault")
    print("Replay controls passed")


if __name__ == "__main__":
    main()
