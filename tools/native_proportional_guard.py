#!/usr/bin/env python3
"""Record and replay the separately built native proportional wrong-owner control."""
import argparse
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import threading
import time

from native_el0_capture import command
from native_el0_replay import ESR, PA_MASK, SVC, Snapshot, elf_file, permissions, symbol, vread
from ordinary_amc_capture import sha, save
from qemu_capture import request


def need(ok, reason):
    if not ok:
        raise ValueError(reason)


class Pipe:
    def __init__(self, proc):
        self.read = proc.stdout.readline
        self.send = proc.stdin

    def readline(self):
        return self.read()

    def write(self, payload):
        self.send.write(payload)
        self.send.flush()


def extent(kernel):
    base = symbol(kernel, "_start")[0]
    end = symbol(kernel, "__stack_top")[0]
    need(base == 0x40200000 and base < end <= base + (48 << 20), "unexpected kernel extent")
    return base, end - base


def capture(args):
    out = args.directory.resolve()
    need(re.fullmatch("[a-f0-9]{40}", args.revision) and args.timeout > 0, "revision or timeout")
    out.mkdir()
    proc = watchdog = None
    outcome = "incomplete"
    reason = ""
    try:
        for src, name in ((args.kernel, "kernel.elf"), (args.user, "user.elf"), (args.peer_elf, "peer.elf")):
            if src:
                shutil.copyfile(src, out / name)
        base, size = extent(elf_file((out / "kernel.elf").read_bytes()))
        qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
        cmd = command(qemu, out)
        save(out / "launch.json", dict(argv=cmd, revision=args.revision, ram_base=base, ram_bytes=size,
                                      qemu_sha256=sha(Path(qemu)), qemu_version=subprocess.check_output(
                                          [qemu, "--version"], text=True), timeout_seconds=args.timeout))
        with (out / "stderr").open("xb") as err:
            proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)
            watchdog = threading.Timer(args.timeout, proc.kill)
            watchdog.start()
            pipe = Pipe(proc)
            need("QMP" in json.loads(pipe.readline()), "QMP greeting missing")
            request(pipe, "qmp_capabilities")
            uart_file = out / "uart.bin"
            while proc.poll() is None:
                if uart_file.exists():
                    need(uart_file.stat().st_size <= (1 << 20), "oversize UART")
                    uart = uart_file.read_bytes()
                    if re.search(rb"(?m)^NATIVE EL0 (?:FAULT|EXIT) [^\r\n]+\r?\n", uart):
                        break
                time.sleep(0.01)
            need(proc.poll() is None, "guest terminated or capture timed out")
            # The guest parks itself. Only pause/resume the vCPU and read it.
            for i in range(2):
                time.sleep(0.1)
                need(request(pipe, "query-status")["running"], "expected running VM before sample")
                request(pipe, "stop")
                need(request(pipe, "query-status")["status"] == "paused", "guest did not stop")
                stamp = time.monotonic_ns()
                cpus = request(pipe, "query-cpus-fast")
                regs = request(pipe, "human-monitor-command", **{"command-line": "info registers"})
                save(out / f"cpu{i}.json", dict(host_ns=stamp, cpus=cpus, registers=regs))
                request(pipe, "pmemsave", val=base, size=size, filename=str(out / f"ram{i}.bin"))
                if i == 0:
                    request(pipe, "cont")
            outcome = "captured"
    except (RuntimeError, OSError, ValueError) as exc:
        reason = str(exc)
    finally:
        if watchdog:
            watchdog.cancel()
            watchdog.join()
        if proc:
            if proc.poll() is None:
                proc.kill()
            proc.wait()
        save(out / "session.json", dict(outcome=outcome, detail=reason,
             hashes={p.name: sha(p) for p in out.iterdir() if p.is_file()}))
        print(sha(out / "session.json"))
    return int(outcome != "captured")


def linked(kernel, name, size):
    table = kernel.get_section_by_name(".symtab")
    need(table is not None, "missing kernel symbol table")
    # Exact legacy Rust module path. LLVM may append its numeric LTO suffix.
    path = rf"_ZN11jnox_kernel10native_el012proportional{len(name)}{re.escape(name)}"
    mangled = re.compile(path + r"17h[0-9a-f]{16}E(?:\.llvm\.[0-9]+)?")
    found = [s for s in table.iter_symbols() if mangled.fullmatch(s.name)]
    need(len(found) == 1, "missing or ambiguous " + name)
    need(found[0]["st_info"]["type"] == "STT_OBJECT" and found[0]["st_size"] == size,
         "unexpected object type or extent for " + name)
    return found[0]["st_value"]


def trace_context(raw, esr, elr, vbar):
    lines = [s.strip() for s in raw.splitlines() if s.strip()]
    calls = [i for i, s in enumerate(lines) if s.startswith("Taking exception 2 [SVC]")]
    need(calls, "missing external SVC")
    i = calls[-1]
    need(any(s.startswith("Exception return from AArch64 EL1 to AArch64 EL0 PC ") for s in lines[:i]),
         "missing actual EL0 return")
    need(len(lines[i:]) == 5 and lines[i+1] == "...from EL0 to EL1", "last exception was not lower EL SVC")
    def take(pattern, line):
        match = re.fullmatch(pattern, line)
        need(match, "unexpected exception trace: " + line)
        return [int(x, 16) for x in match.groups()]
    need(take(r"\.\.\.with ESR 0x15/(0x\w+)", lines[i+2]) == [esr] and
         take(r"\.\.\.with ELR (0x\w+)", lines[i+3]) == [elr], "external syndrome/ELR")
    vector, status = take(r"\.\.\.to EL1 PC (0x\w+) PSTATE (0x\w+)", lines[i+4])
    need(vector == vbar + 0x400 and status & 15 == 5, "lower A64 vector")
    return vector


def uart_records(raw):
    records = []
    for line in raw.splitlines():
        if line.startswith(b"NATIVE PROP "):
            tag, *words = line.split()[2:]
            if tag in (b"PREDICATE", b"GUARD_EVENT", b"FAULT"):
                need(all(w.isdigit() for w in words), "invalid UART field")
                records.append((tag, list(map(int, words))))
        if line.startswith(b"NATIVE EL0 FAULT "):
            records.append((b"footer", [int(line.split()[-1])]))
    need([(k,len(w)) for k,w in records] == [(b"PREDICATE",8),(b"GUARD_EVENT",6),(b"FAULT",12),
                                             (b"footer",1)], "missing or unexpected diagnostic sequence")
    need(records[-1][1] == [24], "different terminal fault")
    return [w for _,w in records[:3]]

def replay(args):
    out = args.directory
    need(sha(out / "session.json") == args.session_sha256, "session digest")
    session = json.loads((out / "session.json").read_bytes())
    need(session["outcome"] == "captured", "incomplete capture: " + session["detail"])
    for name in ("kernel.elf", "user.elf", "peer.elf", "uart.bin", "exceptions.log", "ram0.bin",
                 "ram1.bin", "cpu0.json", "cpu1.json", "launch.json"):
        need(sha(out / name) == session["hashes"][name], name + " digest")
    uart = (out / "uart.bin").read_bytes()
    pred, event, fault = uart_records(uart)
    kernel = elf_file((out / "kernel.elf").read_bytes())
    user = elf_file((out / "user.elf").read_bytes())
    digest = session["hashes"]["user.elf"]
    need(digest == session["hashes"]["peer.elf"] and all(
        (f"NATIVE EL0 {prefix}ELF {digest}").encode() in uart.splitlines()
        for prefix in ("", "PEER ")), "diagnostic slot ELF identities")
    base, size = extent(kernel)
    first = linked(kernel, "FIRST_REJECTION", 72)
    guard = linked(kernel, "GUARD_EVENT", 48)
    arm_address, arm_size = symbol(kernel, "native_weighted_arm")
    need(arm_size == 80, "arm ABI")
    code, caller, running, stamp, cval, ctl, root, pc, pstate, esr, sp, far = fault
    kind, op, entry, decision, arg, result = event
    dt = pred[6]
    need(pred == [17,10,1,0,0,0,dt,0] and (code,caller,running) == (17,1,0) and
         (kind,op,result) == (2,8,0) and 0 < entry <= decision <= stamp and dt > 0,
         "wrong predicate, operands or event")
    need(pstate & 31 == 0 and esr == ESR and root >> 48 == 1 and sp % 16 == 0 and ctl & 3 == 1,
         "saved EL0 exception/timer context")
    vector = trace_context((out / "exceptions.log").read_text(), esr, pc, symbol(kernel,"native_el0_vectors")[0])
    need(any(seg["p_type"] == "PT_LOAD" and seg["p_flags"] & 1 and
             seg["p_vaddr"] <= pc-4 < seg["p_vaddr"] + seg["p_filesz"] and
             int.from_bytes(seg.data()[pc-4-seg["p_vaddr"]:pc-seg["p_vaddr"]], "little") == SVC
             for seg in user.iter_segments()), "linked user SVC")
    samples = []
    anchor = struct.pack("<5Q", esr, pstate, pc, sp, 4)
    previous_frame = None
    for i in range(2):
        raw = (out / f"ram{i}.bin").read_bytes()
        need(len(raw) == size, "parked RAM extent")
        mem = Snapshot(base, raw)
        discr, *words = struct.unpack("<9Q", mem.read(first, 72))
        need(discr == 1 and words == pred, "FIRST Option discriminant/payload")
        need(list(struct.unpack("<6Q", mem.read(guard, 48))) == event, "pinned guard event")
        arm = struct.unpack("<10Q", mem.read(arm_address, 80))
        need(arm[0] == 0 and arm[9] == 0 and arm[3] + dt == entry and arm[4] == cval and
             arm[4] == min(arm[3]+arm[1], arm[2]) and arm[5] & 3 == 1 and
             arm[3] <= arm[6] <= entry and arm[8] == root and
             mem.read(arm[7],4) == bytes.fromhex("e0039fd6"), "actual arm/owner/elapsed/ERET")
        permissions(mem, root, pc-4, "user rx")
        permissions(mem, root, sp-1, "user rw")
        need(int.from_bytes(vread(mem, root, pc-4, 4),"little") == SVC, "mapped SVC")
        journal = symbol(kernel, "jnox_native_el0_journal")[0]
        header = struct.unpack("<16Q",mem.read(journal,128))
        need(header[1:3] == (4,24) and header[14] & PA_MASK == root & PA_MASK, "journal fault/root")
        cpu = json.loads((out / f"cpu{i}.json").read_bytes())
        need(len(cpu["cpus"]) == 1 and cpu["cpus"][0]["cpu-index"] == 0, "CPU0 topology")
        match = re.search(r"\bPC\s*=\s*(?:0x)?([\da-fA-F]+)",cpu["registers"])
        need(match, "missing parked CPU PC")
        park_pc = int(match[1],16)
        insn = int.from_bytes(mem.read(park_pc,4), "little")
        if insn != 0xd503205f:  # WFE or the loop's direct branch back to WFE
            imm = insn & 0x3ffffff
            offset = (imm - (1 << 26) if imm & (1 << 25) else imm) * 4
            need(insn >> 26 == 0b000101 and mem.read(park_pc+offset,4) == bytes.fromhex("5f2003d5"),
                 "CPU outside park loop")
        candidates = []
        pos = raw.find(anchor)
        while pos >= 0:
            addr = base + pos - 248
            if addr >= base and addr % 16 == 0 and addr+848 <= base+size and mem.read(addr+64,8) == struct.pack("<Q",8):
                candidates.append(addr)
            pos = raw.find(anchor,pos+1)
        need(len(candidates) == 1, "missing or ambiguous lower-A64 exception frame")
        # The measured lower vector pushes x2 and CNTPCT before saving the
        # 832-byte Rust NativeFrame. The complete exception stack is 848 bytes.
        frame = mem.read(candidates[0],848)
        saved_x2, saved_entry = struct.unpack_from("<2Q", frame, 832)
        need(saved_x2 == struct.unpack_from("<Q", frame, 16)[0] and saved_entry == entry,
             "lower vector x2/entry scratch differs from the frame or event")
        need(previous_frame in (None, frame), "parked frame changed")
        previous_frame = frame
        samples.append(dict(host_ns=cpu["host_ns"], parked_pc=park_pc, frame=candidates[0],
                            first_address=first, return_counter=arm[3], vector_entry=saved_entry, eret_pc=arm[7]))
    need(samples[1]["host_ns"] > samples[0]["host_ns"], "undistinguished park samples")
    print(json.dumps(dict(status="recovered", predicate=pred, event=event, fault=fault, vector=vector,
                          parked_samples=samples, revision=json.loads((out/"launch.json").read_bytes())["revision"]),
                     indent=2))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)
    cap = commands.add_parser("capture")
    cap.add_argument("directory", type=Path)
    cap.add_argument("kernel", type=Path)
    cap.add_argument("user", type=Path)
    cap.add_argument("revision")
    cap.add_argument("--peer-elf", type=Path, required=True)
    cap.add_argument("--timeout", type=int, default=30)
    cap.add_argument("--qemu", default="qemu-system-aarch64")
    read = commands.add_parser("replay")
    read.add_argument("directory", type=Path)
    read.add_argument("session_sha256")
    args = parser.parse_args()
    try:
        return capture(args) if args.operation == "capture" else replay(args)
    except (OSError, ValueError, KeyError, struct.error) as exc:
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
