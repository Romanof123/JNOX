"""Mutation controls for a retained engineering capture; input files stay read-only."""
import argparse
from pathlib import Path

from pi4_fixed_witness import decode,evidence,fold,ranges,score,symbol_map


def controls(uart,elf,profile,image_id):
    data=uart.read_bytes()
    score(data,profile,image_id,elf)
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
