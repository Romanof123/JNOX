"""Capture post-terminal RAM for six diagnostic positions on the exact archived MG2 model.

freeze NEW_COHORT QEMU
run COHORT COHORT_SHA256 POSITION (1..6)
replay COHORT COHORT_SHA256 POSITION SESSION_SHA256
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import socket
import struct
import sys
import tempfile
import time
from zipfile import ZipFile

from pi4_migration_capture import CASES, FILES, FLAGS, acquire, save, sha
from qemu_capture import request

LEGACY_TICKS = "pi4_migration_return_ticks"
PUBLISHED_TICKS = "pi4_migration_published_return_ticks"

ARCHIVE = (
    Path(__file__).resolve().parents[1] / "docs/evidence/pi4-migration-mg2-first.zip"
)
ARCHIVE_SHA = "287b2893a6ee8bda2a693789e42c02adf44878f5075ef54da00464fcbc0d2005"
SOURCE = "559449c673ac55a0a2bf34f3e8ae680d99e72f09"
QEMU_SHA = "3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862"
POSITION = [(case, n) for case in CASES for n in (1, 2)]
PROFILE = "mg2-post-terminal-ram-v1"
SCRIPTS = (
    "pi4_mg2_terminal.py",
    "pi4_migration_capture.py",
    "qemu_capture.py",
    "pi4_migration_replay.py",
    "pi4_smp_replay.py",
)
ROW = struct.Struct("<9Q")
JOURNAL_BYTES = 6944
FRAME_BYTES = 832


def need(test, message):
    if not test:
        raise ValueError(message)


def read_json(path):
    return json.loads(path.read_bytes())


def trace_scripts():
    return {n: sha(Path(__file__).with_name(n)) for n in SCRIPTS}


def symbols_and_window(elf):
    from elftools.elf.elffile import ELFFile

    e = ELFFile(io.BytesIO(elf))
    segs = [
        s for s in e.iter_segments() if s["p_type"] == "PT_LOAD" and s["p_flags"] == 6
    ]
    need(
        len(segs) == 1 and e["e_machine"] == "EM_AARCH64" and e["e_entry"] == 0x80000,
        "unexpected archived ELF",
    )
    s = segs[0]
    base, size = s["p_paddr"], s["p_memsz"]
    need(
        base == s["p_vaddr"] and 0x80000 <= base < base + size <= 0x100000,
        "unbounded writable segment",
    )
    entries = list(e.get_section_by_name(".symtab").iter_symbols())
    clock_names = {entry.name for entry in entries if entry.name in (LEGACY_TICKS, PUBLISHED_TICKS)}
    need(len(clock_names) == 1, "ambiguous or missing return sample layout")
    tick_name = clock_names.pop()
    wanted = {
        "LOG": 2 * JOURNAL_BYTES,
        "STACKS": 7 * 16384,
        "STATE": 200,
        "LIVE": 16,
        "TASK_LEASE": 24,
        "LOCK": 1,
        "MILESTONE": 1,
        "FAILED": 1,
        "INITIAL": 1,
        "DONE": 2,
        "HZ": 8,
        "SEALED": 1,
        "INVALID": 1,
        "smp_handler_tops": 16,
        tick_name: 16,
    }
    if tick_name == LEGACY_TICKS:
        wanted["PEER_READY"] = 1
    symbols = {}
    for name, expected in wanted.items():
        module = (
            "17pi4_migration_log" if name in ("LOG", "INVALID") else "13pi4_migration"
        )
        fragment = module + str(len(name)) + name + "17h"
        hits = [t for t in entries if t.name == name or fragment in t.name]
        need(
            len(hits) == 1 and hits[0]["st_size"] == expected,
            "ambiguous or incorrect " + name,
        )
        address = hits[0]["st_value"]
        need(base <= address <= base + size - expected, "out-of-range " + name)
        if name == tick_name:
            need(address % 8 == 0, "unaligned return sample pair")
        symbols[name] = [address, expected]
    top = [t for t in entries if t.name == "__stack_top"]
    need(
        len(top) == 1 and top[0]["st_value"] == base + size,
        "RW end differs from __stack_top",
    )
    return {"pa": base, "bytes": size, "symbols": symbols}


def freeze(args):
    root = args.cohort.resolve()
    model = args.qemu.resolve(strict=True)
    need(sha(model) == QEMU_SHA, "QEMU digest differs from the archived 11.1.3 model")
    need(sha(ARCHIVE) == ARCHIVE_SHA, "archived MG2 bytes differ")
    root.mkdir()
    records = {}
    with ZipFile(ARCHIVE) as z:
        for case in CASES:
            prefix = case.removeprefix("migration-") + "/"
            record = json.loads(z.read(prefix + "freeze.json"))
            need(
                record["flags"] == FLAGS
                and record["source"] == SOURCE
                and record["case"] == case
                and record["positions"] == [1, 2]
                and record["wire_version"] == 2
                and record["qemu_sha256"] == QEMU_SHA
                and (record["ceiling_ns"], record["quiet_ns"])
                == (30_000_000_000, 1_000_000_000),
                "historical freeze",
            )
            folder = root / case
            folder.mkdir()
            for name in FILES:
                blob = z.read(prefix + name)
                need(
                    hashlib.sha256(blob).hexdigest() == record["hashes"][name],
                    "archived input " + name,
                )
                (folder / name).write_bytes(blob)
            record.update(
                qemu=str(model),
                profile=PROFILE,
                ram=symbols_and_window((folder / "kernel.elf").read_bytes()),
            )
            save(folder / "freeze.json", record)
            records[case] = sha(folder / "freeze.json")
    save(
        root / "cohort.json",
        dict(
            profile=PROFILE,
            source=SOURCE,
            archive_sha256=ARCHIVE_SHA,
            qemu_sha256=QEMU_SHA,
            tools=trace_scripts(),
            positions=POSITION,
            cases=records,
        ),
    )
    print(sha(root / "cohort.json"))
    return 0


def selected(args):
    root = args.cohort.resolve()
    need(sha(root / "cohort.json") == args.cohort_sha256, "untrusted cohort")
    cohort = read_json(root / "cohort.json")
    need(
        cohort["profile"] == PROFILE
        and cohort["positions"] == [list(p) for p in POSITION]
        and cohort["source"] == SOURCE
        and cohort["qemu_sha256"] == QEMU_SHA
        and cohort["archive_sha256"] == ARCHIVE_SHA,
        "unsupported cohort",
    )
    case, n = POSITION[args.position - 1]
    folder = root / case
    need(sha(folder / "freeze.json") == cohort["cases"][case], "untrusted case freeze")
    return root, cohort, folder, n


class TerminalRAM:
    def __init__(self, path):
        self.path = path

    def argv(self):
        return ["-qmp", "unix:" + str(self.path) + ",server=on,wait=off"]

    def capture(self, dest, record, tids):
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as channel:
            channel.settimeout(3)
            channel.connect(str(self.path))
            with channel.makefile("rwb", buffering=0) as stream:
                need("QMP" in json.loads(stream.readline()), "missing QMP greeting")
                request(stream, "qmp_capabilities")
                prior = request(stream, "query-status")
                need(prior.get("running") is True, "guest already stopped")
                before = time.monotonic_ns()
                request(stream, "stop")
                stopped = request(stream, "query-status")
                after = time.monotonic_ns()
                need(
                    stopped.get("running") is False
                    and stopped.get("status") == "paused",
                    "QMP stop unconfirmed",
                )
                scope = record["ram"]
                request(
                    stream,
                    "pmemsave",
                    val=scope["pa"],
                    size=scope["bytes"],
                    filename=str(dest / "terminal-ram.bin"),
                )
                need(
                    (dest / "terminal-ram.bin").stat().st_size == scope["bytes"],
                    "incomplete physical RAM window",
                )
                cpus = request(stream, "query-cpus-fast")
                save(
                    dest / "qmp.json",
                    dict(
                        before=prior,
                        after=stopped,
                        stop_before_ns=before,
                        stop_after_ns=after,
                        cpus=cpus,
                    ),
                )
                need(
                    len(cpus) == 4
                    and {c["cpu-index"] for c in cpus} == set(range(4))
                    and all(
                        tids.get(str(c["thread-id"])) == f"CPU {c['cpu-index']}/TCG"
                        for c in cpus
                    ),
                    "QMP and observed QEMU CPU threads differ",
                )
                for c in sorted(cpus, key=lambda t: t["cpu-index"]):
                    cpu = c["cpu-index"]
                    text = request(
                        stream,
                        "human-monitor-command",
                        **{"command-line": "info registers", "cpu-index": cpu},
                    )
                    (dest / f"cpu-{cpu}-registers.txt").write_text(text)
                    need(
                        re.search(r"\bPC=[0-9a-fA-F]+\b", text) is not None,
                        "register snapshot lacks PC",
                    )


def run(args):
    root, cohort, folder, n = selected(args)
    need(trace_scripts() == cohort["tools"], "collector changed since freeze")
    need(
        sys.platform.startswith("linux"),
        "native Linux procfs and QMP Unix sockets required",
    )
    for earlier_case, earlier_n in POSITION[: args.position - 1]:
        earlier_dir = root / earlier_case / f"first-{earlier_n}"
        previous = read_json(earlier_dir / "session.json")
        need(
            previous["freeze_sha256"] == cohort["cases"][earlier_case]
            and previous["ordinal"] == earlier_n
            and previous["case"] == earlier_case
            and previous["diagnostic"] == PROFILE,
            "previous diagnostic position missing or misattributed",
        )
    # A short private socket path works even when the evidence directory is long.
    with tempfile.TemporaryDirectory(prefix="jnox-mg2-") as tmp:
        observer = TerminalRAM(Path(tmp) / "status.sock")
        case = argparse.Namespace(
            directory=folder, digest=cohort["cases"][folder.name], ordinal=n
        )
        return acquire(case, terminal_observer=observer)


def rows_and_frames(raw, scope):
    base = scope["pa"]
    need(len(raw) == scope["bytes"], "short physical RAM")

    def grab(addr, n):
        offset = addr - base
        need(0 <= offset <= len(raw) - n, "read outside frozen range")
        return raw[offset : offset + n]

    sym = scope["symbols"]

    def value(key):
        return int.from_bytes(grab(*sym[key]), "little")

    journals = []
    for cpu in (0, 1):
        offset = sym["LOG"][0] + cpu * JOURNAL_BYTES
        used, lost, hz = struct.unpack("<3Q", grab(offset + 6912, 24))
        sealed = grab(offset + 6936, 1)[0]
        need(used <= 96 and sealed in (0, 1), "invalid journal count/seal")
        rows = []
        for i in range(used):
            words = ROW.unpack(grab(offset + i * ROW.size, ROW.size))
            fields, kind = words[:8], words[8]
            need(1 <= kind <= 13, "invalid committed row")
            rows.append({"seq": i, "kind": kind, "words": fields})
        journals.append(
            {
                "cpu": cpu,
                "committed": used,
                "lost": lost,
                "sealed": sealed,
                "seal_frequency": hz,
                "rows": rows,
            }
        )
    # Only the final M SVC in the 0x1d path parks before its frame is reused.
    # The P1 SVC stack can already have been recycled on a successful run.
    frames = []
    for row in journals[0]["rows"]:
        epoch, role, ptr, elr, sp, x21, cause, _ = row["words"]
        if row["kind"] != 4 or (epoch, role, cause) != (0, 0, 65535):
            continue
        stack = sym["STACKS"][0]
        need(
            stack + 16 <= ptr
            and ptr + FRAME_BYTES == sp <= stack + 16384 - 16
            and ptr % 16 == 0,
            "M frame extent",
        )
        words = struct.unpack("<104Q", grab(ptr, FRAME_BYTES))
        need(words[31] == elr and words[21] == x21, "M frame no longer matches trap")
        frames.append(
            {
                "role": "M",
                "epoch": epoch,
                "frame": ptr,
                "elr": elr,
                "sp": sp,
                "x21": x21,
                "spsr": words[32],
                "entry_tick": words[39],
                "svc_x0_to_x4": words[:5],
            }
        )
    flags = {name: value(name) for name in ("LOCK", "INITIAL", "MILESTONE", "FAILED", "SEALED", "INVALID")}
    if "PEER_READY" in sym:
        flags["PEER_READY"] = value("PEER_READY")
    tick_name = PUBLISHED_TICKS if PUBLISHED_TICKS in sym else LEGACY_TICKS
    return {
        "journals": journals,
        "m_opening_svc_frames": frames,
        "frequency": value("HZ"),
        "flags": flags,
        "task_lease": struct.unpack("<3Q", grab(*sym["TASK_LEASE"])),
        "live_cpu_lease": struct.unpack("<2Q", grab(*sym["LIVE"])),
        "last_pre_eret_per_cpu": struct.unpack(
            "<2Q", grab(*sym[tick_name])
        ),
    }


def observations(path, session, frozen, ram):
    from pi4_migration_replay import (
        Unobserved,
        checkpoint_summary,
        journal,
        layout,
        observed_threads,
        score,
    )

    rows = [j["rows"] for j in ram["journals"]]

    def items(cpu, kind):
        return [(r["seq"], r["words"]) for r in rows[cpu] if r["kind"] == kind]

    report = {"workload_qualification": "diagnostic only", "cross_cpu_duration": None}
    report["journal_integrity"] = {
        "both_sealed": all(j["sealed"] == 1 for j in ram["journals"]),
        "lost_rows": [j["lost"] for j in ram["journals"]],
        "invalid_writer": ram["flags"]["INVALID"],
    }
    try:
        external, mapping = observed_threads(path, session["hashes"])
    except Unobserved as error:
        report["external_unobserved"] = str(error)
        return report
    report["external"] = external
    report["external_lines"] = mapping
    uart = (path / "uart.bin").read_bytes()
    need(session["terminal"].encode() in uart.splitlines(), "terminal line missing")
    sparse = []
    retirements = []
    sends = []
    finished = []
    for cpu in (0, 1):
        dispatch = {w[2]: (w[0], w[1]) for _, w in items(cpu, 11) if w[1] != 3}
        for seq, w in items(cpu, 5):
            task, before, after, index, value, *_ = w
            activation = dispatch.get(after)
            need(activation is not None and activation[1] == task, "checkpoint owner")
            ep = activation[0]
            start = 16 if (task, ep) == (2, 1) else 0
            need(
                index > start
                and index % 16 == 0
                and value == (index * (index + 1) - start * (start + 1)) // 2,
                "checkpoint sum mismatch",
            )
            sparse.append(
                dict(
                    cpu=cpu,
                    seq=seq,
                    task=task,
                    epoch=ep,
                    before=before,
                    after=after,
                    index=index,
                    value=value,
                )
            )
        for seq, w in items(cpu, 3):
            ep, task, lease, start, stop, elapsed, _, _ = w
            need(
                dispatch.get(lease) == (ep, task)
                and stop > start
                and stop - start == elapsed,
                "local lease/interval mismatch",
            )
            retirements.append(
                dict(
                    cpu=cpu,
                    seq=seq,
                    task=task,
                    epoch=ep,
                    lease=lease,
                    local_start=start,
                    local_stop=stop,
                    elapsed=elapsed,
                )
            )
        for seq, w in items(cpu, 13):
            task, ep, lease, count, fold, index, value, reserved = w
            need(
                dispatch.get(lease) == (ep, task)
                and not reserved
                and (count, fold, index, value) == checkpoint_summary(task, ep),
                "activation digest",
            )
            matches = [
                (i, d) for i, d in items(cpu, 9) if d[:4] == (task, value, lease, ep)
            ]
            finished.append(
                dict(
                    cpu=cpu,
                    seq=seq,
                    task=task,
                    epoch=ep,
                    lease=lease,
                    checkpoint_count=count,
                    digest=fold,
                    final_index=index,
                    final_sum=value,
                    finish_seq=matches[0][0] if len(matches) == 1 else None,
                )
            )
        for seq, w in items(cpu, 7):
            dst, encoded, epoch, *reserved = w
            boot = items(dst, 12) if dst in (0, 1) else []
            need(
                dst == 1 - cpu
                and len(boot) == 1
                and encoded == (boot[0][1][3] << 16 | 0x8001)
                and not any(reserved),
                "SGIR target or encoding mismatch",
            )
            if epoch == 0:
                need(
                    cpu == 1
                    and any(
                        x["cpu"] == 1
                        and x["task"] == 2
                        and x["epoch"] == 0
                        and x["seq"] < seq
                        for x in retirements
                    ),
                    "opening send before P1 retirement",
                )
            sends.append(
                dict(cpu=cpu, seq=seq, destination=dst, epoch=epoch, sgir=encoded)
            )
    report.update(
        sparse_checkpoints=sparse,
        local_retirements=retirements,
        completed_summaries=finished,
        posted_sgir=sends,
    )
    if b"PI4 MIG UNOBSERVED M FINISHED BEFORE IRQ" in uart:
        need(
            session["terminal"] == "PI4 MIG FAIL 000000000000001d"
            and len(ram["m_opening_svc_frames"]) == 1,
            "early finish lacks matching terminal frame",
        )
        frame = ram["m_opening_svc_frames"][0]
        count, fold, index, value = checkpoint_summary(0, 1)
        need(
            frame["svc_x0_to_x4"] == (0, value, count, fold, index)
            and ("svc", frame["elr"]) in external[0],
            "M frame arithmetic / independent SVC",
        )
        start = ram["last_pre_eret_per_cpu"][0]
        need(0 < start <= frame["entry_tick"], "M interval at terminal SVC")
        report.update(
            guest_result="M finished before IRQ",
            M_frame_digest_verified=True,
            M_local_pre_eret_to_terminal_svc=frame["entry_tick"] - start,
        )
    elif session["terminal"].startswith("MG END "):
        need(
            report["journal_integrity"]
            == {"both_sealed": True, "lost_rows": [0, 0], "invalid_writer": 0},
            "sealed result has incomplete local journals",
        )
        case = CASES.index(frozen["case"])
        sections, hz, totals = journal(uart, case, 2)
        need(
            all(
                sections[cpu] == [(r["kind"], tuple(r["words"])) for r in rows[cpu]]
                for cpu in (0, 1)
            ),
            "sealed UART and terminal RAM differ",
        )
        bodies, _ = layout(*[(path.parent / n).read_bytes() for n in FILES])
        scored = score(sections, hz, totals, case, bodies, 2)
        need(
            all(external[c] == scored["expected_external"][c] for c in (0, 1)),
            "independent exception histories",
        )
        report.update(
            guest_result="sealed MG2 terminal",
            inner_loop_witness=scored["qualifying_inner_pc"],
        )
    else:
        report["guest_result"] = "other terminal; retained prefix only"
    return report


def replay(args):
    _, cohort, folder, n = selected(args)
    path = folder / f"first-{n}"
    need(
        sha(path / "session.json") == args.session_sha256,
        "untrusted diagnostic session",
    )
    s = read_json(path / "session.json")
    f = read_json(folder / "freeze.json")
    argv = s["argv"]
    prefix = [f["qemu"], *FLAGS]
    tail = argv[len(prefix) :]
    need(
        argv[: len(prefix)] == prefix
        and len(tail) == 8
        and tail[0] == "-qmp"
        and re.fullmatch(r"unix:/[^,]+/status\.sock,server=on,wait=off", tail[1])
        and tail[2] == "-kernel"
        and Path(tail[3]).name == "kernel8.img"
        and tail[4:7] == ["-d", "int,tid", "-D"]
        and Path(tail[7]).name == "observer-%d.log",
        "diagnostic launch configuration",
    )
    need(
        s["diagnostic"] == PROFILE
        and s["case"] == folder.name
        and s["ordinal"] == n
        and s["freeze_sha256"] == cohort["cases"][folder.name],
        "session attribution",
    )
    for name in FILES:
        need(sha(folder / name) == f["hashes"][name], "input changed: " + name)
    need(
        f["ram"] == symbols_and_window((folder / "kernel.elf").read_bytes()),
        "RAM scope mismatch",
    )
    members = {
        p.name for p in path.iterdir() if p.is_file() and p.name != "session.json"
    }
    need(
        members == set(s["hashes"])
        and all(sha(path / k) == d for k, d in s["hashes"].items()),
        "session evidence changed",
    )
    report = {
        "position": args.position,
        "case": folder.name,
        "session_sha256": args.session_sha256,
        "diagnostic_only": True,
        "terminal": s["terminal"],
        "capture": s.get("post_terminal"),
        "reason": s["reason"],
    }
    if s.get("post_terminal", {}).get("status") != "captured":
        print(json.dumps(report, indent=2))
        return 2
    need(
        s["reason"] == "terminal_quiet"
        and 1_000_000_000 <= s["quiet_ns"] <= s["elapsed_ns"] <= 30_000_000_000,
        "terminal observation window",
    )
    timing = s["observation_monotonic_ns"]
    qmp = read_json(path / "qmp.json")
    tids = read_json(path / "threads.json")
    need(
        timing["end"] - timing["last_uart"] == s["quiet_ns"]
        and timing["end"] - timing["start"] == s["elapsed_ns"]
        and timing["end"] <= qmp["stop_before_ns"] <= qmp["stop_after_ns"]
        and qmp["before"]["running"] is True
        and qmp["after"]["running"] is False
        and qmp["after"]["status"] == "paused",
        "QMP stop/terminal chronology",
    )
    cpus = qmp["cpus"]
    need(
        len(cpus) == 4
        and {c["cpu-index"] for c in cpus} == set(range(4))
        and all(
            tids.get(str(c["thread-id"])) == f"CPU {c['cpu-index']}/TCG" for c in cpus
        ),
        "independent CPU thread identity",
    )
    report["terminal_ram"] = rows_and_frames(
        (path / "terminal-ram.bin").read_bytes(), f["ram"]
    )
    report["observations"] = observations(path, s, f, report["terminal_ram"])
    report["stopped_pcs"] = {
        cpu: re.search(
            r"\bPC=([0-9a-fA-F]+)\b", (path / f"cpu-{cpu}-registers.txt").read_text()
        )[1]
        for cpu in range(4)
    }
    print(json.dumps(report, indent=2))
    return 0


def main():
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = p.add_subparsers(dest="action", required=True)
    for action in ("freeze", "run", "replay"):
        op = sub.add_parser(action)
        op.add_argument("cohort", type=Path)
        if action == "freeze":
            op.add_argument("qemu", type=Path)
        else:
            op.add_argument("cohort_sha256")
            op.add_argument("position", type=int, choices=range(1, 7))
            if action == "replay":
                op.add_argument("session_sha256")
    args = p.parse_args()
    try:
        return {"freeze": freeze, "run": run, "replay": replay}[args.action](args)
    except (OSError, ValueError, KeyError, RuntimeError, ImportError) as error:
        p.exit(1, f"{error}\n")


if __name__ == "__main__":
    sys.exit(main())
