#!/usr/bin/env python3
"""Package up to five native AArch64 applications for virt or Raspberry Pi 4."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import re
import shutil
import subprocess
import sys

from build_kernel import add_kernel_profile, cargo_command, read_registry
from native_shell_rom import shell_package
from native_extension_program import parse_program
from native_amc_roots import linked_roots
from native_hierarchy import parse_topology, unique_keys
from native_lottery_admission import forecast

ROOT = Path(__file__).resolve().parents[1]
SDK = ROOT / "sdk/native-aarch64"
SOURCE = ROOT / "examples/portability/native_startup_probe.c"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def output(args, cwd=ROOT, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True).strip()


class SinglePath(argparse.Action):
    def __call__(self, parser, namespace, value, option_string=None):
        if getattr(namespace, self.dest) is not None:
            parser.error(f"{option_string} may appear only once")
        setattr(namespace, self.dest, value)


def batch_job(value):
    fields = value.split(",")
    if len(fields) != 2:
        raise argparse.ArgumentTypeError("expected offset,estimate in physical counter ticks")
    offset, estimate = map(unsigned_decimal, fields)
    if not (0 <= offset < 2**63 and 1 <= estimate < 2**63):
        raise argparse.ArgumentTypeError("batch offset must be 0..2^63-1 and estimate 1..2^63-1")
    return offset, estimate


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


def topic_grant(value):
    parts = value.split(":")
    if len(parts) != 2 or parts[0] not in ("0", "1", "2"):
        raise argparse.ArgumentTypeError("expected client SLOT:MASK for slot 0, 1 or 2")
    mask = parts[1]
    base = 16 if mask.startswith("0x") else 10
    digits = mask[2:] if base == 16 else mask
    allowed = "0123456789abcdefABCDEF" if base == 16 else "0123456789"
    if not digits or any(c not in allowed for c in digits) or int(digits, base) > 15:
        raise argparse.ArgumentTypeError("topic mask must be 0..15 (decimal or 0x hex)")
    return int(parts[0]), int(digits, base)


def topic_vector(parser, grants):
    masks = [0, 0, 0]
    seen = set()
    for slot, mask in grants:
        if slot in seen:
            parser.error("duplicate client slot in topic grants for one direction")
        seen.add(slot)
        masks[slot] = mask
    return "".join(format(mask, "x") for mask in masks)


def ipc_route(value):
    if len(value) != 3 or value[1] != ":" or value[0] not in "01234" or value[2] not in "01234":
        raise argparse.ArgumentTypeError("expected two colon-separated slots 0 through 4")
    return int(value[0]), int(value[2])


def cap_grant(value):
    fields = value.split(":")
    if len(fields) != 3 or fields[0] not in ("0", "1", "2") or fields[1] not in ("0", "1", "2") or fields[2] not in ("1", "2", "3"):
        raise argparse.ArgumentTypeError("expected OWNER:TARGET:RIGHTS with slots 0..2 and rights 1..3")
    return tuple(map(int, fields))


def unsigned_decimal(value):
    if not (value.isascii() and value.isdecimal()):
        raise argparse.ArgumentTypeError("expected unsigned decimal digits")
    return int(value)


def pipe_fields(value):
    return tuple(unsigned_decimal(field) for field in value.split(":"))


def tmpfs_grant(value):
    match = re.fullmatch(r"([0-4]):(ro|rw)", value)
    if not match:
        raise argparse.ArgumentTypeError("expected SLOT:ro or SLOT:rw")
    return int(match[1]), match[2]


def service_account(value):
    fields = pipe_fields(value)
    if len(fields) != 2 or not all(1 <= item <= 65534 for item in fields):
        raise argparse.ArgumentTypeError("expected UID:GID, each in 1..65534")
    return fields


def native_service_slots(parser, args, policy, board):
    if len(args.socket_service) > 1 or len(args.display_service) > 1:
        parser.error("--socket-service and --display-service may each appear only once")
    if args.net_service and (len(args.net_service) != 1 or args.net_service != args.socket_service):
        parser.error("--net-service must appear once and match --socket-service")
    slots = 1 + len(args.peer_elf)
    net_standby = getattr(args, "net_standby", None)
    if net_standby is not None and (not args.net_service or net_standby in args.net_service
                                    or int(net_standby) >= slots):
        parser.error("--net-standby requires a distinct packaged slot and --net-service")
    if args.net_service and slots > 3:
        parser.error("--net-service supports at most three native processes")
    display = int(args.display_service[0]) if args.display_service else None
    if getattr(args, "pointer_tablet", False) and display is None:
        parser.error("--pointer-tablet requires --display-service")
    broker = int(args.socket_service[0]) if args.socket_service else None
    if args.shared_fd and (args.shared_fd != 1 or broker is None or args.net_service
                           or board != "virt" or policy not in ("fifo", "rr") or args.threads):
        parser.error("--shared-fd requires one native AF_UNIX socket broker on virt FIFO or RR")
    standby = args.uart_rx_standby
    if standby is not None:
        if standby != "2" or slots != 3 or display != 0 or args.uart_rx_irq != 1:
            parser.error("--uart-rx-standby 2 requires three processes, --display-service 0 and --uart-rx-irq")
        if any((args.ipc_route, args.mailbox_route, args.notify_route, args.cap_grant, args.cap_route,
                args.loan_route, args.pipes, args.waitset, args.socket_service, args.pubsub_service)):
            parser.error("--uart-rx-standby excludes IPC routes, endpoints and services")
    display_standby = args.display_standby
    active = getattr(args, "display_five_active", False)
    if active or display_standby is not None:
        if ((active and display_standby is not None) or slots != 5 or display != 0 or broker != 3 or args.shared_fd != 1
                or any((args.uart_rx_irq, args.exec_child, args.rom_file, args.mailbox_route, args.notify_route,
                        args.loan_route, args.cap_grant, args.cap_route, args.pipes, args.waitset))):
            parser.error("select one five-role profile with owner 0, broker 3, shared FDs and ordinary processes")
    elif slots > 4:
        parser.error("five native processes require --display-standby 2 or --display-five-active")
    if broker == 3:
        if slots != (5 if active or display_standby else 4) or display != 0 or args.pubsub_service:
            parser.error("socket broker 3 requires the selected display profile with owner 0")
        callers = [0, 1, 2, 4] if active else [0, 1, 2]
        if sorted(args.ipc_route) != [(pid, 3) for pid in callers]:
            parser.error(f"display/socket package requires exactly the broker routes from slots {callers}")
        mask = str(sum(1 << pid for pid in callers))
        if args.peer3_arg and args.peer3_arg != [mask]:
            parser.error(f"socket broker 3 requires client mask {mask}")
        args.peer3_arg = [mask]
    if display is not None and (board != "virt" or policy != "fifo" or args.threads
                                or display >= slots or (slots > 2 and broker != 3 and standby is None)):
        parser.error("--display-service requires an ordinary virt FIFO owner in one or two slots, or owner 0 with UART standby 2 or socket broker 3")
    if args.uart_rx_irq:
        if args.uart_rx_irq != 1 or display is None or args.exec_child or args.rom_file:
            parser.error("--uart-rx-irq must appear once and requires an ordinary --display-service package")
    clients = sorted(caller for caller, server in args.ipc_route if server == broker)
    if broker is not None:
        if slots > 3 and broker != 3 and not args.pubsub_service:
            parser.error("four native socket processes require display broker 3 or a separate pubsub service")
        if 3 in clients:
            parser.error("socket service accepts identities 0..2")
        if not clients:
            parser.error("--socket-service requires an explicit --ipc-route into its slot")
    return display, broker, clients


def validate_pipes(parser, args, policy, board):
    if not args.pipes:
        if args.fifo_grant or args.pipe_link:
            parser.error("--fifo-grant and --pipe-link require --pipes")
        return
    slots = len(args.peer_elf) + 1
    if args.pipes != 1 or board != "virt" or policy not in ("fifo", "rr") or args.threads or slots > 3:
        parser.error("--pipes requires one to three ordinary FIFO/RR processes on virt")
    owners = set()
    for grant in args.fifo_grant:
        if len(grant) != 3:
            parser.error("--fifo-grant requires OWNER:READERMASK:WRITERMASK")
        owner, readers, writers = grant
        if owner >= slots or owner in owners or not readers or not writers or (readers | writers) >= (1 << slots):
            parser.error("duplicate FIFO owner, empty grant, or slot outside package")
        owners.add(owner)
    if len(args.pipe_link) > 4:
        parser.error("at most four boot pipe links")
    occupied = set()
    references = [0] * slots
    for link in args.pipe_link:
        if len(link) != 4:
            parser.error("--pipe-link requires WRITER:WFD:READER:RFD")
        writer, wfd, reader, rfd = link
        for slot, fd in ((writer, wfd), (reader, rfd)):
            if slot >= slots or fd > 10 or (slot, fd) in occupied:
                parser.error("pipe link has an unavailable slot or descriptor")
            if references[slot] >= 8:
                parser.error("at most eight pipe descriptors per process")
            occupied.add((slot, fd))
            references[slot] += 1


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
    max_peers = 4 if args.display_standby is not None or getattr(args, "display_five_active", False) else 3
    if len(args.peer_elf) > (max_peers if args.policy in (["amc"], ["fifo"], ["rr"]) else 2):
        parser.error("too many peer ELFs for the selected policy")
    if (args.peer_elf or args.policy) and not (args.source or args.elf):
        parser.error("peers and scheduling policies require an ordinary --source or --elf")
    for name in ("policy", "priority", "weight", "job", "batch_job", "periodic", "reservation", "quantum_ticks", "primary_class", "allotment_ticks", "boost_ticks"):
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
    if policy in ("sjf", "srtf", "hrrn"):
        if not args.batch_job or len(args.peer_batch_job) != len(args.peer_elf):
            parser.error("batch scheduling requires one offset,estimate tuple per supplied application")
    elif args.batch_job or args.peer_batch_job:
        parser.error("batch job tuples require --policy sjf, srtf or hrrn")
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
    vectors = [args.arg, args.peer_arg, args.peer2_arg, args.peer3_arg, args.peer4_arg]
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


def encode_environment(parser, args, vectors, policy):
    entries = [{} for _ in vectors]
    for declaration in args.env:
        slot_text, sep, assignment = declaration.partition(":")
        name, eq, value = assignment.partition("=")
        if (not sep or slot_text not in ("0", "1", "2", "3", "4") or not eq
                or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name) or "\0" in value):
            parser.error("--env requires SLOT:NAME=VALUE with an ASCII identifier and a NUL-free value")
        slot = int(slot_text)
        if not (args.source or args.elf) or slot > len(args.peer_elf):
            parser.error(f"--env slot {slot} requires a supplied ordinary application")
        if policy == "amc" or (args.exec_child and slot > 0):
            parser.error("--env is unsupported for AMC generations and dynamically spawned children")
        if name in entries[slot]:
            parser.error(f"slot {slot}: duplicate environment name {name}")
        entries[slot][name] = value

    encoded = {}
    for slot, environment in enumerate(entries):
        if not environment:
            continue
        argv = vectors[slot]
        try:
            wire = b"".join(f"{name}={value}\0".encode("utf-8") for name, value in environment.items())
            used = len(wire) + sum(len(arg.encode("utf-8")) + 1 for arg in argv)
        except UnicodeEncodeError:
            parser.error(f"slot {slot}: startup strings must be valid UTF-8")
        if len(argv) + len(environment) > 15 or used > 1024:
            parser.error(f"slot {slot}: extra arguments and environment exceed 15 entries or 1024 bytes")
        encoded[f"JNOX_NATIVE_ENV{slot}_HEX"] = wire.hex()
    return entries, encoded


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
    add_kernel_profile(parser)
    parser.add_argument("--out", required=True, type=Path, action=SinglePath, help="new package directory")
    parser.add_argument("--board", choices=["virt", "pi4"], action=SinglePath)
    source_device = parser.add_mutually_exclusive_group()
    source_device.add_argument("--linux-source-rtc", dest="linux_source_driver", action="store_const", const="rtc",
                               help="grant the virt PL031 and INTID 34 to driver slot 0 in a two-process native FIFO package")
    source_device.add_argument("--linux-source-net", dest="linux_source_driver", action="store_const", const="net",
                               help="grant the virtio-net MMIO transport to driver slot 0 in a two-process native FIFO package")
    source_device.add_argument("--linux-source-net-queues", dest="linux_source_driver", action="store_const", const="net-queues",
                               help="allow driver slot 0 to set up and query a virtio-net RX/TX queue pair in the native FIFO package")
    source_device.add_argument("--linux-source-net-packets", dest="linux_source_driver", action="store_const", const="net-packets",
                               help="grant driver slot 0 native packet I/O on the virtio-net RX/TX queue pair")
    source_device.add_argument("--linux-source-net-rx", dest="linux_source_driver", action="store_const", const="net-rx",
                               help="grant slot 0 virtio-net receive buffers and connect it to the slot 1 consumer")
    source_device.add_argument("--linux-source-net-rx-poll", dest="linux_source_driver", action="store_const",
                               const="net-rx-poll", help="grant slot 0 RX callback control for upstream NAPI polling")
    parser.add_argument("--linux-source-net-rx-irq-rearm-test", action="store_true",
                        help="enable the test-only synthetic rearm event for the RX IRQ package")
    parser.add_argument("--linux-source-net-restart-pending-test", action="store_true",
                        help="enable the EL1 pending-retirement test barrier in a restart package")
    source_device.add_argument("--linux-source-net-lifecycle-failures", dest="linux_source_driver", action="store_const",
                               const="net-lifecycle-failures", help="package the two-slot lifecycle failure fixture")
    source_device.add_argument("--linux-source-net-duplex", dest="linux_source_driver", action="store_const",
                               const="net-duplex", help="grant slot 0 paired TX/RX on one original lifecycle netdev")
    source_device.add_argument("--linux-source-net-lifecycle", dest="linux_source_driver", action="store_const",
                               const="net-lifecycle", help="grant slot 0 the original virtio-net driver lifecycle")
    source_device.add_argument("--linux-source-net-restart", dest="linux_source_driver", action="store_const",
                               const="net-restart", help="package a primary IRQ driver, survivor and dormant spare")
    source_device.add_argument("--linux-source-net-rx-irq", dest="linux_source_driver", action="store_const",
                               const="net-rx-irq", help="grant slot 0 the virtio-net receive IRQ on virt")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--task-policy", type=Path, action=SinglePath, help="package a declarative virt task policy")
    group.add_argument("--source", type=Path, action=SinglePath, help="C application to compile with the native SDK")
    group.add_argument("--elf", type=Path, action=SinglePath, help="already linked native AArch64 application")
    parser.add_argument("--peer-elf", type=Path, action="append", default=[],
                        help="independently linked peer in slot order (four peers for five-slot display profiles, three for virt FIFO/RR or AMC, two otherwise)")
    parser.add_argument("--policy", choices=["fifo", "rr", "weighted", "edf", "llf", "rm", "dm", "cbs", "mlq", "mlfq", "partition", "polling", "deferrable", "sporadic", "hierarchy", "lottery", "stride", "sjf", "srtf", "hrrn", "extension", "amc"], action="append", default=[])
    parser.add_argument("--server-slot-ms", type=unsigned_decimal, action=SinglePath)
    parser.add_argument("--server-budget", type=budget_tuple, action=SinglePath, metavar="Q,P")
    parser.add_argument("--server-credits", type=unsigned_decimal, action=SinglePath)
    parser.add_argument("--extension-program", type=Path, action=SinglePath, metavar="RULE.txt")
    parser.add_argument("--amc-clock-page", action="count", default=0)
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
    parser.add_argument("--batch-job", type=batch_job, action="append", default=[], metavar="OFFSET,ESTIMATE",
                        help="primary initial ready offset and service estimate in physical counter ticks")
    parser.add_argument("--peer-batch-job", type=batch_job, action="append", default=[], metavar="OFFSET,ESTIMATE",
                        help="offset and estimate for each peer in ELF order, in physical counter ticks")
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
    for slot, option in enumerate(("--arg", "--peer-arg", "--peer2-arg", "--peer3-arg", "--peer4-arg")):
        parser.add_argument(option, action="append", default=[], metavar="VALUE",
                            help=f"append one UTF-8 argument to slot {slot} (use {option}=-name for a leading dash)")
    parser.add_argument("--env", action="append", default=[], metavar="SLOT:NAME=VALUE",
                        help="set one package environment entry for an ordinary process slot")
    parser.add_argument("--thread-priorities", type=thread_priorities, action=SinglePath)
    parser.add_argument("--threads", choices=["2", "4"], action="append", default=[],
                        help="thread context capacity for one ordinary ELF")
    parser.add_argument("--deadline-service-elf", type=Path, action=SinglePath)
    parser.add_argument("--deadline-service-cells", action="append", default=[], metavar="CELL,...")
    parser.add_argument("--deadline-switch-cap", type=unsigned_decimal, action="append", default=[])
    parser.add_argument("--ipc-route", type=ipc_route, action="append", default=[], metavar="CALLER:SERVER",
                        help="grant a directed synchronous call right between native FIFO/RR processes")
    parser.add_argument("--mailbox-route", type=ipc_route, action="append", default=[], metavar="CALLER:OWNER",
                        help="grant a send right to another native FIFO/RR process's mailbox")
    parser.add_argument("--notify-route", type=ipc_route, action="append", default=[], metavar="SENDER:OWNER",
                        help="grant a notification right to another native FIFO/RR process")
    parser.add_argument("--exec-child", action="append", default=[], metavar="SLOT:NAME",
                        help="reserve child executable slot 1 or 2 in a native shell package")
    parser.add_argument("--rom-file", action="append", default=[], metavar="NAME=FILE",
                        help="embed a host file under an immutable guest basename")
    parser.add_argument("--nodename", help="one explicit ASCII node label for all native FIFO/RR processes")
    parser.add_argument("--service-account", type=service_account, action=SinglePath, metavar="UID:GID",
                        help="unprivileged account for every native FIFO/RR process in this package (default 65534:65534)")
    parser.add_argument("--boot-utc-seconds", type=int, metavar="SECONDS",
                        help="provision signed POSIX UTC seconds at the virt FIFO/RR monotonic origin")
    parser.add_argument("--console-write", action="append", type=int, choices=range(5), default=[],
                        help="grant this process slot bounded SVC 4 console output on fd 1 and 2")
    parser.add_argument("--pipes", action="count", default=0, help="enable native pipe descriptors")
    parser.add_argument("--fifo-grant", type=pipe_fields, action="append", default=[],
                        metavar="OWNER:READERMASK:WRITERMASK")
    parser.add_argument("--pipe-link", type=pipe_fields, action="append", default=[],
                        metavar="WRITER:WFD:READER:RFD")
    parser.add_argument("--pointer-tablet", action="store_true",
                        help="grant the display owner a modern virtio-input absolute tablet on QEMU virt")
    parser.add_argument("--display-service", choices=("0", "1"), action="append", default=[], metavar="SLOT",
                        help="give one ordinary virt FIFO process ramfb scanout and PL011 byte input")
    parser.add_argument("--display-five-active", action="store_true", help="start five ordinary display roles with broker 3 and X11 client 4")
    parser.add_argument("--display-standby", choices=("2",), action=SinglePath, metavar="SLOT",
                        help="reserve slot 2 for one display handoff in the five-process local compositor package")
    parser.add_argument("--uart-rx-irq", action="count", default=0,
                        help="receive PL011 input via GICv2 INTID 33 for the selected display owner")
    parser.add_argument("--uart-rx-standby", choices=("2",), action=SinglePath, metavar="SLOT",
                        help="keep slot 2 dormant for one handoff when UART owner 0 exits with status 86")
    parser.add_argument("--net-service", choices=("0", "1", "2"), action="append", default=[], metavar="SLOT",
                        help="give the selected socket service the virtio network interface")
    parser.add_argument("--net-standby", choices=("0", "1", "2"), action=SinglePath, metavar="SLOT",
                        help="allow one packaged process to claim raw NIC access after owner retirement")
    parser.add_argument("--socket-service", choices=("0", "1", "2", "3"), action="append", default=[], metavar="SLOT",
                        help="select the socket broker; display broker 3 accepts the profile-specific callers on virt FIFO")
    parser.add_argument("--shared-fd", action="count", default=0,
                        help="allow the selected AF_UNIX broker to pass shared-memory descriptors")
    parser.add_argument("--tmpfs", type=tmpfs_grant, action="append", default=[], metavar="SLOT:ro|rw",
                        help="grant this client access to the volatile /tmp service on the selected socket broker")
    parser.add_argument("--cap-grant", type=cap_grant, action="append", default=[], metavar="OWNER:TARGET:RIGHTS",
                        help="install a mailbox capability (SEND=1, DELEGATE=2); per-owner order is preserved")
    parser.add_argument("--cap-route", type=ipc_route, action="append", default=[], metavar="SRC:DST",
                        help="allow capability transfers between the specified slots")
    parser.add_argument("--loan-route", type=ipc_route, action="append", default=[], metavar="PRODUCER:CONSUMER",
                        help="grant one native FIFO/RR process pair a shared-buffer loan channel")
    parser.add_argument("--waitset", action="count", default=0,
                        help="enable waitsets for configured mailbox or notification routes")
    parser.add_argument("--pubsub-service", choices=["3"], action="append", default=[])
    parser.add_argument("--pubsub-publish", type=topic_grant, action="append", default=[], metavar="SLOT:MASK")
    parser.add_argument("--pubsub-subscribe", type=topic_grant, action="append", default=[], metavar="SLOT:MASK")
    args = parser.parse_args()
    if args.linux_source_net_restart_pending_test and args.linux_source_driver != "net-restart":
        parser.error("--linux-source-net-restart-pending-test requires --linux-source-net-restart")
    if args.linux_source_net_rx_irq_rearm_test and args.linux_source_driver != "net-rx-irq":
        parser.error("--linux-source-net-rx-irq-rearm-test requires --linux-source-net-rx-irq")
    if args.linux_source_driver:
        option = f"--linux-source-{args.linux_source_driver}"
        allowed = {"out", "board", "kernel_profile", "linux_source_driver", "linux_source_net_rx_irq_rearm_test", "linux_source_net_restart_pending_test", "elf", "peer_elf",
                   "policy", "priority", "peer_priority", "arg", "peer_arg", "env", "service_account"}
        conflicts = [a.option_strings[0] for a in parser._actions if a.dest not in allowed
                     and hasattr(args, a.dest) and getattr(args, a.dest) != a.default]
        peers = 2 if args.linux_source_driver == "net-restart" else 1
        if args.board not in (None, "virt") or args.policy != ["fifo"] or not args.elf or len(args.peer_elf) != peers:
            parser.error(f"{option} requires virt FIFO, one --elf and {peers} --peer-elf argument(s)")
        if conflicts:
            parser.error(f"options incompatible with {option}: " + ", ".join(conflicts))
        if args.linux_source_driver in ("net-rx", "net-rx-poll", "net-rx-irq", "net-restart", "net-lifecycle", "net-lifecycle-failures", "net-duplex"):
            args.pipes = 1
            args.pipe_link = [(0, 3, 1, 3), (1, 4, 0, 4)]
            if args.linux_source_driver == "net-restart":
                args.pipe_link.extend([(2, 3, 1, 5), (1, 6, 2, 4)])
    if args.task_policy is not None:
        allowed = {"out", "board", "kernel_profile", "task_policy"}
        conflicts = [action.option_strings[0] for action in parser._actions
                     if action.dest not in allowed and hasattr(args, action.dest)
                     and getattr(args, action.dest) != action.default]
        if conflicts or args.board == "pi4":
            parser.error("--task-policy requires virt and excludes legacy options " + ", ".join(conflicts))
        from build_task_policy import package
        try:
            return package(args, ROOT)
        except (ValueError, OSError, subprocess.SubprocessError) as exc:
            parser.error(str(exc))
    if len(args.pubsub_service) > 1:
        parser.error("--pubsub-service may appear only once")
    if (args.pubsub_publish or args.pubsub_subscribe) and not args.pubsub_service:
        parser.error("topic grants require --pubsub-service")
    if args.pubsub_service:
        if (args.board not in (None, "virt") or args.policy not in (["fifo"], ["rr"])
                or args.threads or len(args.peer_elf) != 3 or args.peer3_arg):
            parser.error("pubsub requires four ordinary virt FIFO/RR applications and kernel-generated slot 3 arguments")
        if not all((pid, 3) in args.ipc_route for pid in range(3)):
            parser.error("pubsub requires explicit IPC routes 0:3, 1:3 and 2:3")
        publish = topic_vector(parser, args.pubsub_publish)
        subscribe = topic_vector(parser, args.pubsub_subscribe)
    if len(args.loan_route) > 1:
        parser.error("--loan-route may appear only once")
    if args.waitset > 1:
        parser.error("--waitset may appear only once")
    if args.waitset and not (args.mailbox_route or args.notify_route):
        parser.error("--waitset requires --mailbox-route or --notify-route")
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
    display_slot, service_slot, service_clients = native_service_slots(parser, args, policy, board)
    if args.tmpfs:
        slots = [slot for slot, _ in args.tmpfs]
        if (board != "virt" or policy not in ("fifo", "rr") or not args.elf or args.threads or
                not args.socket_service or args.net_service or len(set(slots)) != len(slots) or
                any((slot, service_slot) not in args.ipc_route or slot == service_slot
                    or slot > len(args.peer_elf) for slot in slots)):
            parser.error("--tmpfs requires distinct, routed clients of the virt FIFO/RR AF_UNIX broker")
        broker_elf = [args.elf, *args.peer_elf][service_slot]
        if not re.search(r"\bjnox_tmpfs_call$", output(["readelf", "-sW", broker_elf]), re.M):
            parser.error("--tmpfs requires a socket broker compiled with the tmpfs Cargo feature")
    validate_pipes(parser, args, policy, board)
    if args.service_account is not None and (policy not in ("fifo", "rr") or args.threads or not (args.elf or args.source)):
        parser.error("--service-account requires packaged native FIFO/RR processes")
    if args.nodename is not None and (policy not in ("fifo", "rr") or args.threads or not args.elf or
            not re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?", args.nodename)):
        parser.error("--nodename requires native FIFO/RR and one 1..63 byte alphanumeric or hyphen label")
    if args.boot_utc_seconds is not None and (
            board != "virt" or policy not in ("fifo", "rr") or args.threads or
            not (args.elf or args.source) or not -(1 << 63) <= args.boot_utc_seconds < (1 << 63)):
        parser.error("--boot-utc-seconds requires ordinary virt FIFO/RR and a signed 64-bit POSIX epoch")
    if args.console_write and (policy not in ("fifo", "rr") or not args.elf or
            len(set(args.console_write)) != len(args.console_write) or
            max(args.console_write) > len(args.peer_elf) or (4 in args.console_write and not args.display_five_active)):
        parser.error("--console-write requires distinct, packaged native FIFO/RR process slots")
    shell = None
    if args.exec_child or args.rom_file:
        if not args.pipes or len(args.peer_elf) != 2:
            parser.error("shell packaging requires --pipes and exactly two peer ELFs")
        try:
            shell = shell_package(args.exec_child, args.rom_file)
        except (OSError, ValueError) as exc:
            parser.error(str(exc))
    grants = [
        ("ipc", "caller", "server", sorted(args.ipc_route)),
        ("mailbox", "caller", "owner", sorted(args.mailbox_route)),
        ("notify", "sender", "owner", sorted(args.notify_route)),
    ]
    for kind, routes in [(kind, routes) for kind, _, _, routes in grants] + [("loan", args.loan_route), ("cap", args.cap_route)]:
        if not routes:
            continue
        count = 1 + len(args.peer_elf)
        maximum = (5 if args.display_standby or args.display_five_active else 4) if kind == "ipc" else 3
        if board != "virt" or policy not in ("fifo", "rr") or args.threads or not 2 <= count <= maximum:
            parser.error(f"--{kind}-route requires 2..{maximum} native processes under FIFO or RR on virt")
        if len(set(routes)) != len(routes):
            parser.error(f"duplicate --{kind}-route")
        if any(caller == server or max(caller, server) >= count for caller, server in routes):
            parser.error(f"--{kind}-route requires distinct slots present in the package")
    if args.cap_grant:
        slots = 1 + len(args.peer_elf)
        if board != "virt" or policy not in ("fifo", "rr") or args.threads or slots not in (2, 3):
            parser.error("--cap-grant requires two or three ordinary FIFO/RR processes on virt")
        if len(set(args.cap_grant)) != len(args.cap_grant):
            parser.error("duplicate --cap-grant")
        per_owner = [0] * slots
        for owner, target, _ in args.cap_grant:
            if max(owner, target) >= slots:
                parser.error("--cap-grant references an absent process")
            per_owner[owner] += 1
            if per_owner[owner] > 4:
                parser.error("at most four initial capabilities per owner")
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
    if args.pubsub_service:
        vectors[3] = [publish, subscribe]
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
    if (args.extension_program is not None) != (policy == "extension"):
        parser.error("--extension-program is required exclusively with --policy extension")
    if policy == "extension":
        if len(args.peer_elf) != 2:
            parser.error("extension requires one reserved application and two delegated peers")
        try:
            rule_path = args.extension_program.resolve(strict=True)
            rule_source = rule_path.read_bytes()
            ops = parse_program(rule_source)
        except (ValueError, OSError) as exc:
            parser.error(f"invalid --extension-program: {exc}")
    if args.amc_clock_page and (args.amc_clock_page != 1 or policy != "amc"):
        parser.error("--amc-clock-page is accepted once, only with --policy amc")
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
    environments, encoded_environment = encode_environment(parser, args, vectors, policy)
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
    if args.kernel_profile != "release":
        kernel_command.extend(["--kernel-profile", args.kernel_profile])
    kernel_target = read_registry(ROOT)["aarch64"]
    kernel_artifact = Path("target") / kernel_target / args.kernel_profile / "jnox-kernel"
    metadata = {
        "kernel_build": {
            "cargo_profile": args.kernel_profile, "target": kernel_target,
            "cargo_command": cargo_command(kernel_target, args.kernel_profile),
            "artifact": kernel_artifact.as_posix(),
            "config_sha256": {p: sha256(ROOT / p) for p in
                              ("kernel/Cargo.toml", "kernel/.cargo/config.toml", "kernel/Cargo.lock")},
            "profile_environment": {k: v for k, v in env.items() if k.startswith("CARGO_PROFILE_")},
        },
        "format": 1, "profile": "native", "app": app, "board": board,
        "commit": commit, "tracked_changes": dirty, "source_sha256": inputs,
        "input": {"kind": "elf" if args.elf else "c", "path": input_name, "sha256": input_digest},
        "compiler": None if args.elf else output(["clang", "--version"], env=env),
        "linker": None if args.elf else output(["ld.lld", "--version"], env=env),
        "rust": output(["rustc", "-vV"], ROOT / "kernel", env),
        "cargo": output(["cargo", "--version"], ROOT / "kernel", env),
        "sdk_command": user_command, "kernel_command": kernel_command,
    }
    if args.linux_source_driver:
        driver_kind = "rtc" if args.linux_source_driver == "rtc" else "net"
        env[f"JNOX_NATIVE_LINUX_SOURCE_{driver_kind.upper()}"] = "1"
        if driver_kind == "rtc":
            device = {"device": "arm,pl031", "mmio_base": 0x09010000, "mmio_bytes": 4096,
                      "intid": 34, "driver_source": "drivers/rtc/rtc-pl031.c"}
        else:
            device = {"device": "virtio-net", "mmio_base": 0x0a003e00, "mmio_bytes": 0x200,
                      "driver_source": "drivers/net/virtio_net.c", "transport": "virtio-net-mmio-modern",
                      "qemu_device": "virtio-net-device"}
        metadata[f"linux_source_{driver_kind}"] = {"driver_slot": 0, "unauthorized_slots": [1], **device}
        if args.linux_source_driver in ("net-queues", "net-packets", "net-rx", "net-rx-poll", "net-rx-irq", "net-restart", "net-lifecycle", "net-lifecycle-failures", "net-duplex"):
            env["JNOX_NATIVE_LINUX_PACKET_QUEUES"] = "1"
            metadata["linux_packet_queues"] = {
                "queue_size": 32, "indices": {"rx": 0, "tx": 1},
                "status_after_setup": "FEATURES_OK", "buffers_required_for_driver_ok": True,
                "driver_source": device["driver_source"], "syscalls": {"setup": 161, "query": 162},
            }
        if args.linux_source_driver in ("net-packets", "net-rx", "net-rx-poll", "net-rx-irq", "net-restart", "net-lifecycle", "net-lifecycle-failures", "net-duplex"):
            env["JNOX_NATIVE_LINUX_NET_PACKETS"] = "1"
            metadata["linux_net_packets"] = {
                "driver_slot": 0, "unauthorized_slots": [1], "virtio_net_header_bytes": 12,
                "min_ethernet_bytes": 14, "max_ethernet_bytes": 1514, "status_after_start": 15,
                "syscalls": {"start": 163, "tx_submit": 164, "tx_kick": 165, "tx_reap": 166,
                             "rx_receive": 167, "irq_ack": 168, "tx_callbacks": 169},
            }
        if args.linux_source_driver in ("net-rx", "net-rx-poll", "net-rx-irq", "net-restart", "net-lifecycle", "net-lifecycle-failures", "net-duplex"):
            env["JNOX_NATIVE_LINUX_SOURCE_NET_RX"] = "1"
            packet_calls = metadata["linux_net_packets"]["syscalls"]
            del packet_calls["start"], packet_calls["rx_receive"]
            metadata["linux_source_net_rx"] = {
                "driver_slot": 0, "consumer_slot": 1, "unauthorized_slots": [1],
                "driver_source": device["driver_source"], "queue_size": 32, "grant_bytes": 1530,
                "virtio_net_header_bytes": 12, "frame_count": 80, "frame_ethertype": 0x88b5,
                "syscalls": {"start": 170, "post": 171, "reap": 172},
            }
        if args.linux_source_driver in ("net-rx-poll", "net-rx-irq", "net-restart", "net-lifecycle", "net-lifecycle-failures", "net-duplex"):
            env["JNOX_NATIVE_LINUX_SOURCE_NET_RX_POLL"] = "1"
            metadata["linux_source_net_rx_poll"] = {
                "driver_slot": 0, "syscalls": {"rx_callbacks": 173},
                "modes": {"disable": 0, "enable_prepare": 1, "poll": 2, "refill_kick": 3},
                "initial_kick": "driver_ok", "live_refill_kick": "explicit",
            }
        if args.linux_source_driver in ("net-rx-irq", "net-restart", "net-lifecycle", "net-lifecycle-failures", "net-duplex"):
            env["JNOX_NATIVE_LINUX_SOURCE_NET_RX_IRQ"] = "1"
            if args.linux_source_net_rx_irq_rearm_test:
                env["JNOX_NATIVE_LINUX_SOURCE_NET_RX_IRQ_REARM_TEST"] = "1"
            metadata["linux_source_net_rx_irq"] = {
                "rearm_test_only": args.linux_source_net_rx_irq_rearm_test,
                "driver_slot": 0, "peer_slot": 1, "queue_index": 0, "intid": 79,
                "callback": "skb_recv_done", "registrar": "virtnet_find_vqs",
                "syscalls": {"wait": 174},
            }
        if args.linux_source_driver == "net-restart":
            env["JNOX_NATIVE_LINUX_SOURCE_NET_RESTART"] = "1"
            if args.linux_source_net_restart_pending_test:
                env["JNOX_NATIVE_LINUX_SOURCE_NET_RESTART_PENDING_TEST"] = "1"
            metadata["linux_source_net_restart"] = {"primary_slot": 0, "survivor_slot": 1,
                "dormant_spare_slot": 2, "intid": 79, "max_replacements": 1}
        if args.linux_source_driver == "net-duplex":
            metadata["linux_source_net_duplex"] = {"driver_slot": 0, "peer_slot": 1,
                "rx_queue": 0, "tx_queue": 1, "queue_pairs": 1,
                "tx_submit_notify_reap_svc": [164, 165, 166], "rx_post_reap_svc": [171, 172],
                "irq_wait_svc": 174, "device_ready_svc": 177}
        if args.linux_source_driver in ("net-lifecycle", "net-lifecycle-failures", "net-duplex"):
            env["JNOX_NATIVE_LINUX_SOURCE_NET_LIFECYCLE"] = "1"
            metadata["linux_source_net_lifecycle"] = {"driver_slot": 0, "peer_slot": 1,
                "driver": "virtio_net_driver", "netdev_ops": "virtnet_netdev", "intid": 79,
                "syscalls": {"driver_ok_empty_queues": 177, "status": 160, "release": 156}}
        if args.linux_source_driver == "net-lifecycle-failures":
            metadata["linux_source_net_lifecycle_failures"] = {"test_only": True, "driver_slot": 0,
                "peer_slot": 1, "kernel_abi": "linux-source-net-lifecycle", "fault_scope": "EL0 adapter"}
        metadata["package_argv"] = sys.argv[:]
    if args.pubsub_service:
        env["JNOX_NATIVE_PUBSUB_SERVICE"] = "3"
        env["JNOX_NATIVE_PUBSUB_PUBLISH"] = publish
        env["JNOX_NATIVE_PUBSUB_SUBSCRIBE"] = subscribe
        metadata["pubsub_service"] = {"slot": 3, "authorized_clients": [0, 1, 2],
                                      "publish_masks": publish, "subscribe_masks": subscribe}
    if shell is not None:
        names, files, image = shell
        rom = out / "native_shell.rom"
        rom.write_bytes(image)
        digest = hashlib.sha256(image).hexdigest()
        env.update(JNOX_NATIVE_EXEC_CHILDREN=f"1:{names[1]},2:{names[2]}",
                   JNOX_NATIVE_ROM_IMAGE=str(rom), JNOX_NATIVE_ROM_SHA256=digest)
        metadata.update(exec_names=names, rom_image=rom.name, rom_sha256=digest, rom_files=files)
    if args.console_write:
        metadata["console_write_slots"] = sorted(args.console_write)
        env["JNOX_NATIVE_CONSOLE_WRITERS"] = "".join(map(str, sorted(args.console_write)))
    if args.tmpfs:
        volume_grants = dict(args.tmpfs)
        env["JNOX_NATIVE_TMPFS_GRANTS"] = "".join(
            {"ro": "1", "rw": "2"}.get(volume_grants.get(i), "0") for i in range(5))
        metadata["tmpfs"] = {"service_slot": service_slot, "grants": volume_grants, "mount": "/tmp",
                             "files": 4, "bytes_per_file": 1024, "fd_first": 32, "fds_per_client": 8,
                             "directories_including_root_and_tmp": 6, "directory_entries_including_tmp": 12,
                             "max_path_bytes": 63, "max_component_bytes": 31}
    if args.pipes:
        env["JNOX_NATIVE_PIPES"] = "1"
        fifo_grants = sorted(args.fifo_grant)
        for key, rows in (("JNOX_NATIVE_FIFO_GRANTS", fifo_grants), ("JNOX_NATIVE_PIPE_LINKS", args.pipe_link)):
            if rows:
                env[key] = ",".join(":".join(map(str, row)) for row in rows)
        metadata.update(pipes=True,
                        fifo_grants=[dict(owner=o, reader_mask=r, writer_mask=w) for o, r, w in fifo_grants],
                        pipe_links=[dict(writer=w, writer_fd=a, reader=r, reader_fd=b) for w, a, r, b in args.pipe_link],
                        package_argv=sys.argv[:])
    if args.display_service:
        env["JNOX_NATIVE_DISPLAY_SERVICE"] = str(display_slot)
        metadata["display_service"] = {
            "slot": display_slot, "display": "ramfb", "input": "pl011",
            "width": 320, "height": 240, "stride": 1280, "format": "XRGB8888",
            "syscalls": {"init": 72, "write_row": 73, "poll_key": 74},
        }
        if args.pointer_tablet:
            env["JNOX_NATIVE_POINTER_TABLET"] = "1"
            metadata["pointer_tablet"] = {"device": "virtio-tablet-device", "owner_slot": display_slot,
                                          "transport": "virtio-mmio-modern", "service": 197,
                                          "queue_size": 16, "irq_mode": "polled"}
        if args.display_five_active:
            env["JNOX_NATIVE_DISPLAY_FIVE_ACTIVE"] = "1"
            metadata["display_five_active"] = {"compositor": 0, "xwayland": 1, "native_client": 2, "broker": 3, "x11_client": 4}
        if args.display_standby is not None:
            env["JNOX_NATIVE_DISPLAY_STANDBY"] = args.display_standby
            metadata["display_standby"] = {"owner_slot": 0, "client_slot": 1, "standby_slot": 2,
                                           "broker_slot": 3, "headless_slot": 4, "max_handoffs": 1}
            if args.pointer_tablet:
                metadata["pointer_tablet"].update(standby_slot=2, max_handoffs=1,
                                                 handoff_requires_acknowledged_reset=True)
            metadata["package_argv"] = sys.argv[:]
        if args.uart_rx_irq:
            env["JNOX_NATIVE_UART_RX_IRQ"] = "1"
            metadata["uart_rx_irq"] = {"device": "pl011", "controller": "gicv2", "intid": 33,
                                       "capacity": 16, "stats_syscall": 75}
            if args.uart_rx_standby is not None:
                env["JNOX_NATIVE_UART_RX_STANDBY"] = args.uart_rx_standby
                metadata["uart_rx_standby"] = {"owner_slot": 0, "continuing_peer_slot": 1,
                                               "standby_slot": 2, "failure_exit": 86, "max_handoffs": 1}
    if args.net_service:
        env["JNOX_NATIVE_NET_SERVICE"] = str(service_slot)
        metadata["net_service"] = {"slot": service_slot, "transport": "virtio-net-mmio-modern",
                                   "mac": "52:54:00:12:34:56",
                                   "syscalls": {"rx": 38, "tx": 39, "try_receive": 40}}
    if args.net_standby is not None:
        env["JNOX_NATIVE_NET_STANDBY"] = args.net_standby
        metadata["net_service"].update(standby_slot=int(args.net_standby), max_handoffs=1)
        metadata["net_service"]["syscalls"]["claim"] = 76
        metadata["package_argv"] = sys.argv[:]
    if args.socket_service:
        env["JNOX_NATIVE_SOCKET_SERVICE"] = str(service_slot)
        metadata["socket_service"] = {"slot": service_slot, "authorized_clients": service_clients}
    if args.shared_fd:
        env["JNOX_NATIVE_SHARED_FD"] = "1"
        metadata["shared_fd"] = {"broker_slot": service_slot, "client_slots": [0, 1, 2] if args.display_five_active else service_clients,
                                 "fd_first": 96, "fds_per_client": 8, "mappings_per_client": 4,
                                 "objects": 4, "max_bytes_per_object": 307200, "in_flight": 8}
    if args.cap_grant or args.cap_route:
        cap_routes = sorted(args.cap_route)
        for key, rows in (("JNOX_NATIVE_CAP_GRANTS", args.cap_grant), ("JNOX_NATIVE_CAP_ROUTES", cap_routes)):
            if rows:
                env[key] = ",".join(":".join(map(str, row)) for row in rows)
        metadata.update(cap_grants=[dict(owner=o, target=t, rights=r) for o, t, r in args.cap_grant],
                        cap_routes=[dict(src=src, dst=dst) for src, dst in cap_routes],
                        mailbox_routes=[], cap_handle_slots=4, cap_notice_slots=2, package_argv=sys.argv[:])
    if args.loan_route:
        producer, consumer = args.loan_route[0]
        env["JNOX_NATIVE_LOAN_ROUTE"] = f"{producer}:{consumer}"
        metadata["loan_route"] = {"producer": producer, "consumer": consumer}
        metadata["loan_pool"] = {"pages": 3, "page_bytes": 4096, "ring_entries": 2, "max_length_bytes": 4096}
        metadata["package_argv"] = sys.argv[:]
    if args.waitset:
        env["JNOX_NATIVE_WAITSET"] = "1"
        metadata["waitset"] = True
    for kind, origin, endpoint, routes in grants:
        if routes:
            env[f"JNOX_NATIVE_{kind.upper()}_ROUTES"] = ",".join(f"{caller}:{dest}" for caller, dest in routes)
            metadata[f"{kind}_routes"] = [{origin: caller, endpoint: dest} for caller, dest in routes]
            metadata["package_argv"] = sys.argv[:]
    if policy == "extension":
        program = out / "extension_program.txt"
        program.write_bytes(("\n".join(ops) + "\n").encode("ascii"))
        digest = sha256(program)
        source_name = str(rule_path.relative_to(ROOT) if rule_path.is_relative_to(ROOT) else rule_path)
        inputs[source_name] = hashlib.sha256(rule_source).hexdigest()
        metadata.update(extension_program={"file": program.name, "sha256": digest, "operations": ops,
                                           "source": source_name}, package_argv=sys.argv[:])
        env.update(JNOX_NATIVE_EXTENSION_PROGRAM=str(program), JNOX_NATIVE_EXTENSION_SHA256=digest)
    if policy == "amc":
        canonical = out / "amc-v1.txt"
        canonical.write_bytes(amc_wire)
        digest = hashlib.sha256(amc_wire).hexdigest()
        inputs[str(amc_path)] = hashlib.sha256(amc_source).hexdigest()
        metadata.update(amc=amc, amc_input={"path": str(amc_path), "sha256": inputs[str(amc_path)]},
                        amc_canonical={"file": canonical.name, "sha256": digest},
                        timing_unit="cells", package_argv=sys.argv[:])
        metadata["amc_clock_page"] = {"enabled": bool(args.amc_clock_page), "abi": 1, "va": 0x01400000}
        if args.amc_clock_page:
            env["JNOX_AMC_CLOCK_PAGE"] = "1"
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
        if args.linux_source_net_restart_pending_test:
            metadata["linux_source_net_restart"]["pending_retirement_test"] = {
                "scope": "EL1 only",
                "slot_elf_sha256": {role: slot["elf_sha256"] for role, slot in
                                    zip(("primary", "survivor", "spare"), slots)},
            }
        periodic = args.periodic + args.peer_periodic
        reservations = args.reservation + args.peer_reservation
        classes = args.primary_class + args.peer_class
        batch_jobs = args.batch_job + args.peer_batch_job
        if batch_jobs:
            metadata.update(timing_unit="physical_counter_ticks", package_argv=sys.argv[:])
            env["JNOX_NATIVE_BATCH_JOBS"] = ";".join(f"{offset},{estimate}" for offset, estimate in batch_jobs)
        jobs = args.job + args.peer_job
        for slot in slots:
            if policy == "extension":
                i = slot["slot"]
                slot.update(generation=(7, 11, 17)[i], cpu=0, role="reserved" if i == 0 else "delegated")
            if policy == "amc":
                slot["amc_task"] = amc["tasks"][slot["slot"]]
            slot["argv"] = (["/bin/app", *vectors[slot["slot"]]] if app == "process"
                            else ["/bin/native_startup_probe", "8", "13"])
            slot["environment"] = environments[slot["slot"]]
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
            if batch_jobs:
                offset, estimate = batch_jobs[slot["slot"]]
                slot["batch_job"] = {"offset_ticks": offset, "estimate_ticks": estimate}
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
        if len(slots) >= 4 and policy in ("fifo", "rr"):
            metadata["fixed_event_layout"] = {"fields": 12 + 3 * len(slots), "ready_start": 12,
                                              "service_start": 12 + len(slots), "remaining_start": 12 + 2 * len(slots),
                                              "slots_per_group": len(slots)}
        if policy:
            env["JNOX_NATIVE_POLICY"] = policy
        if priorities:
            env["JNOX_NATIVE_PRIORITIES"] = ",".join(map(str, priorities))
        if weights:
            env["JNOX_NATIVE_WEIGHTS"] = ",".join(map(str, weights))
        if quantum is not None:
            env["JNOX_NATIVE_QUANTUM_TICKS"] = str(quantum)
        if args.nodename is not None:
            metadata["nodename"] = args.nodename
            env["JNOX_NATIVE_NODENAME"] = args.nodename
        if policy in ("fifo", "rr") and not args.threads:
            uid, gid = args.service_account or (65534, 65534)
            metadata["service_account"] = {"uid": uid, "gid": gid}
            if args.service_account is not None:
                env["JNOX_NATIVE_SERVICE_ACCOUNT"] = f"{uid}:{gid}"
        if board == "virt" and policy in ("fifo", "rr") and not args.threads:
            metadata["utc_clock"] = {"source": "unset"}
            if args.boot_utc_seconds is not None:
                seed = str(args.boot_utc_seconds)
                env["JNOX_NATIVE_BOOT_UTC_SECONDS"] = seed
                metadata["utc_clock"] = {"source": "package", "epoch_seconds": seed,
                                         "reference": "monotonic-origin", "uncertainty_ns": None}
        env.update(encoded_arguments)
        env.update(encoded_environment)
        env.update(JNOX_BOARD=board, JNOX_PROFILE="native", JNOX_APP=app,
                   JNOX_NATIVE_ELF=str(user_elf), JNOX_NATIVE_ELF_SHA256=digest,
                   CARGO_TARGET_DIR=str(out / "target"))
        subprocess.run(kernel_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                       check=True)
    shutil.copyfile(out / kernel_artifact, kernel_elf)
    if board == "pi4":
        metadata.update(package_pi4_image(out, kernel_elf, env), package_argv=sys.argv[:])
    metadata.update(user_elf_sha256=digest, kernel_elf_sha256=sha256(kernel_elf),
                    build_log_sha256=sha256(out / "build.log"))
    if (policy == "amc" or args.uart_rx_standby or args.net_standby is not None
            or args.linux_source_driver == "net-restart"
            or (policy in ("fifo", "rr") and len(slots) >= 4)):
        for slot, root in zip(slots, linked_roots(kernel_elf, len(slots))):
            slot["expected_ttbr0_el1"] = root
    (out / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"kernel {metadata['kernel_elf_sha256']}  {kernel_elf}")
    print(f"native {digest}  {user_elf}")
    for slot in slots[1:]:
        print(f"peer {slot['elf_sha256']}  {out / slot['elf']}")


if __name__ == "__main__":
    main()
