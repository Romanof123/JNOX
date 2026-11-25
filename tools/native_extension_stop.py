#!/usr/bin/env python3
"""Acquire or replay one #349 position. Pin the integrated accepted manifest out of band."""
import argparse
import json
from pathlib import Path
import re
import shutil
import struct
import sys

import native_el0_capture as acq
import native_extension_replay as ext
from native_el0_replay import Snapshot, argument_stacks, elf_file, permissions, symbol, vread
from ordinary_amc_capture import sha

RULE = ["ready 2 1 2", "propose 2 17 0 1", "yield"]
FILES = ("user.elf", "native_peer.elf", "native_peer2.elf")
STOPS = {"a-active": (1, 3), "b-active": (9,), "a-dormant": (), "b-dormant": (), "b-ordinary": ()}
OWNERS = {"a-active": "21012021", "a-dormant": "22021011",
          "b-active": "220110130330", "b-dormant": "220110130330", "b-ordinary": "220110130330"}
check = ext.check


def stack(elf, name):
    matches = [s for s in elf.get_section_by_name(".symtab").iter_symbols()
               if "native_el0" in s.name and name in s.name and s["st_size"] == 16384]
    check(len(matches) == 1, "missing or ambiguous kernel stack " + name)
    base = matches[0]["st_value"]
    return base, base + 16384


def sites(kernel):
    elf = elf_file(kernel.read_bytes())
    typed, extent = symbol(elf, "jnox_extension_evaluator_enter")
    loop, size = symbol(elf, "jnox_native_extension_stop_loop")
    check(extent == 64 and typed % 64 == 0 and size == 4 and loop == typed + 36,
          "source entry extent and stop-loop location")
    return (loop,
            symbol(elf, "native_el0_vectors")[0] + 5 * 128,
            symbol(elf, "native_sleep_irq")[0], typed)


def command(qemu, root, plain, ordinary=False):
    argv = acq.command(qemu, root, plain)
    if not plain and not ordinary:
        index = argv.index("-d")
        ranges = ",".join(f"0x{pc:x}+4" for pc in sites(root / "kernel.elf"))
        argv[index:index + 2] = ["-dfilter", ranges, "-d", "int,cpu"]
    return argv


def package_inputs(args):
    check(sha(args.reference) == args.reference_sha256, "archived reference manifest digest")
    reference = ext.load_json(args.reference)
    manifest = ext.load_json(args.package / "manifest.json")
    cohort_root = args.package.resolve().parent
    position, variant = args.position.split("-")
    check(args.package.resolve().name == variant and sha(cohort_root/"cohort.json") == args.cohort_sha256,
          "pinned source cohort/package position")
    cohort = ext.load_json(cohort_root/"cohort.json")
    spec = ext.load_json(cohort_root/"spec.json")
    linked = ext.load_json(cohort_root/"link"/f"{variant}.json")
    check(spec["position"] == position and sha(cohort_root/"spec.json") == cohort["spec_sha256"]
          and sha(cohort_root/"link/inputs.json") == cohort["inputs_sha256"]
          and sha(cohort_root/"link"/f"{variant}.json") == cohort["links"][variant]
          and cohort["manifests"][variant] == sha(args.package/"manifest.json")
          and linked["exit"] == 0 and linked["output_sha256"] == manifest["kernel_elf_sha256"]
          and sha(args.package/"build.log") == manifest["build_log_sha256"], "source cohort link chain")
    check(sha(args.preflight) == args.preflight_sha256, "independently sealed equivalence report")
    proof = ext.load_json(args.preflight)
    check(proof["position"] == position and proof["scope"] == "linked preflight only"
          and proof["verdict"] == "PASS" and proof["provenance"]["cohort_sha256"] == args.cohort_sha256
          and proof["kernels"][variant] == linked["output_sha256"], "same-position source equivalence")
    if variant == "ordinary":
        kernel = elf_file((args.package/"kernel.elf").read_bytes())
        typed, size = symbol(kernel, "jnox_extension_evaluator_enter")
        check(size == 64 and typed % 64 == 0 and not kernel.get_section_by_name(".symtab").get_symbol_by_name(
              "jnox_native_extension_stop_cells"), "selector-free ordinary typed entry")
    else:
        sites(args.package/"kernel.elf")
    order = [0, 1, 2] if args.position.startswith("a") else [2, 1, 0]
    check(reference["policy"] == "extension" and reference["extension_program"]["operations"] == RULE
          and manifest["policy"] == "extension" and manifest["board"] == "virt", "deployment")
    check(manifest["extension_program"]["operations"] == RULE
          and (args.package / "extension_program.txt").read_bytes() == ("\n".join(RULE) + "\n").encode()
          and sha(args.package / "extension_program.txt") == manifest["extension_program"]["sha256"], "rule")
    expected_selector = None if variant == "ordinary" else dict(position=args.position, cells=list(STOPS[args.position]))
    check(manifest.get("extension_stop_bench") == expected_selector, "package variant")
    check(sha(args.package/"kernel.elf") == manifest["kernel_elf_sha256"], "package kernel")
    check(len(manifest["slots"]) == 3 and len(reference["slots"]) == 3, "three slots")
    for i, role in enumerate(order):
        slot = manifest["slots"][i]
        expected = ["/bin/app", "four-batches"] if i == 0 and role == 2 else ["/bin/app"]
        check(slot["argv"] == expected and slot["cpu"] == 0 and slot["generation"] == ext.GEN[i]
              and slot["role"] == ("reserved" if i == 0 else "delegated"), "slot role/argv")
        check(slot["elf_sha256"] == reference["slots"][role]["elf_sha256"]
              == sha(args.package / FILES[i]), "archive ELF identity")
    check(sha(args.reference_launch) == args.reference_launch_sha256, "archived launch digest")
    original = ext.load_json(args.reference_launch)
    check(original["argument_package"] == reference, "archived accepted launch/manifest binding")
    return manifest, order, original["qemu_sha256"]


def read_trace(raw, rows, returns, addresses, eval_stack, idle_stack, stopped):
    loop, vector, handler, typed = addresses
    expected = [("IRQ" if r["cause"] == 1 else "SVC", int(r["src"] == 3), r["src_pc"])
                for r in rows if r["cause"] in (1, 2)]
    check(ext.trace_entries(raw) == expected, "external IRQ/SVC order or origin differs")
    check(ext.trace_returns(raw) == returns, "external ERET destinations differ")
    events = list(re.finditer(rb"(?m)^Taking exception ", raw))
    check(len(events) == len(expected), "unmatched external exception")
    # CPU blocks start at PC and terminate with PSTATE. Never borrow a register
    # across a different exception or a subsequent TB dump.
    dump = re.compile(rb"(?m)^[ \t]*PC=([0-9a-fA-F]+)\b([^\r\n]*\r?\n"
                      rb"(?:(?![ \t]*(?:PC|PSTATE)=)[^\r\n]*\r?\n){0,16})"
                      rb"[ \t]*PSTATE=([0-9a-fA-F]+)\b")
    def samples(chunk, pc):
        found = []
        for m in dump.finditer(chunk):
            if int(m[1], 16) != pc:
                continue
            sp = re.search(rb"\bSP=[ \t]*([0-9a-fA-F]+)\b", m[2])
            cell = re.search(rb"\bX0?0=[ \t]*([0-9a-fA-F]+)\b", m[2])
            if sp and cell:
                found.append((int(sp[1], 16), int(m[3], 16), m.start(), int(cell[1], 16)))
        return found

    unknown, witness = [], []
    decisions = [(i, r) for i, r in enumerate(r for r in rows if r["cause"] in (1, 2))
                 if r["reason"] != ext.NONE and r["cell"] % 3 != 2]
    typed_rows = []
    for i, row in decisions:
        chunk = raw[events[i-1].start() if i else 0:events[i].start()]
        samples_at_entry = samples(chunk, typed)
        if not samples_at_entry:
            unknown.append(f"cell {row['cell']}: missing typed-entry register sample")
            continue
        check(len(samples_at_entry) == 1, "duplicate typed-entry sample")
        sp, pstate, _, x0 = samples_at_entry[0]
        check(x0 == row["cell"] and eval_stack[0] <= sp < eval_stack[1] and sp % 16 == 0
              and pstate & 0x8f == 0x85, "typed entry cell, private SP or masked EL1h")
        typed_rows.append(dict(cell=x0, pc=typed, sp=sp, pstate=pstate))
    check(len(samples(raw, typed)) == len(typed_rows), "unexpected typed entry outside delegated window")
    fault_entries = [i for i, value in enumerate(expected) if value == ("IRQ", 1, loop)]
    check(len(fault_entries) == len(stopped), "external cutoff count at fail-stop PC")
    for k, i in zip(stopped, fault_entries):
        before = raw[events[i-1].start() if i else 0:events[i].start()]
        after = raw[events[i].start():events[i+1].start() if i+1 < len(events) else len(raw)]
        pre, entry, post = samples(before, loop), samples(after, vector), samples(after, handler)
        if not pre or not entry or not post:
            unknown.append(f"cell {k}: missing loop/vector/handler register dump")
            continue
        sp, pstate, _, x0 = pre[-1]
        vsp, vstate, vi, _ = entry[0]
        hsp, hstate, hi, _ = post[0]
        check(x0 == k and pstate & 0x8f == 5 and sp % 16 == 0 and eval_stack[0] <= sp < eval_stack[1],
              "loop EL1h/unmask/private stack")
        prior = next((sample for sample in typed_rows if sample["cell"] == k), None)
        if prior:
            check(prior["sp"] == sp, "typed entry to interrupted loop SP")
        check(vi < hi and vsp == sp and vstate & 0x8f == 0x85 and hstate & 0x8f == 0x85
              and idle_stack[0] <= hsp <= idle_stack[1] and hsp % 16 == 0, "abort to idle caller stack")
        witness.append(dict(cell=k, elr=loop, live_pstate=pstate, pre_abort_sp=vsp, handler_sp=hsp))
    return witness, unknown, typed_rows


def replay(args, manifest, order, qemu_hash):
    cap = args.root
    check(sha(cap/"session.json") == args.session_sha256, "session digest")
    session = ext.load_json(cap/"session.json")
    launch = ext.load_json(cap/"launch.json")
    plain = args.plain
    for name in ["kernel.elf", "ram.bin", "uart.bin", "launch.json", "user.elf"] + ([] if plain else ["exceptions.log"]):
        check(sha(cap/name) == session["hashes"][name], "capture hash " + name)
    check(matches_archived_command(launch["argv"], cap, plain, args.position == "b-ordinary")
          and launch["qemu_sha256"] == qemu_hash, "launch profile/QEMU identity")
    check(launch["argument_package"] == manifest and launch["revision"] == manifest["commit"]
          and session["hashes"]["kernel.elf"] == manifest["kernel_elf_sha256"] == launch["kernel_sha256"]
          and session["hashes"]["user.elf"] == manifest["slots"][0]["elf_sha256"] == launch["user_sha256"],
          "package/session binding")
    uart = (cap/"uart.bin").read_bytes()
    config, rows, loss, exits = ext.journal(uart)
    check(config[2:4] == [62500000, 6250000], "frozen clock")
    kernel = (cap/"kernel.elf").read_bytes()
    elf = elf_file(kernel)
    mem = Snapshot(launch["ram_base"], (cap/"ram.bin").read_bytes())
    info = argument_stacks(kernel, mem, manifest, uart)
    roots = [s["root"] for s in info]
    images = [elf_file((args.package/name).read_bytes()) for name in FILES]
    initial = [(img["e_entry"], slot["initial_sp"]) for img, slot in zip(images, info)]
    cells, returns, issues, svc_exits = ext.analyze(rows, config, RULE, roots, mem, loss, initial, False)
    stopped = STOPS[args.position]
    ordinary = args.position == "b-ordinary"
    if not ordinary:
        addr, size = symbol(elf, "jnox_native_extension_stop_cells")
        check(size == 16 and struct.unpack("<2Q", mem.read(addr, size)) ==
              stopped + (ext.NONE,) * (2 - len(stopped)), "loaded selector table")
    check([c["cell"] for c in cells] == list(range(len(OWNERS[args.position])))
          and [c.get("owner") for c in cells] == list(map(int, OWNERS[args.position])), "owner sequence")
    check([r["cell"] for r in rows if r["canceled"]] == list(stopped), "actual canceled cells")
    eval_stack, idle_stack = stack(elf, "9EVALUATOR"), stack(elf, "4IDLE")
    addresses = None if ordinary else sites(cap/"kernel.elf")
    loop = addresses[0] if addresses else None
    last_root = roots[0] | 1 << 48
    for r in rows:
        check(r["src_root"] == last_root, "exception root/ASID")
        if r["dst"] < 3 and r["pre_return"]:
            last_root = roots[r["dst"]] | (r["dst"] + 1) << 48
        if r["canceled"]:
            check(r["src"] == 3 and r["src_pc"] == loop and r["old_eval"] == 1 and not r["evaluating"]
                  and r["cause"] == 1 and r["op"] & 1023 == 30 and r["entry"] >= r["cutoff"]
                  and r["cval"] == r["old_compare"] == r["cutoff"] and r["ctl"] & 7 == 5,
                  "live evaluator PPI30 at nominal cutoff")
            permissions(mem, r["src_root"], loop, "kernel rx")
            check(vread(mem, r["src_root"], loop, 4) == bytes.fromhex("00000014"), "fail-stop instruction")
            for va in range(eval_stack[0] & ~4095, eval_stack[1], 4096):
                permissions(mem, r["src_root"], va, "kernel rw")
    for cell in cells:
        if cell["owner"] == 3:
            r = rows[cell["decision"]]
            check(r["idle_landing"] >= r["entry"] >= cell["start"], "idle witness precedes decision IRQ")
    by_role = sorted(range(3), key=lambda i: order[i])
    products = ext.products(mem, [images[i] for i in by_role], [roots[i] for i in by_role],
                            [manifest["slots"][i] for i in by_role])
    expected_exits = [30 + role for role in order]
    check(exits == expected_exits and svc_exits == dict(enumerate(exits)) and all(p["matches"] for p in products),
          "private products or SVC/terminal exit")
    witness, unknown, typed_rows = [], [], []
    if not plain and ordinary:
        raw = (cap/"exceptions.log").read_bytes()
        entries = [("IRQ" if r["cause"] == 1 else "SVC", int(r["src"] == 3), r["src_pc"])
                   for r in rows if r["cause"] in (1, 2)]
        check(ext.trace_entries(raw) == entries and ext.trace_returns(raw) == returns,
              "ordinary external exception/return stream")
    elif not plain:
        witness, unknown, typed_rows = read_trace((cap/"exceptions.log").read_bytes(), rows, returns, addresses,
                                      eval_stack, idle_stack, stopped)
    return dict(position=args.position, mode="plain" if plain else "captured", session=args.session_sha256,
                reference=args.reference_sha256, cohort=args.cohort_sha256, preflight=args.preflight_sha256, typed_entries=typed_rows, cells=cells, events=rows, roots=roots, exits=exits,
                products_by_voltage_temperature_alarms=products, cutoff_registers=witness,
                comparator_evidence="opening intended compare and observed entry CVAL/CTL; no at-arm hardware readback",
                pstate_evidence="live pre-IRQ state and unchanged self-branch; no direct saved SPSR_EL1 dump",
                issues=issues, unknown=unknown, plain_external_state="unobserved" if plain else None)


def matches_archived_command(argv, cap, plain, ordinary):
    old = Path(argv[-1]).parent
    local = command(argv[0], cap, plain, ordinary)
    return argv == [s.replace(str(cap/"uart.bin"), str(old/"uart.bin"))
                    .replace(str(cap/"exceptions.log"), str(old/"exceptions.log"))
                    .replace(str(cap/"kernel.elf"), str(old/"kernel.elf")) for s in local]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("capture", "replay"))
    parser.add_argument("position", choices=STOPS)
    for name in ("package", "root", "reference"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--cohort-sha256", required=True, help="pinned same-position cohort.json digest")
    parser.add_argument("--preflight", type=Path, required=True, help="same-position source-built equivalence.json")
    parser.add_argument("--preflight-sha256", required=True)
    parser.add_argument("--reference-sha256", required=True)
    parser.add_argument("--reference-launch", required=True, type=Path)
    parser.add_argument("--reference-launch-sha256", required=True)
    parser.add_argument("--session-sha256")
    parser.add_argument("--plain", action="store_true")
    parser.add_argument("--probe", type=Path)
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--nm", default="aarch64-linux-gnu-nm")
    args = parser.parse_args()
    manifest, order, qemu_hash = package_inputs(args)
    if args.operation == "replay":
        result = replay(args, manifest, order, qemu_hash)
        print(json.dumps(result, indent=2))
        return 1 if result["issues"] else 2 if result["unknown"] else 0
    check(args.probe is not None, "capture requires the archived probe source")
    check(sha(Path(shutil.which(args.qemu) or args.qemu)) == qemu_hash, "capture QEMU differs from archived executable")
    args.kernel, args.user = args.package/"kernel.elf", args.package/FILES[0]
    args.manifest = args.package/"manifest.json"
    args.revision = manifest["commit"]
    args.arg, args.peer_arg, args.peer2_arg = [], [], []
    args.expect_exit = 30 + order[0]
    return acq.capture(args, command_factory=lambda q, r, p: command(q, r, p, args.position == "b-ordinary"))


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, ImportError, IndexError, TypeError, struct.error) as error:
        sys.exit(f"inconsistent or incomplete evidence: {error}")
