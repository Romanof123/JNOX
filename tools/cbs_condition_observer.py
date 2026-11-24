#!/usr/bin/env python3
"""Offline basic CBS first-condition reader. Requires pyelftools.

Use qemu_boot.py --profile cbs --runs 1 --capture-dir NEW and, for the
opt-in image only, --cbs-diagnostic. Retain merged runner stdout/stderr.
Supply a separately frozen SHA-256 of receipt.json. Its fields are:
revision, image_kind (plain or diagnostic), runner_exit, pid, and hashes
of kernel.elf (the loaded image), sites.csv, runner.log, uart, exceptions.log.
Copy original files into the receipt directory; retain original argv paths.
"""
import argparse
import ast
import csv
import hashlib
import io
import json
from pathlib import Path
import re
import sys

from qemu_boot import CBS_TRANSCRIPT

MAX = (1 << 64) - 1
HEADER = ["site", "mask", "name", "a", "b", "c", "condition_en", "condition_ko"]
DECIMAL = rb"(?:0|[1-9][0-9]{0,19})"
WIRE = re.compile(rb"CBS DIAG1 " + b" ".join([b"(" + DECIMAL + b")"] * 5))
LIMITS = {"kernel.elf": 32 << 20, "sites.csv": 256 << 10, "exceptions.log": 2 << 20,
          "runner.log": 128 << 10, "uart": 512}


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path, limit):
    with path.open("rb") as source:
        data = source.read(limit + 1)
    require(len(data) <= limit, f"oversize {path.name}")
    return data


def sites(raw):
    rows = csv.DictReader(io.StringIO(raw.decode("utf-8")))
    require(rows.fieldnames == HEADER, "incorrect site dictionary headers")
    result = {}
    for row in rows:
        require(None not in row and all(v is not None for v in row.values()), "invalid dictionary row")
        require(re.fullmatch(DECIMAL, row["site"].encode()) and
                re.fullmatch(rb"[0-7]", row["mask"].encode()), "invalid dictionary site/mask")
        key = int(row["site"])
        require(key <= MAX and key not in result and row["name"] and row["condition_en"]
                and row["condition_ko"], "invalid, duplicate or unnamed site")
        for index,label in enumerate(("a","b","c")):
            require(bool(row[label]) == bool(int(row["mask"]) & (1 << index)),
                    "dictionary operand names disagree with mask")
        result[key] = row
    require(result, "empty dictionary")
    return result


def classify(raw, dictionary, diagnostic):
    require(len(raw) <= LIMITS["uart"], "oversize UART")
    canonical = CBS_TRANSCRIPT.splitlines()
    normalized = raw.replace(b"\r\n", b"\n")
    if normalized == CBS_TRANSCRIPT:
        return {"uart": "ordinary_success", "prefix_lines": len(canonical)}
    lines = normalized.split(b"\n")
    complete = lines.pop()
    prefix = 0
    while prefix < len(lines) and prefix < len(canonical) and lines[prefix] == canonical[prefix]:
        prefix += 1
    if not lines[prefix:] and prefix < len(canonical) and canonical[prefix].startswith(complete):
        return {"uart": "partial", "prefix_lines": prefix}
    require(2 <= prefix < len(canonical), "invalid ordinary marker prefix")
    rest = lines[prefix:]
    if not complete and rest == [b"CBS FAIL"]:
        return {"uart": "ordinary_failure_cause_unobserved", "prefix_lines": prefix}
    if diagnostic and len(rest) <= 1 and (not rest or not complete or b"CBS FAIL".startswith(complete)):
        if (not rest and (b"CBS DIAG1 ".startswith(complete) or complete.startswith(b"CBS DIAG1 "))) or (
                rest and WIRE.fullmatch(rest[0])):
            return {"uart": "partial", "prefix_lines": prefix}
    require(diagnostic and not complete and len(rest) == 2 and rest[1] == b"CBS FAIL",
            "missing, extra or misordered failure records")
    match = WIRE.fullmatch(rest[0])
    require(match and (rest[0]+b"\n") in raw, "invalid diagnostic encoding")
    site, mask, *operands = map(int, match.groups())
    require(site in dictionary and mask == int(dictionary[site]["mask"]) and
            all(value <= MAX and (mask & (1 << i) or value == 0)
                for i,value in enumerate(operands)), "unknown site, invalid mask or operand")
    row = dictionary[site]
    a,b,c = operands
    # Cross-check reported values against the failed predicate, including
    # checked u64 arithmetic. Some sites have no available operands.
    rules = {
        "a <= b": a <= b, "a > b": a > b, "a < b": a < b, "a == b": a == b,
        "a != 0": a != 0, "b != 0": b != 0, "(a & b) != 0": (a & b) != 0,
        "(a & 1023) == b": (a & 1023) == b, "(a & 7) == b": (a & 7) == b,
        "(a & 143) == b": (a & 143) == b, "(a >> 32) == b": (a >> 32) == b,
        "low32(a) > b": (a & 0xffffffff) > b, "b <= a < c": b <= a < c,
        "a * b fits u64": a*b <= MAX, "a + b fits u64": a+b <= MAX,
        "supported marker pair": (a,b) in ((0,0),(1,0),(2,0),(0,1),(1,1))
    }
    if row["condition_en"] in rules:
        require(not rules[row["condition_en"]], "reported comparison actually passed")
    if row["name"] in ("release_error", "tick_error"):
        require(b in (1,2,3,5), "incorrect policy error encoding")
    if row["name"] in ("release_missed_deadline", "tick_missed_deadline"):
        require(b == 4, "incorrect missed-deadline variant")
    return {"uart":"diagnosed_failure", "prefix_lines":prefix, "site":site,
            "name":row["name"], "mask":mask,
            "operands":[{"name":row[label],"value":v} for i,(label,v) in
                        enumerate(zip(("a","b","c"),operands)) if mask & (1 << i)],
            "condition_en":row["condition_en"], "condition_ko":row["condition_ko"]}


def linked_trace(image, raw):
    from elftools.elf.elffile import ELFFile
    from elftools.common.exceptions import ELFError
    try:
        elf = ELFFile(io.BytesIO(image))
    except ELFError as exc:
        raise ValueError("invalid ELF") from exc
    require(elf.elfclass == 64 and elf.little_endian and elf["e_machine"] == "EM_AARCH64",
            "incorrect image architecture")
    table = elf.get_section_by_name(".symtab")
    require(table is not None, "missing symbol table")
    def addr(name):
        found = table.get_symbol_by_name(name) or []
        require(len(found) == 1, "missing/ambiguous symbol " + name)
        return found[0]["st_value"]
    roles = ["cbs_a","cbs_b","cbs_idle","cbs_a","cbs_b"]
    vector = addr("sched_vectors") + 5*128
    lines = raw.decode("ascii").splitlines()
    saved = {}
    events = []
    i = 0
    while i < len(lines):
        slot = len(events)//2
        require(slot < len(roles), "extra exception history")
        name = roles[slot]
        if len(events)%2 == 0:
            eret = re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC (0x[0-9a-fA-F]+)",lines[i])
            require(eret, "missing linked ERET")
            pc = int(eret[1],16)
            require(pc == saved.get(name,addr(name)), "incorrect linked dispatch or saved-PC return")
            events.append(["eret",pc])
            i += 1
        else:
            require(re.fullmatch(r"Taking exception 5 \[IRQ\](?: on CPU 0)?",lines[i]), "non-IRQ exception")
            i += 1
            require(lines[i:i+2] == ["...from EL1 to EL1","...with ESR 0x0/0x0"], "incorrect IRQ level/syndrome")
            i += 2
            if i < len(lines) and lines[i].startswith("...with SPSR "):
                i += 1
            require(i+2 <= len(lines), "truncated IRQ")
            elr = re.fullmatch(r"\.\.\.with ELR (0x[0-9a-fA-F]+)",lines[i])
            target = re.fullmatch(r"\.\.\.to EL1 PC (0x[0-9a-fA-F]+) PSTATE 0x[0-9a-fA-F]+",lines[i+1])
            require(elr and target and int(target[1],16) == vector, "incorrect IRQ vector")
            pc = int(elr[1],16)
            require(pc%4 == 0 and addr(name+"_begin") <= pc < addr(name+"_end"), "unexpected interrupted PC")
            saved[name] = pc
            events.append(["irq",pc])
            i += 2
    return events

def replay(root, trusted):
    receipt = read(root/"receipt.json", 32 << 10)
    require(sha(receipt) == trusted, "receipt hash mismatch")
    info = json.loads(receipt, object_pairs_hook=unique)
    require(re.fullmatch(r"[0-9a-f]{40}",info["revision"]) and
            info["image_kind"] in ("plain","diagnostic") and type(info["pid"]) is int and info["pid"] > 0
            and type(info["runner_exit"]) is int and info["runner_exit"] in (0,1), "invalid run identity")
    require(set(info["hashes"]) == set(LIMITS), "missing or unexpected artifact")
    artifacts = {name:read(root/name,limit) for name,limit in LIMITS.items()}
    require(all(sha(data) == info["hashes"][name] for name,data in artifacts.items()), "artifact hash mismatch")
    log = artifacts["runner.log"].decode().splitlines()
    def one(prefix):
        found = [line[len(prefix):] for line in log if line.startswith(prefix)]
        require(len(found)==1, "missing or ambiguous runner field "+prefix)
        return found[0]
    require(one("Run ") == "1/1  ELF SHA-256 = "+info["hashes"]["kernel.elf"] and
            one("  QEMU PID = ") == str(info["pid"]), "run/ELF/PID mismatch")
    argv = json.loads(one("  argv = "))
    fixed = ["-machine","virt,gic-version=2","-cpu","cortex-a72","-smp","1","-accel","tcg",
             "-m","128M","-nodefaults","-display","none","-monitor","none","-serial","stdio","-kernel"]
    require(argv[1:1+len(fixed)] == fixed and len(argv) == 1+len(fixed)+5 and
            argv[-4:-1] == ["-d","int","-D"] and Path(argv[-1]).name == "exceptions.log",
            "unexpected model or trace argv")
    raw = artifacts["uart"]
    uart_field = one("  UART ")
    marker = f"({len(raw)} bytes) = "
    require(uart_field.startswith(marker) and ast.literal_eval(uart_field[len(marker):]) == raw,
            "runner/UART mismatch")
    result = classify(raw,sites(artifacts["sites.csv"]),info["image_kind"]=="diagnostic")
    try:
        events = linked_trace(artifacts["kernel.elf"],artifacts["exceptions.log"])
    except ValueError as exc:
        if result["uart"] == "ordinary_success":
            raise
        result["trace_discrepancy"] = str(exc)
        events = []
    result.update(revision=info["revision"], image_kind=info["image_kind"], pid=info["pid"],
                  receipt_sha256=trusted, hashes=info["hashes"], events=events)
    success = result["uart"] == "ordinary_success"
    if success:
        require(info["runner_exit"]==0 and log.count("  PASS (host stopped QEMU after the quiet window)")==1
                and log[-1]=="PASS: 1 cold boot(s)" and len(events)==10,
                "original nine-line/quiet/host-stop/five-IRQ criterion incomplete")
    else:
        require(info["runner_exit"]==1 and not any(line.startswith(("  PASS","PASS:")) for line in log),
                "failure contradicts original runner verdict")
        if result["uart"] != "partial" and "trace_discrepancy" not in result:
            markers = min(5,result["prefix_lines"]-2)
            erets = (len(events)+1)//2
            irqs = len(events)//2
            if not (markers <= erets <= markers+1 and max(0,markers-1) <= irqs <= markers):
                result["trace_discrepancy"] = "UART prefix and exception progress disagree"
    result["observation"] = ("uncorroborated UART; cause remains open" if "trace_discrepancy" in result
                             else result["uart"])
    result["ordinary_pass"] = success
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("bundle",type=Path)
    parser.add_argument("receipt_sha256")
    args = parser.parse_args()
    try:
        result = replay(args.bundle,args.receipt_sha256)
    except (ValueError,KeyError,TypeError,IndexError,OSError,ImportError,SyntaxError,csv.Error) as exc:
        print(json.dumps({"ordinary_pass":False,"observation":"unobservable or inconsistent","reason":str(exc)}))
        return 1
    print(json.dumps(result,ensure_ascii=False,indent=2))
    return 0 if result["ordinary_pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
