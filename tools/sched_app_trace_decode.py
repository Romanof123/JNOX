#!/usr/bin/env python3
"""Seal first-attempt evidence, then decode a sealed CPU0 scheduling trace offline."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import uuid

SYMBOL = "jnox_sched_app_trace_v1"
WITNESS = "mlfq_wake_witness"
EVENTS = ["", "BLOCK", "READY", "DISPATCH", "TIMER_ENTER", "APP_RECEIPT", "TIMER_EXIT"]
EXTENT = 8320
MAX = (1 << 64) - 1


def check(condition, reason):
    if not condition:
        raise ValueError(reason)


def digest(path):
    with path.open("rb") as src:
        return hashlib.file_digest(src, "sha256").hexdigest() if hasattr(hashlib, "file_digest") else stream_hash(src)


def stream_hash(stream):
    result = hashlib.sha256()
    for block in iter(lambda: stream.read(1 << 20), b""):
        result.update(block)
    return result.hexdigest()


def put(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def load(path):
    return json.loads(path.read_bytes())


def location(elf, name):
    table = subprocess.check_output(["aarch64-linux-gnu-nm", "-S", elf], text=True)
    matches = [line.split() for line in table.splitlines() if line.split()[-1:] == [name]]
    if not matches:
        return None
    check(len(matches) == 1 and len(matches[0]) == 4, "ambiguous symbol " + name)
    address, size, _, _ = matches[0]
    return [int(address, 16), int(size, 16)]


def seal(out, build_log, boot_log):
    # The observer owns the boot log and may finish writing after wrapper exit.
    check(not (out / "manifest.json").exists() and not (out / "manifest.sha256").exists(),
          "directory already sealed")
    errors = []
    for source, name in [(build_log, "build.log"), (boot_log, "boot.log")]:
        try:
            dest = out / name
            if source.resolve() != dest.resolve():
                check(not dest.exists(), "log already exists: " + name)
                shutil.copyfile(source, dest)
        except (OSError, ValueError) as exc:
            errors.append(str(exc))
    manifest = {"schema": 1, "dictionary": EVENTS[1:], "seal_errors": errors}
    try:
        capture = load(out / "capture.json")
        launch = load(out / "launch.json")
        manifest.update(run_uuid=capture["run_uuid"], flags=capture["flags"],
                        mode=capture["flags"]["JNOX_SCHED_APP_TRACE"] or "off",
                        profile="virt/multilevel/mlfq-wake", source_commit=capture["source_commit"],
                        source_status=capture["source_status"], tools=capture["tools"],
                        command=launch["argv"], symbol=location(out / "kernel.elf", SYMBOL),
                        extraction=[launch["trace_address"], launch["trace_size"]]
                        if "trace_address" in launch else None)
        if (out / "trace.bin").exists():
            with (out / "trace.bin").open("rb") as src:
                freq, boot, session = struct.unpack_from("<3Q", src.read(80), 56)
            manifest.update(frequency=freq, boot=boot, session=session)
    except (OSError, ValueError, KeyError, struct.error, subprocess.SubprocessError) as exc:
        errors.append(str(exc))
    manifest["artifacts"] = {p.name: digest(p) for p in sorted(out.iterdir()) if p.is_file()}
    put(out / "manifest.json", manifest)
    (out / "manifest.sha256").write_text(digest(out / "manifest.json") + "  manifest.json\n")
    return not errors


def parse_trace(raw, mode):
    check(len(raw) == EXTENT, "trace extent")
    head = struct.unpack_from("<16Q", raw)
    check(raw[:8] == b"JNXTRC01" and head[1:5] == (1, 128, 64, 128), "trace ABI")
    cap, state, freq, boot, session, count, dk, da, flags, cpu, reserved = head[5:]
    check(mode in ("disabled", "active", "saturate"), "unknown mode")
    check(cap == (4 if mode == "saturate" else 128), "logical capacity")
    check(state == (0 if mode == "disabled" else 2), "disabled or frozen state required")
    check(freq > 0 and (boot, session, cpu, reserved) == (1, 1, 0, 0), "clock/namespace")
    check(count <= cap and (not (dk or da) or count == cap), "count or premature loss")
    check(flags == 0 or (flags == 1 and MAX in (dk, da)), "saturation flag")
    check(mode != "disabled" or not (count or dk or da), "disabled publications")
    check(not any(raw[128 + 64*count:]), "nonzero unpublished slots")
    events = []
    irqs = []
    opened = None
    last_instance = last_kernel = 0
    for i in range(count):
        seq, kind, tick, actor, subject, a, b, zero = struct.unpack_from("<8Q", raw, 128+64*i)
        refs = range(1 << 32, (1 << 32) + 4)
        check(seq == i+1 and 1 <= kind <= 6 and not zero and tick > 0, "sequence/type/time/reserved")
        check(subject in refs and (actor in refs or (kind == 3 and i == 0 and actor == 0)),
              "task reference/generation")
        actor = actor & 3 if actor else None
        subject &= 3
        if kind != 5:
            check(tick >= last_kernel, "kernel counter discontinuity")
            last_kernel = tick
        if kind in (1, 3):
            check(a == b == 0 and (kind == 3 or actor == subject), "scheduler arguments")
        elif kind == 2:
            check((actor, subject, b) == (1, 0, 0) and 0 < a <= tick, "ready arguments")
        elif kind == 5:
            check((actor, subject, b) == (0, 0, 0), "receipt attribution")
        else:
            check(actor == subject and a == 30 and 0 < b <= tick, "timer arguments")
            if kind == 4:
                check(opened is None and b > last_instance, "overlapping or reused timer instance")
                opened = (actor, a, b, tick)
                last_instance = b
            else:
                check(opened is not None and opened[:3] == (actor, a, b), "orphan or mismatched timer exit")
                irqs.append({"owner": actor, "line": a, "instance": b, "enter": opened[3],
                             "exit": tick, "rust_service_ticks": tick-opened[3]})
                opened = None
        events.append(dict(sequence=seq, event=EVENTS[kind], tick=tick, actor=actor,
                           subject=subject, arg0=a, arg1=b))
    check(opened is None or dk or da, "unclosed timer instance")
    return dict(frequency=freq, boot=boot, session=session, cpu=0, task_generation=1,
                dropped_kernel=dk,
                dropped_application=da, counter_saturated=bool(flags), events=events,
                timer_services=irqs, open_timer=opened)


def match_witness(raw, report, mode):
    check(len(raw) == (40+15*64)*8, "witness extent")
    h = struct.unpack_from("<40Q", raw)
    check(h[0] == 4 and h[39] == 0 and 0 < h[4] <= 64, "workload incomplete")
    rows = list(struct.iter_unpack("<15Q", raw[320:320+120*h[4]]))
    check(rows[-1][0] == 1 and rows[-1][9] == 4, "incorrect terminal transition")
    if mode == "off":
        return
    check(report["frequency"] // 100 == h[1] > 0, "witness counter frequency")
    if mode == "disabled":
        return
    expected = {k: [] for k in EVENTS[1:]}
    kernel_order = []
    for r in rows:
        kind, ok, owner, _, _, stop = r[:6]
        if kind == 4:
            kernel_order.append("TIMER_ENTER")
            for key in ("TIMER_ENTER", "TIMER_EXIT"):
                expected[key].append((owner, owner, 30, stop))
        if kind == 2:
            kernel_order.append("BLOCK")
            expected["BLOCK"].append((owner, owner, stop))
        if kind == 3 and ok == 1:
            kernel_order.append("READY")
            expected["READY"].append((owner, 0, stop))
        if r[9] < 4:
            kernel_order.append("DISPATCH")
            expected["DISPATCH"].append((owner if owner < 4 else None, r[9], r[10]))
        if kind == 4:
            kernel_order.append("TIMER_EXIT")
    actual_order = [e["event"] for e in report["events"] if e["event"] != "APP_RECEIPT"]
    check(actual_order == kernel_order[:len(actual_order)], "incorrect kernel event order")
    expected["APP_RECEIPT"].append((0, 0, h[28], h[9]))
    seen = {key: [] for key in expected}
    for e in report["events"]:
        key = e["event"]
        values = (e["actor"], e["subject"])
        if key.startswith("TIMER"):
            values += (e["arg0"], e["arg1"])
        elif key == "APP_RECEIPT":
            values += (e["arg0"], e["tick"])
        else:
            values += (e["arg0"] if key == "READY" else e["tick"],)
        seen[key].append(values)
    for key in expected:
        check(seen[key] == expected[key][:len(seen[key])], key + " disagrees with workload witness")
    missing = {k: len(expected[k])-len(seen[k]) for k in expected}
    check(report["dropped_application"] == missing.pop("APP_RECEIPT") and
          report["dropped_kernel"] == sum(missing.values()), "missing events or wrong source losses")
    events = report["events"]
    if report["dropped_kernel"] or report["dropped_application"]:
        report["coverage"] = "incomplete-loss"
        return
    check(mode != "saturate", "saturation run did not overflow")
    blocks = [e for e in events if e["event"] == "BLOCK" and e["subject"] == 0]
    wakes = [e for e in events if e["event"] == "READY"]
    receipts = [e for e in events if e["event"] == "APP_RECEIPT"]
    check(len(blocks) == len(wakes) == len(receipts) == 1, "ambiguous workload join")
    ready, receipt = wakes[0], receipts[0]
    selected = next((e for e in events if e["sequence"] > ready["sequence"]
                     and e["event"] == "DISPATCH" and e["subject"] == 0), None)
    check(selected and blocks[0]["sequence"] < ready["sequence"] and
          selected["sequence"] < receipt["sequence"], "missing ordered dispatch join")
    w, q, s, e = ready["arg0"], ready["tick"], selected["tick"], receipt["tick"]
    check(blocks[0]["tick"] <= w <= q <= s <= e and (w,s,e) == (h[5],h[8],h[9]),
          "wake join or clock discontinuity")
    check(not any(r[2] == 0 and s <= r[5] < e for r in rows), "target stopped before receipt")
    check(e-w == (q-w)+(s-q)+(e-s) and e-w == h[15] and e-s == h[16] and e-w <= 23*h[1], "workload bound")
    report["interval"] = {"W": w, "Q": q, "S": s, "E": e, "Q-W": q-w, "S-Q": s-q,
                          "S-W": s-w, "E-S": e-s, "E-W": e-w}
    report["coverage"] = "complete"



def decode(out):
    manifest = out / "manifest.json"
    check((out / "manifest.sha256").read_text().strip() == digest(manifest) + "  manifest.json",
          "manifest digest")
    m = load(manifest)
    check(m["schema"] == 1 and m["dictionary"] == EVENTS[1:] and not m["seal_errors"],
          "schema, dictionary or incomplete seal")
    artifacts = m["artifacts"]
    for name, sha in artifacts.items():
        check(name == Path(name).name and not (out / name).is_symlink(), "invalid artifact path")
        check(digest(out / name) == sha, "artifact digest: " + name)
    required = {"capture.json", "launch.json", "exit.json", "kernel.elf", "uart.bin", "int.trace",
                "registers.txt", "qmp.json", "halted.json", "build.log", "boot.log", WITNESS + ".bin",
                "multilevel_terminal_ctl.bin"}
    check(required <= artifacts.keys(), "missing artifact(s): " + str(required - artifacts.keys()))
    c, launch = load(out / "capture.json"), load(out / "launch.json")
    check(c.get("result") == 0 and "error" not in c, "capture failed")
    uid = uuid.UUID(c["run_uuid"])
    check(uid.version == 4 and str(uid) == m["run_uuid"], "run UUID")
    check(m["flags"] == c["flags"] and m["tools"] == c["tools"] and
          m["source_status"] == c["source_status"] and m["source_commit"] == c["source_commit"]
          and re.fullmatch("[0-9a-f]{40,64}", m["source_commit"]), "source/tool metadata")
    flags = m["flags"]
    check([flags[k] for k in ("JNOX_BOARD", "JNOX_PROFILE", "JNOX_APP")] ==
          ["virt", "multilevel", "mlfq-wake"] and m["profile"] == "virt/multilevel/mlfq-wake",
          "profile mismatch")
    mode = m["mode"]
    check(flags["JNOX_SCHED_APP_TRACE"] in (None, "disabled", "active", "saturate") and
          mode == (flags["JNOX_SCHED_APP_TRACE"] or "off") and
          flags["JNOX_MULTILEVEL_TRACE"] in (None, "1"), "effective flags")
    argv = launch["argv"]
    check(argv == m["command"] and argv.count("-kernel") == 1 and
          argv[argv.index("-kernel")+1] == c["capture_directory"] + "/kernel.elf",
          "archived launch ELF")
    boot_log = (out / "boot.log").read_text(errors="replace")
    check("Run 1/1  ELF SHA-256 = " + artifacts["kernel.elf"] in boot_log and
          "PASS: 1 cold boot(s)" in boot_log and "FAIL:" not in boot_log, "observer verdict/ELF")
    check((out / "build.log").stat().st_size > 0, "empty build log")
    elf = out / "kernel.elf"
    check(location(elf, WITNESS) == [launch["audit_address"], launch["audit_size"]],
          "original witness extraction")
    extras = {"multilevel_terminal_ctl"}
    if flags["JNOX_MULTILEVEL_TRACE"]:
        extras.add("ordinary_multilevel_witness")
    check(set(launch["extra_symbols"]) == extras, "auxiliary witness set")
    for name in extras:
        extent = location(elf, name)
        check(name+".bin" in artifacts and extent == launch["extra_symbols"][name] and
              extent[1] == (out / (name+".bin")).stat().st_size, name + " extraction")
    check((location(elf, "ordinary_multilevel_witness") is not None) ==
          bool(flags["JNOX_MULTILEVEL_TRACE"]), "ELF full-frame selector")
    state = load(out / "qmp.json")
    check(state["status"]["running"] is True and state["status"]["status"] == "running"
          and len(state["cpus"]) == 1 and
          state["cpus"][0]["cpu-index"] == 0, "CPU0 pre-halt status")
    halted = load(out / "halted.json")
    check(halted["running"] is False and halted["status"] == "paused", "halted extraction status")
    pc = re.search(r"\bPC=([A-Fa-f0-9]+)\b", (out / "registers.txt").read_text())
    idle, length = location(elf, "fixed_runtime_idle")
    check(pc and idle <= int(pc[1], 16) < idle+length, "terminal idle PC")
    ctl, = struct.unpack("<Q", (out / "multilevel_terminal_ctl.bin").read_bytes())
    check(not ctl & 1, "terminal timer still enabled")
    check((out / "int.trace").stat().st_size > 0, "empty exception transcript")
    uart = (out / "uart.bin").read_bytes().replace(b"\r\n", b"\n")
    check(uart == b"JNOX M1 BOOT\nJNOX M1 READY\nMLFQ WAKE ADMIT\nMLFQ WAKE VERIFIED\nMULTILEVEL DONE\n",
          "workload UART incomplete")
    check(load(out / "exit.json")["qemu_status"] in (0, -15), "guest shutdown")
    trace_symbol = location(elf, SYMBOL)
    check(trace_symbol == m["symbol"], "recorder symbol metadata")
    report = {"run_uuid": str(uid), "mode": mode, "coverage": mode, "interval": None,
              "manifest_sha256": digest(manifest)}
    if mode == "off":
        check(trace_symbol is None and "trace.bin" not in artifacts and not (out / "trace.bin").exists()
              and m["extraction"] is None and "trace_symbol" not in launch, "OFF recorder present")
    else:
        check("trace.bin" in artifacts and launch["trace_symbol"] == SYMBOL and
              [launch["trace_address"], launch["trace_size"]] == trace_symbol == m["extraction"]
              and trace_symbol[0] % 8 == 0 and trace_symbol[1] == EXTENT, "recorder extraction")
        report.update(parse_trace((out / "trace.bin").read_bytes(), mode))
        check(all(report[k] == m[k] for k in ("frequency", "boot", "session")), "clock/session metadata")
    match_witness((out / (WITNESS+".bin")).read_bytes(), report, mode)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    sealing = commands.add_parser("seal")
    sealing.add_argument("directory", type=Path)
    sealing.add_argument("--build-log", required=True, type=Path)
    sealing.add_argument("--boot-log", required=True, type=Path)
    commands.add_parser("decode").add_argument("directory", type=Path)
    args = parser.parse_args()
    out = args.directory
    if args.command == "seal":
        return 0 if seal(out, args.build_log, args.boot_log) else 1
    try:
        result = decode(out)
        failed = False
    except (OSError, KeyError, IndexError, TypeError, ValueError, struct.error,
            subprocess.SubprocessError) as error:
        result = {"coverage": "incomplete", "diagnostic": str(error), "interval": None}
        failed = True
    put(out / "decoded.json", result)
    (out / "decoded.sha256").write_text(digest(out / "decoded.json") + "  decoded.json\n")
    print(result["coverage"], result.get("diagnostic", ""))
    return int(failed or (result.get("mode") == "active" and result["coverage"] != "complete"))


if __name__ == "__main__":
    sys.exit(main())
