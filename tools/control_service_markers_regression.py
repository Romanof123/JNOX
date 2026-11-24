"""Replay one real healthy marker boot, then check mutations on temporary copies."""
import argparse
from pathlib import Path
import struct
import tempfile

from control_service_markers import inspect


def controls(journal, uart, exceptions):
    result = inspect(journal, uart, exceptions)
    rows = result["markers"]
    phases = result["phases"]
    assert [p["phase"] for p in phases] == list(range(26))
    decisions = [r for r in rows if r["kind"] == 8]
    for r,p in zip(decisions, phases, strict=True):
        assert (r["t"],r["a"],r["b"]) == (p["decision_start"],p["decision_end"],p["phase"])
        sliced, = [s for s in result["slices"] if s.get("phase")==p["phase"]]
        assert sum(sliced["disjoint_ticks"].values()) == sliced["end"]-sliced["start"]
        assert sliced["phase_class"] == ("first" if p["phase"]==0 else
                                           "same_job_continuation" if p["phase"]%2 else "later_job_first")
        assert p["check_ticks"] == r["a"]-r["t"]
        assert p["pre_svc_to_vector"] == p["svc_entry"]-p["pre_svc"] >= 0

    def reject(j,u,e):
        try:
            inspect(j,u,e)
        except ValueError:
            return
        raise AssertionError("accepted corrupt capture")

    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        j,u,e = (directory / name for name in ("journal","uart","exceptions"))
        original = journal.read_bytes()
        first, second = decisions[:2]
        def mutated(changes):
            blob = bytearray(original)
            for row,field,value in changes:
                struct.pack_into("<Q",blob,24+56*row["n"]+8*field,value)
            j.write_bytes(blob)

        # A forged phase can look like a counter tick and agree with a forged UART row.
        mutated([(first,3,first["a"])])
        lines = uart.read_text().splitlines()
        i = next(i for i,x in enumerate(lines) if x.startswith("21 "))
        fields = lines[i].split()
        fields[2] = str(first["a"])
        lines[i] = " ".join(fields)
        u.write_text("\n".join(lines)+"\n")
        reject(j,u,exceptions)

        # Reversing a nonzero check interval in both inputs must still fail.
        mutated([(first,1,first["a"]),(first,2,first["t"])])
        lines[i] = f'21 {first["t"]} 0 {first["a"]}'
        u.write_text("\n".join(lines)+"\n")
        reject(j,u,exceptions)
        for changes in ([(first,0,5)], [(second,3,0)], [(first,1,2**64-1)]):
            mutated(changes)
            reject(j,uart,exceptions)
        lines = uart.read_text().splitlines()
        i = next(i for i,s in enumerate(lines) if s.startswith("8 "))
        u.write_text("\n".join(lines[:i]+lines[i+1:])+"\n")
        reject(journal,u,exceptions)

        lines = exceptions.read_text().splitlines()
        positions = [i for i,s in enumerate(lines) if s.startswith("Exception return")]
        assert len(positions) == 1+sum(r["kind"]==4 for r in rows)
        for i in (positions[0],positions[-1]):
            bad = lines.copy()
            bad[i] = bad[i].rsplit(" ",1)[0]+" 0x0"
            e.write_text("\n".join(bad)+"\n")
            reject(journal,uart,e)
        e.write_text("\n".join(lines[:positions[-1]])+"\n")
        reject(journal,uart,e)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("journal","uart","exceptions"):
        parser.add_argument(name,type=Path)
    args = parser.parse_args()
    controls(args.journal,args.uart,args.exceptions)
