#!/usr/bin/env python3
"""Freeze one physical case and record each of two separately witnessed cold power cycles.

freeze CASE NEW_DIR MOUNTED_BOOT ELF DISASSEMBLY --source SHA --board REV --card ID --eeprom REV
run FROZEN_DIR TRUSTED_FREEZE_SHA ORDINAL MOUNTED_BOOT PORT --witness NAME
The mounted card is read back before arming. Power-cycle the same board once
after ARMED. A witness name/USB log cannot establish physical instruction history.
"""
import argparse
import json
from pathlib import Path
import sys
import time
import shutil

from pi4_migration_capture import CASES, FILES, save, sha
from pi4_serial_capture import artifacts, serial_port


def card(directory, elf):
    record = artifacts(directory, elf)
    return {k:record[k] for k in ("files_sha256","firmware_commit","package_manifest_sha256","elf_sha256")}


def main():
    parser=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter)
    sub=parser.add_subparsers(dest="mode",required=True)
    f=sub.add_parser("freeze")
    f.add_argument("case",choices=CASES)
    for key in ("root","mounted_boot","elf","disassembly"):
        f.add_argument(key,type=Path)
    for key in ("source","board","card","eeprom"):
        f.add_argument("--"+key,required=True)
    a=sub.add_parser("run")
    a.add_argument("root",type=Path)
    a.add_argument("digest")
    a.add_argument("ordinal",type=int,choices=(1,2))
    a.add_argument("mounted_boot",type=Path)
    a.add_argument("port")
    a.add_argument("--witness",required=True)
    args=parser.parse_args()
    root=args.root
    if args.mode=="freeze":
        root.mkdir()
        readback=card(args.mounted_boot,args.elf)
        for name,src in zip(FILES,(args.elf,args.mounted_boot/"kernel8.img",args.disassembly)):
            shutil.copyfile(src,root/name)
        save(root/"freeze.json",dict(platform="physical",case=args.case,source=args.source,positions=[1,2],
             board=args.board,card=args.card,eeprom=args.eeprom,card_readback=readback,
             hashes={name:sha(root/name) for name in FILES}))
        print(sha(root/"freeze.json"))
        return 0
    if sha(root/"freeze.json")!=args.digest or not args.witness.strip():
        parser.error("freeze digest/witness mismatch")
    frozen=json.loads((root/"freeze.json").read_bytes())
    if frozen["platform"]!="physical" or args.ordinal==2 and not (root/"first-1/session.json").is_file():
        parser.error("invalid platform or missing first attempt")
    folder=root/f"first-{args.ordinal}"
    folder.mkdir()
    data=dict(ordinal=args.ordinal,case=frozen["case"],platform="physical",freeze_sha256=args.digest,
              cycle_witness=args.witness,adapter=args.port,wall_start_ns=time.time_ns())
    save(folder/"attempt.json",data)
    start=last=time.monotonic_ns()
    reason="deadline";terminal=None
    try:
        data["card_readback"]=card(args.mounted_boot,root/"kernel.elf")
        if data["card_readback"]!=frozen["card_readback"]:
            raise ValueError("mounted-card readback changed")
        with serial_port(args.port) as port,(folder/"uart.bin").open("xb") as out,(folder/"reads.jsonl").open("x") as reads:
            print("ARMED: witness one full power cycle now",flush=True)
            start=last=time.monotonic_ns()
            partial=b""
            while time.monotonic_ns()-start<30_000_000_000:
                block=port.read(4096)
                if block:
                    last=time.monotonic_ns()
                    reads.write(json.dumps(dict(elapsed_ns=last-start,offset=out.tell(),length=len(block)))+"\n")
                    out.write(block);out.flush()
                    lines=(partial+block).split(b"\n");partial=lines.pop()
                    if len(partial)>8192 or out.tell()>8*1024*1024:
                        raise ValueError("UART evidence exceeded capacity")
                    for line in lines:
                        if line.startswith((b"MG END ",b"MG FAIL ",b"PI4 MIG FAIL",b"PI4 SMP BSP FAIL")):
                            terminal=line.decode("ascii","replace")
                elif terminal and time.monotonic_ns()-last>=1_000_000_000:
                    reason="terminal_quiet";break
    except (ValueError,OSError) as exc:
        reason=str(exc)
    finally:
        end=time.monotonic_ns()
        data.update(reason=reason,terminal=terminal,elapsed_ns=end-start,quiet_ns=end-last,
                    wall_end_ns=time.time_ns(),hashes={f.name:sha(f) for f in sorted(folder.iterdir()) if f.is_file()})
        save(folder/"session.json",data)
        print(sha(folder/"session.json"))
    return int(reason!="terminal_quiet")


if __name__=="__main__":
    sys.exit(main())
