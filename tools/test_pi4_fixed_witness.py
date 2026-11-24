"""Mutation controls for a retained engineering capture; input files stay read-only."""
import argparse
import hashlib
from pathlib import Path

from pi4_fixed_witness import decode,evidence,fold,ranges,score,symbol_map


def controls(uart,elf,profile,image_id):
    data=uart.read_bytes()
    result = score(data,profile,image_id,elf)
    original=decode(data)
    exe=ranges(elf.read_bytes())
    syms=symbol_map(elf,profile)
    def reject(fn):
        try:
            fn()
        except (ValueError,IndexError):
            return
        raise AssertionError("corruption was accepted")
    def serialize(records):
        h=0xcbf29ce484222325
        lines=[]
        for k,r in records:
            for v in [k,*r]:
                h=fold(h,v)
            lines.append("PW "+" ".join(map(str,[k,*r])))
        lines.append(f"PW END {len(records)} {h}")
        return ("\n".join(lines)+"\n").encode()
    def mutated(kind,index,value):
        rows=[(k,r.copy()) for k,r in original]
        r=next(r for k,r in rows if k==kind)
        r[index]=value
        return rows
    for rows in (mutated(0,1,image_id^1),mutated(0,11,1),mutated(3,2,3),
                 mutated(3,0,0),mutated(3,4,0),original[:-1],original+[original[-1]]):
        reject(lambda:evidence(decode(serialize(rows)),profile,image_id,exe,syms))
    # Recompute the footer for each mutation so rejection is semantic.
    def inspect(records):
        return evidence(decode(serialize(records)), profile, image_id, exe, syms)

    def dispatch_mutation(generation, field, value):
        rows = [(tag, fields.copy()) for tag, fields in original]
        dispatch = next(fields for tag, fields in rows if tag == 2 and fields[0] == generation)
        dispatch[field] = value
        return rows

    terminal = original[-1][1]
    generation, entry = terminal[0], terminal[5]
    final_dispatch = next(fields for tag, fields in original if tag == 2 and fields[0] == generation)
    assert final_dispatch[9] != entry and final_dispatch[10] == entry
    wrong_target = syms["fixed_runtime_enter"][0]
    reject(lambda: inspect(dispatch_mutation(generation, 9, entry)))
    reject(lambda: inspect(dispatch_mutation(generation, 10, wrong_target)))
    reject(lambda: inspect(mutated(4, 5, final_dispatch[9])))
    forged = dispatch_mutation(generation, 10, wrong_target)
    forged[-1][1][5] = wrong_target
    reject(lambda: inspect(forged))

    # Independent expected continuations in the retained first model inputs.
    # Other genuine captures can legitimately lack a post-HIGH work sample.
    first_resume = {
        "3c0977fc5875d5bc35852ba79dd499077795a2356ef73975d3dee280bc2c09bc": 5,
        "eeccb78bea27a54126b8abc0ff0899a1caaf24d2f6359b60f5ddf0b4da5c9c8d": 3,
    }.get(hashlib.sha256(data).hexdigest())
    if first_resume is not None:
        assert result["higher_priority_preempt_resume"] and result["complete"]
        reject(lambda: inspect(dispatch_mutation(first_resume, 10, wrong_target)))
        missing = [(tag, fields) for tag, fields in original
                   if not (tag == 3 and fields[:2] == [first_resume, 1])]
        partial = inspect(missing)
        assert not partial["higher_priority_preempt_resume"] and not partial["complete"]

    blob=serialize(original)
    reject(lambda:decode(blob[:-1]))
    reject(lambda:decode(blob.replace(b"PW 0 1 ",b"PW 0 2 ",1)))


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart",type=Path)
    parser.add_argument("elf",type=Path)
    parser.add_argument("profile",choices=("pi4-fixed-fifo-witness","pi4-fixed-rr-witness"))
    parser.add_argument("image_id",type=int)
    args=parser.parse_args()
    controls(args.uart,args.elf,args.profile,args.image_id)
