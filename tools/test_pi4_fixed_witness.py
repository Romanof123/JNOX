"""Mutation controls for a retained engineering capture; input files stay read-only."""
import argparse
import hashlib
from pathlib import Path

from pi4_fixed_witness import HANDOFF_PROFILES, INNER_HANDOFF, PROFILES, inner_site, decode,evidence,fold,ranges,score,symbol_map


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

    if profile in HANDOFF_PROFILES:
        # Re-seal malformed rows so each reaches semantic validation.
        cells = [r for k,r in original if k == 5]
        reject(lambda:inspect(original[:-2]+[(5,cells[0])]+original[-2:]))
        reject(lambda:inspect(mutated(5,0,1)))
        reject(lambda:inspect(mutated(5,1,3)))
        for index,cell in enumerate(cells):
            if cell[1] != 1:
                continue
            for field,value in ((2,cell[3]+1),(6,0),(8,0),(9,0),(11,1-index)):
                forged = [(k,r.copy()) for k,r in original]
                row = [r for k,r in forged if k == 5][index]
                row[field] = value
                reject(lambda:inspect(forged))
        if all(r[1] for r in cells):
            reject(lambda:inspect(mutated(5,7,cells[1][7]^1)))

    if profile == INNER_HANDOFF:
        inner = [r for k,r in original if k == 6]
        reject(lambda:inspect([(k,r) for k,r in original if k != 6]))
        reject(lambda:inspect(mutated(6,0,1)))
        reject(lambda:inspect(mutated(6,1,3)))
        for role,cell in enumerate(inner):
            def inner_mutation(field, value):
                rows = [(k,r.copy()) for k,r in original]
                [r for k,r in rows if k == 6][role][field] = value
                return rows
            reject(lambda:inspect(inner_mutation(4, cell[4]+4)))
            reject(lambda:inspect(inner_mutation(5,cell[5]^8)))
            if cell[1]:
                reject(lambda:inspect(inner_mutation(3,cells[role][3]+1)))
                reject(lambda:inspect(inner_mutation(1,3-cell[1])))
        assert result["complete"] == (result["higher_priority_preempt_resume"]
                                       and result["inner_rr_data_handoff"])
        assert result["outer_complete"] == (result["higher_priority_preempt_resume"]
                                             and result["outer_rr_data_handoff"])
        # Mutate linked instruction operands independently of UART integrity.
        # Synthetic addresses here exercise the instruction predicate only.
        pc = 0x1000
        gen = pc-20
        for opcode in (0xc89ffc00, 0xc8dffc00):
            words = [0xd503201f, 0x10000001, 0xaa0503e8, 0x10000062, 0x910003e3,
                     0xc8dffc24, opcode | (5 << 5) | 6, 0xc8dffc27]
            assert inner_site(words, pc, opcode, gen)
            for index,mask in ((1,32),(2,1 << 16),(3,32),(5,32),(6,0x400000),(7,32)):
                forged = words.copy()
                forged[index] ^= mask
                assert not inner_site(forged, pc, opcode, gen)
            for index,value in ((6,opcode | (1 << 5) | 6),(7,0xc8dffc24),
                                (5,0xc8dffc25),(7,0xc8dffc3f)):
                forged = words.copy()
                forged[index] = value
                assert not inner_site(forged, pc, opcode, gen)

    blob=serialize(original)
    reject(lambda:decode(blob[:-1]))
    reject(lambda:decode(blob.replace(b"PW 0 ",b"PW 9 ",1)))


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("uart",type=Path)
    parser.add_argument("elf",type=Path)
    parser.add_argument("profile",choices=PROFILES)
    parser.add_argument("image_id",type=int)
    args=parser.parse_args()
    controls(args.uart,args.elf,args.profile,args.image_id)
