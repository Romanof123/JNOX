#!/usr/bin/env python3
"""Package up to three native AArch64 applications for virt or Raspberry Pi 4."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from native_hierarchy import parse_topology, unique_keys
from native_lottery_admission import forecast

ROOT = Path(__file__).resolve().parents[1]
SDK = ROOT / "sdk/native-aarch64"
SOURCE = ROOT / "examples/portability/native_startup_probe.c"
TARGET = "aarch64-unknown-none"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def output(args, cwd=ROOT, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True).strip()


class SinglePath(argparse.Action):
    def __call__(self, parser, namespace, value, option_string=None):
        if getattr(namespace, self.dest) is not None:
            parser.error(f"{option_string} may appear only once")
        setattr(namespace, self.dest, value)


def deadline_job(value):
    fields = value.split(",")
    if len(fields) != 3 or any(not (field.isascii() and field.isdecimal()) for field in fields):
        raise argparse.ArgumentTypeError("expected release,deadline,cost in unsigned decimal grid units")
    release, deadline, cost = map(int, fields)
    if not (release < deadline <= 16 and 0 < cost < 2**64):
        raise argparse.ArgumentTypeError("require 0 <= release < deadline <= 16 and positive u64 cost")
    return release, deadline, cost


def periodic_parameters(value):
    fields = value.split(",")
    if len(fields) != 3 or any(not (field.isascii() and field.isdecimal()) for field in fields):
        raise argparse.ArgumentTypeError("expected phase,period,relative_deadline in unsigned decimal nanoseconds")
    phase, period, deadline = map(int, fields)
    if not (phase < 2**64 and 0 < deadline <= period < 2**64):
        raise argparse.ArgumentTypeError("require u64 nanoseconds and 0 < relative_deadline <= period")
    return phase, period, deadline


def budget_tuple(value):
    fields = value.split(",")
    if len(fields) != 2 or any(not (field.isascii() and field.isdecimal()) for field in fields):
        raise argparse.ArgumentTypeError("expected quota,period in unsigned decimal slots")
    quota, period = map(int, fields)
    if not 0 < quota <= period <= 2**32 - 1:
        raise argparse.ArgumentTypeError("require 0 < quota <= period <= 2^32-1")
    return quota, period


def unsigned_decimal(value):
    if not (value.isascii() and value.isdecimal()):
        raise argparse.ArgumentTypeError("expected unsigned decimal digits")
    return int(value)


def allotments(value):
    values = [unsigned_decimal(field) for field in value.split(",")]
    if len(values) != 3 or any(not 1 <= ticks < 2**63 for ticks in values):
        raise argparse.ArgumentTypeError("expected three positive allotments, each at most 2^63-1 ticks")
    return values


def partition_table(parser, raw):
    fields = raw.split(",")
    if not 2 <= len(fields) <= 16:
        parser.error("partition table requires 2..16 windows")
    windows = []
    frame = 0
    for field in fields:
        owner, separator, millis = field.partition(":")
        if owner not in ("a", "b", "idle") or not separator or not (millis.isascii() and millis.isdecimal()):
            parser.error("expected a:MS, b:MS or idle:MS")
        digits = millis.lstrip("0")
        if not digits or len(digits) > 20:
            parser.error("window duration must be a positive u64")
        duration = int(digits)
        frame += duration
        if frame >= 2**64:
            parser.error("partition frame exceeds u64 milliseconds")
        windows.append({"owner": owner, "duration_ms": duration})
    if not {"a", "b"}.issubset(window["owner"] for window in windows):
        parser.error("partition table requires both owners")
    return windows, frame


def weighted_notification(value):
    mode, separator, count = value.partition(":")
    if mode not in ("closures", "wakes") or not separator or not (count.isascii() and count.isdecimal()):
        raise argparse.ArgumentTypeError("expected closures:N or wakes:N")
    digits = count.lstrip("0")
    if not digits or len(digits) > 20 or int(digits) >= 2**64:
        raise argparse.ArgumentTypeError("notification threshold must be a positive u64")
    return mode, int(digits)


def thread_priorities(value):
    fields = value.split(",")
    if len(fields) != 4:
        raise argparse.ArgumentTypeError("expected exactly four comma-separated thread priorities")
    values = [unsigned_decimal(field) for field in fields]
    if any(not 1 <= priority <= 31 for priority in values):
        raise argparse.ArgumentTypeError("thread priorities must be in 1..31")
    return values


def native_schedule(parser, args):
    if len(args.peer_elf) > (3 if args.policy == ["amc"] else 2):
        parser.error("too many peer ELFs for the selected policy")
    if (args.peer_elf or args.policy) and not (args.source or args.elf):
        parser.error("peers and scheduling policies require an ordinary --source or --elf")
    for name in ("policy", "priority", "weight", "job", "periodic", "reservation", "quantum_ticks", "primary_class", "allotment_ticks", "boost_ticks"):
        values = getattr(args, name)
        if len(values) > 1:
            option = "class" if name == "primary_class" else name.replace("_", "-")
            parser.error(f"--{option} may appear only once")
    policy = args.policy[0] if args.policy else None
    classes = args.primary_class + args.peer_class
    if policy == "mlq":
        if not args.primary_class or len(args.peer_class) != len(args.peer_elf):
            parser.error("MLQ requires one class per supplied ELF")
        if any(level > 255 for level in classes):
            parser.error("MLQ classes must be in 0..255")
    elif classes:
        parser.error("class options require --policy mlq")
    if policy == "mlfq":
        if not args.allotment_ticks or not args.boost_ticks:
            parser.error("MLFQ requires both --allotment-ticks and --boost-ticks")
        if not 1 <= args.boost_ticks[0] < 2**63:
            parser.error("boost period must be in 1..2^63-1")
    elif args.allotment_ticks or args.boost_ticks:
        parser.error("allotment and boost options require --policy mlfq")
    if policy in ("rm", "dm"):
        if not args.periodic or len(args.peer_periodic) != len(args.peer_elf):
            parser.error("RM and DM require one periodic tuple per supplied ELF")
    elif args.periodic or args.peer_periodic:
        parser.error("periodic tuples require --policy rm or dm")
    if policy == "cbs":
        if not args.reservation or len(args.peer_reservation) != len(args.peer_elf):
            parser.error("CBS requires one reservation per supplied ELF")
    elif args.reservation or args.peer_reservation:
        parser.error("reservations require --policy cbs")
    if policy in ("edf", "llf"):
        if not args.job or len(args.peer_job) != len(args.peer_elf):
            parser.error("EDF and LLF require exactly one job tuple per supplied ELF")
    elif args.job or args.peer_job:
        parser.error("job tuples require --policy edf or llf")
    if args.threads == ["4"]:
        if policy not in ("fifo", "rr") or args.thread_priorities is None:
            parser.error("--threads 4 requires FIFO or RR and --thread-priorities")
    elif args.thread_priorities is not None:
        parser.error("--thread-priorities requires --threads 4")
    if args.threads == ["2"] and policy not in (None, "rr"):
        parser.error("thread scheduling supports only cooperative or RR")
    if policy in ("fifo", "rr") and not args.threads:
        if not args.priority or len(args.peer_priority) != len(args.peer_elf):
            parser.error("fixed scheduling requires exactly one priority per supplied ELF")
    elif args.priority or args.peer_priority:
        parser.error("application priorities require process-level FIFO or RR")
    priorities = args.priority + args.peer_priority
    if any(value < 0 or value > 255 for value in priorities):
        parser.error("priorities must be in 0..255")
    weight_fields = args.weight + args.peer_weight
    try:
        weights = [unsigned_decimal(v) if policy in ("lottery", "stride") else int(v) for v in weight_fields]
    except (ValueError, argparse.ArgumentTypeError) as exc:
        parser.error(str(exc))
    if policy in ("weighted", "lottery", "stride"):
        if not args.weight or len(args.peer_weight) != len(args.peer_elf):
            parser.error("weighted scheduling requires exactly one weight per supplied ELF")
        if policy in ("lottery", "stride") and any(not 1 <= value <= 16 for value in weights):
            parser.error("lottery and stride weights require 1..16")
        if any(not 1 <= value <= 2**32 - 1 for value in weights):
            parser.error("weights must be in 1..2^32-1")
    elif weights:
        parser.error("application weights require --policy weighted")
    if bool(args.quantum_ticks) != (policy in ("rr", "weighted", "mlq", "lottery", "stride")):
        parser.error("--quantum-ticks is required exclusively with --policy rr, weighted or mlq")
    quantum = None
    if args.quantum_ticks:
        value = args.quantum_ticks[0]
        if (policy in ("mlq", "lottery", "stride") or args.threads) and not (value.isascii() and value.isdecimal()):
            parser.error("quantum requires unsigned decimal digits")
        try:
            quantum = int(value)
        except ValueError:
            parser.error("invalid --quantum-ticks integer")
    if quantum is not None and not 1 <= quantum <= 2**63 - 1:
        parser.error("--quantum-ticks must be in 1..2^63-1")
    return policy, priorities, weights, quantum


def amc_manifest(source, slots):
    doc = json.loads(source.decode("utf-8"), object_pairs_hook=unique_keys)
    if type(doc) is not dict or set(doc) not in ({"tasks"}, {"tasks", "resets"}):
        raise ValueError("expected tasks and optional resets")
    tasks, resets = doc["tasks"], doc.get("resets", [])
    if type(tasks) is not list or len(tasks) != slots or type(resets) is not list or len(resets) > 16:
        raise ValueError("supply one task per slot and at most 16 resets")
    lines = ["amc-v1"]
    keys = ("priority", "c_lo", "c_hi", "deadline", "period", "offset", "generations")
    max_cell = (2**64 - 1) // 3125000  # deployment frequency 62.5 MHz, Q=F/20
    for task in tasks:
        if type(task) is not dict or set(task) != {"tier", "argv", *keys}:
            raise ValueError("incorrect AMC task fields")
        if any(type(task[k]) is not int or not 0 <= task[k] < 2**64 for k in keys):
            raise ValueError("AMC task parameters must be unsigned u64 integers")
        priority, lo, hi, deadline, period, offset, count = (task[k] for k in keys)
        tier = task["tier"]
        if (tier not in ("LO", "HI") or not 1 <= priority <= 255 or not 0 < lo <= hi <= deadline <= period
                or (tier == "LO" and lo != hi) or not 1 <= count <= 16
                or offset + (count - 1) * period + deadline > max_cell):
            raise ValueError("invalid AMC parameters or unrepresentable deployment timeline")
        if type(task["argv"]) is not list or len(task["argv"]) != count:
            raise ValueError("supply one argv vector for every generation")
        lines.append(" ".join(map(str, ("task", tier, priority, lo, hi, deadline, period, offset, count))))
        for argv in task["argv"]:
            if type(argv) is not list or len(argv) > 15 or any(type(a) is not str or "\0" in a for a in argv):
                raise ValueError("expected up to 15 NUL-free strings per generation")
            args = [a.encode("utf-8") for a in argv]
            if sum(len(a) + 1 for a in args) > 1024:
                raise ValueError("generation arguments exceed 1024 bytes including terminators")
            wire = bytes([len(argv)]) + b"".join(len(a).to_bytes(2, "little") + a for a in args)
            lines.append("argv " + wire.hex())
    previous = -1
    for reset in resets:
        if (type(reset) is not dict or set(reset) != {"cell", "approved"} or type(reset["approved"]) is not bool
                or type(reset["cell"]) is not int or not previous < reset["cell"] <= max_cell):
            raise ValueError("resets require strictly increasing representable cells and explicit boolean approval")
        previous = reset["cell"]
        lines.append(f"reset {previous} {int(reset['approved'])}")
    return {"tasks": tasks, "resets": resets}, ("\n".join(lines) + "\n").encode("ascii")


def encode_arguments(parser, args):
    vectors = [args.arg, args.peer_arg, args.peer2_arg]
    encoded = {}
    for slot, extras in enumerate(vectors):
        if not extras:
            continue
        if not (args.source or args.elf) or slot > len(args.peer_elf):
            parser.error(f"slot {slot}: arguments require a supplied ordinary application")
        if len(extras) > 15:
            parser.error(f"slot {slot}: at most 15 extra arguments")
        wire = bytearray([len(extras)])
        used = 0
        for value in extras:
            if "\0" in value:
                parser.error(f"slot {slot}: an argument contains NUL")
            try:
                data = value.encode("utf-8")
            except UnicodeEncodeError:
                parser.error(f"slot {slot}: arguments must be valid UTF-8")
            used += len(data) + 1
            if used > 1024:
                parser.error(f"slot {slot}: extra arguments exceed 1024 bytes including NULs")
            wire.extend(len(data).to_bytes(2, "little"))
            wire.extend(data)
        encoded[f"JNOX_NATIVE_ARGV{slot}_HEX"] = wire.hex()
    return vectors, encoded


def package_pi4_image(out, kernel, env):
    nm = "aarch64-linux-gnu-nm"
    objcopy = "aarch64-linux-gnu-objcopy"
    names = ("_start", "__native_payload_lma", "__native_payload_start",
             "pi4_native_payload_entry", "__native_payload_copy_end")
    symbols = {}
    for line in output([nm, "-n", str(kernel)], env=env).splitlines():
        fields = line.split()
        if len(fields) == 3 and fields[2] in names:
            symbols[fields[2]] = int(fields[0], 16)
    if (symbols["_start"], symbols["__native_payload_lma"],
        symbols["__native_payload_start"], symbols["pi4_native_payload_entry"]) != (
            0x80000, 0x81000, 0x02000000, 0x02000000):
        raise ValueError("unexpected Pi native load or entry layout")
    image = out / "kernel8.img"
    command = [objcopy, "-O", "binary", str(kernel), str(image)]
    subprocess.run(command, cwd=ROOT, env=env, check=True)
    size = image.stat().st_size
    copy_size = symbols["__native_payload_copy_end"] - symbols["__native_payload_start"]
    if copy_size <= 0 or size != 0x1000 + copy_size:
        raise ValueError("raw Pi image extent differs from initialized payload")
    return {"kernel8_img": image.name, "kernel8_img_sha256": sha256(image),
            "kernel8_img_bytes": size, "load_address": symbols["_start"],
            "entry_address": symbols["_start"], "runtime_address": symbols["__native_payload_start"],
            "link_symbols": symbols, "objcopy_command": command,
            "objcopy": output([objcopy, "--version"], env=env), "nm": output([nm, "--version"], env=env)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, action=SinglePath, help="new package directory")
    parser.add_argument("--board", choices=["virt", "pi4"], action=SinglePath)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--source", type=Path, action=SinglePath, help="C application to compile with the native SDK")
    group.add_argument("--elf", type=Path, action=SinglePath, help="already linked native AArch64 application")
    parser.add_argument("--peer-elf", type=Path, action="append", default=[],
                        help="independently linked peer in slot order (up to two)")
    parser.add_argument("--policy", choices=["fifo", "rr", "weighted", "edf", "llf", "rm", "dm", "cbs", "mlq", "mlfq", "partition", "polling", "deferrable", "sporadic", "hierarchy", "lottery", "stride", "amc"], action="append", default=[])
    parser.add_argument("--server-slot-ms", type=unsigned_decimal, action=SinglePath)
    parser.add_argument("--server-budget", type=budget_tuple, action=SinglePath, metavar="Q,P")
    parser.add_argument("--server-credits", type=unsigned_decimal, action=SinglePath)
    parser.add_argument("--amc-manifest", type=Path, action=SinglePath, metavar="DEPLOYMENT.json")
    parser.add_argument("--hierarchy", type=Path, action=SinglePath, metavar="TOPOLOGY.json")
    parser.add_argument("--partition-windows", action=SinglePath, metavar="OWNER:MS,...")
    parser.add_argument("--priority", type=int, action="append", default=[], help="primary priority")
    parser.add_argument("--peer-priority", type=int, action="append", default=[], help="in peer order")
    parser.add_argument("--lottery-seed", type=unsigned_decimal, action=SinglePath)
    parser.add_argument("--lottery-window", action=SinglePath, metavar="H,W")
    parser.add_argument("--lottery-floors", action=SinglePath, metavar="COUNT,...")
    parser.add_argument("--weighted-continuous", action="count", default=0,
                        help="apply recurring leases even with one runnable weighted process")
    parser.add_argument("--weighted-notify", type=weighted_notification, action=SinglePath,
                        metavar="closures:N|wakes:N")
    parser.add_argument("--weight", action="append", default=[], help="primary CPU service weight")
    parser.add_argument("--peer-weight", action="append", default=[], help="CPU service weights in peer order")
    parser.add_argument("--quantum-ticks", action="append", default=[], help="common RR, weighted or MLQ grant in physical counter ticks")
    parser.add_argument("--job", type=deadline_job, action="append", default=[], metavar="R,D,C",
                        help="EDF/LLF primary release, absolute deadline and cost in 100 ms units")
    parser.add_argument("--peer-job", type=deadline_job, action="append", default=[], metavar="R,D,C",
                        help="EDF/LLF job tuple for each peer, in peer order")
    parser.add_argument("--periodic", type=periodic_parameters, action="append", default=[], metavar="PHASE,T,D",
                        help="primary phase, period and relative deadline in nanoseconds")
    parser.add_argument("--peer-periodic", type=periodic_parameters, action="append", default=[], metavar="PHASE,T,D",
                        help="periodic parameters in peer order")
    parser.add_argument("--reservation", type=budget_tuple, action="append", default=[], metavar="Q,P",
                        help="primary CBS quota and period in 50 ms slots")
    parser.add_argument("--peer-reservation", type=budget_tuple, action="append", default=[], metavar="Q,P",
                        help="CBS quota and period in 50 ms slots, in peer order")
    parser.add_argument("--class", dest="primary_class", type=unsigned_decimal, action="append", default=[])
    parser.add_argument("--peer-class", type=unsigned_decimal, action="append", default=[])
    parser.add_argument("--allotment-ticks", type=allotments, action="append", default=[], metavar="TOP,MIDDLE,BOTTOM")
    parser.add_argument("--boost-ticks", type=unsigned_decimal, action="append", default=[])
    for slot, option in enumerate(("--arg", "--peer-arg", "--peer2-arg")):
        parser.add_argument(option, action="append", default=[], metavar="VALUE",
                            help=f"append one UTF-8 argument to slot {slot} (use {option}=-name for a leading dash)")
    parser.add_argument("--thread-priorities", type=thread_priorities, action=SinglePath)
    parser.add_argument("--threads", choices=["2", "4"], action="append", default=[],
                        help="thread context capacity for one ordinary ELF")
    parser.add_argument("--deadline-service-elf", type=Path, action=SinglePath)
    parser.add_argument("--deadline-service-cells", action="append", default=[], metavar="CELL,...")
    parser.add_argument("--deadline-switch-cap", type=unsigned_decimal, action="append", default=[])
    args = parser.parse_args()
    if len(args.threads) > 1:
        parser.error("--threads may appear only once")
    if args.threads and (not (args.elf or args.source) or args.peer_elf):
        parser.error("thread scheduling requires exactly one ordinary application")
    policy, priorities, weights, quantum = native_schedule(parser, args)
    if (args.lottery_seed is not None) != (policy == "lottery"):
        parser.error("--lottery-seed is required exclusively for lottery")
    if args.lottery_seed is not None and args.lottery_seed >= 2**64:
        parser.error("lottery seed must fit u64")
    if (args.lottery_window is None) != (args.lottery_floors is None):
        parser.error("lottery window and floors must be supplied together")
    if args.lottery_window is not None:
        if policy != "lottery":
            parser.error("lottery window requires lottery scheduling")
        try:
            horizon, width = map(unsigned_decimal, args.lottery_window.split(","))
            floors = list(map(unsigned_decimal, args.lottery_floors.split(",")))
            if not 1 <= width <= horizon <= 64 or len(floors) != len(weights) or sum(floors) > width:
                raise ValueError("require 1 <= W <= H <= 64 and one floor per slot summing to at most W")
            forecast(weights, args.lottery_seed, horizon, width, floors)
        except (ValueError, argparse.ArgumentTypeError) as exc:
            parser.error(str(exc))
    board = args.board or "virt"
    if args.weighted_continuous > 1:
        parser.error("--weighted-continuous may appear only once")
    if args.weighted_continuous and (policy != "weighted" or board != "virt"):
        parser.error("--weighted-continuous requires weighted scheduling on virt")
    if args.weighted_notify is not None and not args.weighted_continuous:
        parser.error("--weighted-notify requires --weighted-continuous")
    if board == "pi4" and (policy not in ("fifo", "rr") or len(args.peer_elf) != 1
                           or not (args.source or args.elf) or args.threads):
        parser.error("pi4 requires a primary application and one peer under FIFO or RR")
    server_policy = policy in ("polling", "deferrable", "sporadic")
    if server_policy:
        if len(args.peer_elf) != 1 or args.server_budget is None or args.server_slot_ms is None:
            parser.error("native servers require two applications, --server-budget and --server-slot-ms")
        quota, period = args.server_budget
        if not 0 < args.server_slot_ms * period < 2**64:
            parser.error("server slot and period must fit in positive u64 milliseconds")
        if (args.server_credits is not None) != (policy == "sporadic"):
            parser.error("--server-credits is required exclusively for sporadic")
        if policy == "sporadic" and not 1 <= args.server_credits <= 8:
            parser.error("sporadic credit capacity must be in 1..8")
    elif any(v is not None for v in (args.server_slot_ms, args.server_budget, args.server_credits)):
        parser.error("server options require polling, deferrable or sporadic")
    if policy == "partition":
        if len(args.peer_elf) != 1 or args.partition_windows is None:
            parser.error("partition requires two applications and --partition-windows")
        windows, frame_ms = partition_table(parser, args.partition_windows)
    elif args.partition_windows is not None:
        parser.error("--partition-windows requires --policy partition")
    vectors, encoded_arguments = encode_arguments(parser, args)
    service = args.deadline_service_elf
    cells = []
    if service or args.deadline_service_cells or args.deadline_switch_cap:
        if not service or len(args.deadline_service_cells) != 1 or len(args.deadline_switch_cap) != 1:
            parser.error("specify one service ELF, calendar and switch cap together")
        if policy not in ("edf", "llf") or len(args.peer_elf) != 1:
            parser.error("a recurring service requires two ordinary EDF or LLF jobs")
        if not 0 < args.deadline_switch_cap[0] < 2**64:
            parser.error("switch cap must be a positive u64")
        fields = args.deadline_service_cells[0].split(",")
        if not 1 <= len(fields) <= 16 or any(not (v.isascii() and v.isdecimal()) for v in fields):
            parser.error("service cells must be a list of decimal indices")
        cells = list(map(int, fields))
        if cells != sorted(set(cells)) or cells[-1] > 15:
            parser.error("service cells must increase strictly within 0..15")
        calendar = ",".join(map(str, cells))
        vectors[2] = [calendar]
    if (args.amc_manifest is not None) != (policy == "amc"):
        parser.error("--amc-manifest is required exclusively with --policy amc")
    if policy == "amc":
        if any(vectors):
            parser.error("declare all AMC generation arguments in --amc-manifest")
        try:
            amc_path = args.amc_manifest.resolve(strict=True)
            amc_source = amc_path.read_bytes()
            amc, amc_wire = amc_manifest(amc_source, 1 + len(args.peer_elf))
            vectors = [task["argv"][0] for task in amc["tasks"]]
        except (OSError, ValueError, RecursionError) as exc:
            parser.error(f"invalid --amc-manifest: {exc}")
    if (args.hierarchy is not None) != (policy == "hierarchy"):
        parser.error("--hierarchy is required exclusively with --policy hierarchy")
    if policy == "hierarchy":
        try:
            hierarchy_path = args.hierarchy.resolve(strict=True)
            hierarchy_source = hierarchy_path.read_bytes()
            topology, raw_topology = parse_topology(hierarchy_source, 1 + len(args.peer_elf))
        except (ValueError, OSError, RecursionError) as exc:
            parser.error(f"invalid --hierarchy: {exc}")
    peer_inputs = [path.resolve(strict=True) for path in args.peer_elf]
    if service:
        peer_inputs.append(service.resolve(strict=True))
    peer_digests = [sha256(path) for path in peer_inputs]
    app_input = (args.elf or args.source or SOURCE).resolve(strict=True)
    input_digest = sha256(app_input)
    app = "process" if args.source or args.elf else "startup-probe"
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    user_elf = out / "user.elf"
    kernel_elf = out / "kernel.elf"
    env = {k: v for k, v in os.environ.items() if not k.startswith(("JNOX_", "CARGO_FEATURE_"))
           and k not in ("CARGO_BUILD_TARGET", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
    commit = output(["git", "rev-parse", "HEAD"])
    dirty = output(["git", "status", "--porcelain", "--untracked-files=no"])
    sdk_files = sorted(p for p in SDK.rglob("*") if p.is_file())
    inputs = {str(p.relative_to(ROOT)): sha256(p) for p in sdk_files if not args.elf}
    input_name = str(app_input.relative_to(ROOT) if app_input.is_relative_to(ROOT) else app_input)
    inputs[input_name] = input_digest
    user_command = None if args.elf else ["sh", str(SDK / "build.sh"), str(app_input), str(user_elf)]
    kernel_command = [sys.executable, "tools/build_kernel.py", "--arch", "aarch64"]
    metadata = {
        "format": 1, "profile": "native", "app": app, "board": board,
        "commit": commit, "tracked_changes": dirty, "source_sha256": inputs,
        "input": {"kind": "elf" if args.elf else "c", "path": input_name, "sha256": input_digest},
        "compiler": None if args.elf else output(["clang", "--version"], env=env),
        "linker": None if args.elf else output(["ld.lld", "--version"], env=env),
        "rust": output(["rustc", "-vV"], ROOT / "kernel", env),
        "cargo": output(["cargo", "--version"], ROOT / "kernel", env),
        "sdk_command": user_command, "kernel_command": kernel_command,
    }
    if policy == "amc":
        canonical = out / "amc-v1.txt"
        canonical.write_bytes(amc_wire)
        digest = hashlib.sha256(amc_wire).hexdigest()
        inputs[str(amc_path)] = hashlib.sha256(amc_source).hexdigest()
        metadata.update(amc=amc, amc_input={"path": str(amc_path), "sha256": inputs[str(amc_path)]},
                        amc_canonical={"file": canonical.name, "sha256": digest},
                        timing_unit="cells", package_argv=sys.argv[:])
        env.update(JNOX_NATIVE_AMC_MANIFEST=str(canonical), JNOX_NATIVE_AMC_SHA256=digest)
    if policy == "hierarchy":
        name = str(hierarchy_path.relative_to(ROOT) if hierarchy_path.is_relative_to(ROOT) else hierarchy_path)
        digest = hashlib.sha256(hierarchy_source).hexdigest()
        inputs[name] = digest
        metadata.update(hierarchy=topology, timing_unit="physical_counter_ticks",
                        hierarchy_input={"path": name, "sha256": digest}, package_argv=sys.argv[:])
        env["JNOX_NATIVE_HIERARCHY"] = raw_topology
    with (out / "build.log").open("w", encoding="utf-8") as log:
        if args.elf:
            shutil.copyfile(app_input, user_elf)
        else:
            subprocess.run(user_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                           check=True)
        if sha256(user_elf if args.elf else app_input) != input_digest:
            raise ValueError("application input changed during packaging")
        digest = sha256(user_elf)
        slots = [{"slot": 0, "elf": user_elf.name, "elf_sha256": digest, "input": metadata["input"]}]
        for slot, (peer_input, peer_digest) in enumerate(zip(peer_inputs, peer_digests), 1):
            suffix = "" if slot == 1 else str(slot)
            peer_elf = out / f"native_peer{suffix}.elf"
            shutil.copyfile(peer_input, peer_elf)
            if sha256(peer_elf) != peer_digest:
                raise ValueError(f"peer ELF in slot {slot} changed during packaging")
            peer_name = str(peer_input.relative_to(ROOT) if peer_input.is_relative_to(ROOT) else peer_input)
            inputs[peer_name] = peer_digest
            peer = {"kind": "elf", "path": peer_name, "sha256": peer_digest}
            slots.append({"slot": slot, "elf": peer_elf.name, "elf_sha256": peer_digest, "input": peer})
            metadata[f"peer{suffix}_input"] = peer
            metadata[f"peer{suffix}_elf_sha256"] = peer_digest
            var = f"JNOX_NATIVE_PEER{suffix}_ELF"
            if slot == 2 and service:
                var = "JNOX_NATIVE_DEADLINE_SERVICE_ELF"
            env[var], env[var + "_SHA256"] = str(peer_elf), peer_digest
        periodic = args.periodic + args.peer_periodic
        reservations = args.reservation + args.peer_reservation
        classes = args.primary_class + args.peer_class
        jobs = args.job + args.peer_job
        for slot in slots:
            if policy == "amc":
                slot["amc_task"] = amc["tasks"][slot["slot"]]
            slot["argv"] = (["/bin/app", *vectors[slot["slot"]]] if app == "process"
                            else ["/bin/native_startup_probe", "8", "13"])
            slot["priority"] = priorities[slot["slot"]] if priorities else None
            if weights:
                slot["weight"] = weights[slot["slot"]]
            if periodic:
                phase, period, deadline = periodic[slot["slot"]]
                slot["periodic"] = {"phase_ns": phase, "period_ns": period, "relative_deadline_ns": deadline}
            if reservations:
                quota, period = reservations[slot["slot"]]
                slot["reservation"] = {"quota_slots": quota, "period_slots": period}
            if classes:
                slot["class"] = classes[slot["slot"]]
            if jobs and slot["slot"] < len(jobs):
                release, deadline, cost = jobs[slot["slot"]]
                slot["job"] = {"release": release, "deadline": deadline, "cost": cost}
        if jobs:
            metadata[f"{policy}_profile"] = {"name": "finite", "grid_ms": 100, "horizon_grids": 16}
            env["JNOX_NATIVE_EDF_JOBS"] = ";".join(",".join(map(str, job)) for job in jobs)
        if service:
            metadata["deadline_service"] = {"slot": 2, "elf_sha256": peer_digests[1],
                                            "cells": cells, "mask": sum(1 << cell for cell in cells),
                                            "switch_cap": args.deadline_switch_cap[0]}
            env["JNOX_NATIVE_DEADLINE_SERVICE_CELLS"] = calendar
            env["JNOX_NATIVE_DEADLINE_SWITCH_CAP"] = str(args.deadline_switch_cap[0])
        if periodic:
            env["JNOX_NATIVE_PERIODIC"] = ";".join(",".join(map(str, item)) for item in periodic)
        if reservations:
            metadata["reservation_slot_ms"] = 50
            env["JNOX_NATIVE_RESERVATIONS"] = ";".join(f"{quota},{period}" for quota, period in reservations)
        if policy in ("mlq", "mlfq"):
            metadata.update(timing_unit="physical_counter_ticks", package_argv=sys.argv[:])
        if classes:
            env["JNOX_NATIVE_CLASSES"] = ",".join(map(str, classes))
        if policy == "mlfq":
            metadata.update(allotment_ticks=args.allotment_ticks[0], boost_ticks=args.boost_ticks[0])
            env["JNOX_NATIVE_ALLOTMENT_TICKS"] = ",".join(map(str, args.allotment_ticks[0]))
            env["JNOX_NATIVE_BOOST_TICKS"] = str(args.boost_ticks[0])
        if args.threads:
            metadata["thread_capacity"] = int(args.threads[0])
            if policy == "rr":
                metadata.update(scheduling_scope="within_process", timing_unit="physical_counter_ticks")
            env["JNOX_NATIVE_THREADS"] = args.threads[0]
            if args.thread_priorities is not None:
                metadata.update(scheduling_scope="within_process", thread_slots=[
                    {"slot": i, "base_priority": priority}
                    for i, priority in enumerate(args.thread_priorities)
                ])
                env["JNOX_NATIVE_THREAD_PRIORITIES"] = ",".join(map(str, args.thread_priorities))
        if policy == "partition":
            env["JNOX_NATIVE_PARTITION_WINDOWS"] = args.partition_windows
            metadata.update(partition_windows=windows, partition_frame_ms=frame_ms,
                            partition_windows_input=args.partition_windows)
        if server_policy:
            env.update(JNOX_NATIVE_SERVER_SLOT_MS=str(args.server_slot_ms),
                       JNOX_NATIVE_SERVER_BUDGET=f"{quota},{period}")
            metadata.update(server_slot_ms=args.server_slot_ms, server_quota_slots=quota,
                            server_period_slots=period, server_credits=args.server_credits)
            if policy == "sporadic":
                env["JNOX_NATIVE_SERVER_CREDITS"] = str(args.server_credits)
        if args.weighted_continuous:
            env["JNOX_NATIVE_WEIGHTED_CONTINUOUS"] = "1"
            metadata.update(weighted_continuous=True, weighted_notify=None)
            if args.weighted_notify is not None:
                mode, count = args.weighted_notify
                env["JNOX_NATIVE_WEIGHTED_NOTIFY"] = f"{mode}:{count}"
                metadata["weighted_notify"] = {"mode": mode, "count": count}
        if policy in ("lottery", "stride"):
            metadata.update(timing_unit="physical_counter_ticks", all_ready_mask=(1 << len(weights)) - 1,
                            algorithm="SplitMix64 rejection sampling" if policy == "lottery" else "stride",
                            guarantee="conditional full-grant count" if args.lottery_window is not None
                                      else "seeded selection" if policy == "lottery" else "service-charged stride")
        if policy == "lottery":
            env["JNOX_NATIVE_LOTTERY_SEED"] = str(args.lottery_seed)
            metadata.update(lottery_seed=args.lottery_seed, lottery_window=None, lottery_floors=None)
            if args.lottery_window is not None:
                env["JNOX_NATIVE_LOTTERY_WINDOW"] = f"{horizon},{width}"
                env["JNOX_NATIVE_LOTTERY_FLOORS"] = ",".join(map(str, floors))
                metadata.update(lottery_window=[horizon, width], lottery_floors=floors)
        metadata.update(policy=policy or "cooperative", quantum_ticks=quantum, slots=slots)
        if policy:
            env["JNOX_NATIVE_POLICY"] = policy
        if priorities:
            env["JNOX_NATIVE_PRIORITIES"] = ",".join(map(str, priorities))
        if weights:
            env["JNOX_NATIVE_WEIGHTS"] = ",".join(map(str, weights))
        if quantum is not None:
            env["JNOX_NATIVE_QUANTUM_TICKS"] = str(quantum)
        env.update(encoded_arguments)
        env.update(JNOX_BOARD=board, JNOX_PROFILE="native", JNOX_APP=app,
                   JNOX_NATIVE_ELF=str(user_elf), JNOX_NATIVE_ELF_SHA256=digest,
                   CARGO_TARGET_DIR=str(out / "target"))
        subprocess.run(kernel_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                       check=True)
    shutil.copyfile(out / "target" / TARGET / "release/jnox-kernel", kernel_elf)
    if board == "pi4":
        metadata.update(package_pi4_image(out, kernel_elf, env), package_argv=sys.argv[:])
    metadata.update(user_elf_sha256=digest, kernel_elf_sha256=sha256(kernel_elf),
                    build_log_sha256=sha256(out / "build.log"))
    (out / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"kernel {metadata['kernel_elf_sha256']}  {kernel_elf}")
    print(f"native {digest}  {user_elf}")
    for slot in slots[1:]:
        print(f"peer {slot['elf_sha256']}  {out / slot['elf']}")


if __name__ == "__main__":
    main()
