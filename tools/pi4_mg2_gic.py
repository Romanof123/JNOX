"""Fixed two-boot clustered MG2 comparison on the published raspi4b QEMU.

freeze NEW_DIR PINNED_QEMU
run DIR FREEZE_SHA256 POSITION   (1 control, 2 stock GIC trace)
replay DIR FREEZE_SHA256 POSITION SESSION_SHA256
"""
import argparse
import hashlib
import json
import re
import sys
import tempfile
from pathlib import Path
from zipfile import ZipFile

from pi4_mg2_terminal import (
    ARCHIVE,
    ARCHIVE_SHA,
    QEMU_SHA,
    SCRIPTS,
    SOURCE,
    TerminalRAM,
    need,
    read_json,
    rows_and_frames,
    symbols_and_window,
)
from pi4_migration_capture import FILES, FLAGS, acquire, save, sha
from pi4_migration_replay import Unobserved, observed_threads

PROFILE = "mg2-clustered-gic-pair-v1"
CASE = "migration-clustered"
EVENTS = (
    "gic_dist_write",
    "gic_update_bestirq",
    "gic_update_set_irq",
    "gic_acknowledge_irq",
    "gic_cpu_read",
    "gic_cpu_write",
)
POSITIONS = ["control", "gic-events"]


def collector_hashes():
    return {n: sha(Path(__file__).with_name(n)) for n in (*SCRIPTS, Path(__file__).name)}


def event_args(ordinal):
    return [part for event in EVENTS for part in ("-trace", "enable=" + event)] if ordinal == 2 else []


class ModelObserver(TerminalRAM):
    profile = PROFILE

    def __init__(self, path, ordinal):
        super().__init__(path)
        self.ordinal = ordinal

    def argv(self):
        return super().argv() + event_args(self.ordinal)


def freeze(args):
    root = args.directory.resolve()
    model = args.qemu.resolve(strict=True)
    need(sha(model) == QEMU_SHA and sha(ARCHIVE) == ARCHIVE_SHA, "archived input/model identity")
    with ZipFile(ARCHIVE) as archive:
        record = json.loads(archive.read("clustered/freeze.json"))
        need(
            record["source"] == SOURCE
            and record["case"] == CASE
            and record["qemu_sha256"] == QEMU_SHA
            and record["flags"] == FLAGS
            and record["positions"] == [1, 2]
            and (record["ceiling_ns"], record["quiet_ns"]) == (30_000_000_000, 1_000_000_000),
            "unexpected original freeze",
        )
        root.mkdir()
        for name in FILES:
            raw = archive.read("clustered/" + name)
            need(hashlib.sha256(raw).hexdigest() == record["hashes"][name], name)
            (root / name).write_bytes(raw)
    record.update(
        profile=PROFILE,
        qemu=str(model),
        archive_sha256=ARCHIVE_SHA,
        variants=POSITIONS,
        events=list(EVENTS),
        tools=collector_hashes(),
        ram=symbols_and_window((root / "kernel.elf").read_bytes()),
    )
    save(root / "freeze.json", record)
    print(sha(root / "freeze.json"))
    return 0


def frozen(args):
    root = args.directory.resolve()
    need(sha(root / "freeze.json") == args.digest, "untrusted freeze")
    record = read_json(root / "freeze.json")
    need(
        record["profile"] == PROFILE
        and record["case"] == CASE
        and record["source"] == SOURCE
        and record["archive_sha256"] == ARCHIVE_SHA
        and record["qemu_sha256"] == QEMU_SHA
        and record["variants"] == POSITIONS
        and record["events"] == list(EVENTS)
        and record["flags"] == FLAGS,
        "unrecognized fixed comparison",
    )
    return root, record


def run(args):
    root, record = frozen(args)
    need(sys.platform.startswith("linux"), "Linux procfs and private Unix sockets required")
    need(record["tools"] == collector_hashes(), "collector changed after freeze")
    if args.ordinal == 2:
        prior = read_json(root / "first-1/session.json")
        need(
            prior["diagnostic"] == PROFILE
            and prior["freeze_sha256"] == args.digest
            and prior["ordinal"] == 1
            and prior["case"] == CASE,
            "missing original paired control",
        )
    with tempfile.TemporaryDirectory(prefix="jnox-mg2-gic-") as tmp:
        return acquire(args, terminal_observer=ModelObserver(Path(tmp) / "status.sock", args.ordinal))


def model_path(streams, src, dst, word, iar):
    sender, receiver = streams[src], streams[dst]
    entry_line = f"gic_dist_write dist write at 0x00000f00 size 4: 0x{word:08x}"
    entries = [n for n, line in enumerate(sender) if line == entry_line]
    report = {
        "sender": src,
        "target": dst,
        "sgir_word": word,
        "expected_iar": iar,
        "sgi_write_lines": [n + 1 for n in entries],
        "adjacent_target_best_and_level": [],
    }
    for n in entries:
        # The emitting TCG thread is the sender; the text identifies the target.
        following = sender[n + 1 : n + 3]
        best = re.compile(
            rf"gic_update_bestirq cpu {dst} irq 1 priority (\d+) cpu priority mask (\d+) cpu running priority (\d+)"
        )
        winner = best.fullmatch(following[0]) if following else None
        level = len(following) == 2 and following[1] == f"gic_update_set_irq cpu[{dst}]: irq = 1"
        report["adjacent_target_best_and_level"].append(
            {
                "write_line": n + 1,
                "best_line": n + 2 if winner else None,
                "priority_mask_running": list(map(int, winner.groups())) if winner else None,
                "level_1_decision_line": n + 3 if winner and level else None,
            }
        )
    exact = {
        "target_ack_lines": f"gic_acknowledge_irq cpu {dst} acknowledged irq 1",
        "target_iar_result_lines": f"gic_cpu_read cpu {dst} iface read at 0x0000000c: 0x{iar:08x}",
        "target_eoir_write_lines": f"gic_cpu_write cpu {dst} iface write at 0x00000010 0x{iar:08x}",
    }
    for label, text in exact.items():
        report[label] = [n for n, line in enumerate(receiver, 1) if line == text]
    return report


def guest_rows(snapshot):
    # CPU-local indices only. The final M checkpoint and CPU1 SGIR have separate sequences.
    out = {k: [] for k in ("final_M_checkpoints", "sgi_writes", "irq_traps", "eoi")}
    for journal in snapshot["journals"]:
        cpu = journal["cpu"]
        for row in journal["rows"]:
            seq, kind, w = row["seq"], row["kind"], row["words"]
            if kind == 5 and w[0] == 0 and w[3] == 65536:
                need(w[4] == 2147516416, "final M checkpoint sum")
                out["final_M_checkpoints"].append(
                    dict(cpu=cpu, seq=seq, lease=w[2], index=w[3], value=w[4])
                )
            elif kind == 7:
                out["sgi_writes"].append(dict(cpu=cpu, seq=seq, target=w[0], value=w[1], epoch=w[2]))
            elif kind == 4 and w[6] != 65535:
                out["irq_traps"].append(dict(cpu=cpu, seq=seq, elr=w[3], iar=w[6]))
            elif kind == 8:
                out["eoi"].append(dict(cpu=cpu, seq=seq, iar=w[0]))
    return out


def replay(args):
    root, record = frozen(args)
    folder = root / f"first-{args.ordinal}"
    need(sha(folder / "session.json") == args.session, "untrusted session")
    session = read_json(folder / "session.json")
    need(
        session["diagnostic"] == PROFILE
        and session["freeze_sha256"] == args.digest
        and session["ordinal"] == args.ordinal
        and session["case"] == CASE,
        "wrong session",
    )
    argv = session["argv"]
    prefix = [record["qemu"], *FLAGS, "-qmp"]
    tail = argv[len(prefix) + 1 :]
    wanted = event_args(args.ordinal)
    need(
        argv[: len(prefix)] == prefix
        and len(argv) > len(prefix)
        and re.fullmatch(r"unix:/[^,]+/status\.sock,server=on,wait=off", argv[len(prefix)])
        and len(tail) == len(wanted) + 6
        and tail[: len(wanted)] == wanted,
        "incorrect paired flags",
    )
    kernel, image, debug, items, target, log = tail[len(wanted) :]
    need(
        (kernel, Path(image).name, debug, items, target, Path(log).name)
        == ("-kernel", "kernel8.img", "-d", "int,tid", "-D", "observer-%d.log"),
        "unexpected guest or observer",
    )
    for name in FILES:
        need(sha(root / name) == record["hashes"][name], "input changed: " + name)
    hashes = session["hashes"]
    need(
        set(hashes) == {p.name for p in folder.iterdir() if p.is_file() and p.name != "session.json"}
        and all(sha(folder / name) == digest for name, digest in hashes.items()),
        "session files changed",
    )
    report = {
        "case": CASE,
        "position": args.ordinal,
        "variant": POSITIONS[args.ordinal - 1],
        "session_sha256": args.session,
        "terminal": session["terminal"],
        "capture_reason": session["reason"],
        "quiet_ns": session["quiet_ns"],
        "elapsed_ns": session["elapsed_ns"],
        "snapshot": session.get("post_terminal"),
        "M_final_checkpoint_vs_SGIR": "unresolved",
        "migration_qualification": "diagnostic only",
    }
    tids = read_json(folder / "threads.json")
    if not (len(tids) == 4 and set(tids.values()) == {f"CPU {n}/TCG" for n in range(4)}):
        report["missing"] = "CPU thread attribution"
        print(json.dumps(report, indent=2))
        return 2
    streams = {}
    unexpected = []
    for name in hashes:
        match = re.fullmatch(r"observer-(\d+)\.log", name)
        if not match:
            continue
        path = folder / name
        need(path.stat().st_size <= 64 * 1024 * 1024, "oversize observer stream")
        lines = path.read_text(encoding="ascii").splitlines()
        owner = tids.get(match[1])
        if owner in ("CPU 0/TCG", "CPU 1/TCG"):
            streams[int(owner[4])] = lines
        elif any("dist write at 0x00000f00" in line for line in lines):
            unexpected.append(name)
    report["unattributed_or_inactive_cpu_sgir_streams"] = unexpected
    if set(streams) != {0, 1} or unexpected:
        report["missing"] = "unique participating SGIR streams"
        print(json.dumps(report, indent=2))
        return 2
    counts = {
        event: sum(line.startswith(event + " ") for lines in streams.values() for line in lines)
        for event in EVENTS
    }
    need(args.ordinal == 2 or not any(counts.values()), "GIC events in paired control")
    report["model_event_counts"] = counts
    paths = [model_path(streams, 1, 0, 0x18001, 0x401), model_path(streams, 0, 1, 0x28001, 0x001)]
    report["opening"], report["reverse"] = paths
    try:
        _, mapping = observed_threads(folder, hashes)
    except Unobserved as error:
        report["missing"] = str(error)
        print(json.dumps(report, indent=2))
        return 2
    report["cpu_local_exceptions_and_returns"] = mapping
    captured = session.get("post_terminal", {}).get("status") == "captured"
    if captured:
        need(
            session["reason"] == "terminal_quiet"
            and 1_000_000_000 <= session["quiet_ns"] <= session["elapsed_ns"] <= 30_000_000_000,
            "capture timing outside declared limits",
        )
        need(record["ram"] == symbols_and_window((root / "kernel.elf").read_bytes()), "RAM range changed")
        qmp = read_json(folder / "qmp.json")
        need(
            qmp["before"]["running"]
            and qmp["after"]["status"] == "paused"
            and session["observation_monotonic_ns"]["end"] <= qmp["stop_before_ns"] <= qmp["stop_after_ns"]
            and len(qmp["cpus"]) == 4
            and all(tids.get(str(c["thread-id"])) == f"CPU {c['cpu-index']}/TCG" for c in qmp["cpus"]),
            "QMP chronology/CPU identity",
        )
        snap = rows_and_frames((folder / "terminal-ram.bin").read_bytes(), record["ram"])
        report["journal_status"] = [
            {k: j[k] for k in ("cpu", "committed", "lost", "sealed")} for j in snap["journals"]
        ]
        local = guest_rows(snap)
        report["guest_local_rows"] = local
        for route in paths:
            dst, src, iar = route["target"], route["sender"], route["expected_iar"]
            traps = [t for t in local["irq_traps"] if t["cpu"] == dst and t["iar"] == iar]
            route["guest_send_seq"] = [
                w["seq"]
                for w in local["sgi_writes"]
                if w["cpu"] == src and w["target"] == dst and w["value"] == route["sgir_word"]
            ]
            route["guest_eoi_seq"] = [w["seq"] for w in local["eoi"] if w["cpu"] == dst and w["iar"] == iar]
            route["receiver_irq_joins"] = [
                {
                    "guest_seq": t["seq"],
                    "elr": t["elr"],
                    "trace_rows": [m for m in mapping if m[3:] == [dst, "irq", t["elr"]]],
                }
                for t in traps
            ]
            route["unique_receiver_irq_iar_eoir_chain"] = None
            if len(route["receiver_irq_joins"]) == 1:
                linked = route["receiver_irq_joins"][0]["trace_rows"]
                ack, read, eoi = (
                    route[k]
                    for k in ("target_ack_lines", "target_iar_result_lines", "target_eoir_write_lines")
                )
                if len(linked) == len(ack) == len(read) == len(eoi) == len(route["guest_eoi_seq"]) == 1:
                    irq_line = linked[0][1]
                    if irq_line < ack[0] < read[0] < eoi[0]:
                        route["unique_receiver_irq_iar_eoir_chain"] = {
                            "stream": linked[0][0],
                            "exception_line": irq_line,
                            "ack_line": ack[0],
                            "iar_line": read[0],
                            "eoir_line": eoi[0],
                        }
    print(json.dumps(report, indent=2))
    # An unobserved intermediate stage remains null even in a coherent pair capture.
    return 0 if captured and (args.ordinal == 1 or len(paths[0]["sgi_write_lines"]) == 1) else 2


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    modes = parser.add_subparsers(dest="cmd", required=True)
    for cmd in ("freeze", "run", "replay"):
        sub = modes.add_parser(cmd)
        sub.add_argument("directory", type=Path)
        if cmd == "freeze":
            sub.add_argument("qemu", type=Path)
        else:
            sub.add_argument("digest")
            sub.add_argument("ordinal", type=int, choices=(1, 2))
            if cmd == "replay":
                sub.add_argument("session")
    args = parser.parse_args()
    try:
        return {"freeze": freeze, "run": run, "replay": replay}[args.cmd](args)
    except (OSError, KeyError, TypeError, ValueError, RuntimeError, ImportError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
