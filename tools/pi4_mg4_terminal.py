"""Collect the first two MG4 postterminal diagnostics from previously frozen release images."""
import argparse
import io
import json
from pathlib import Path
import re
import shutil
import socket
import struct
import tempfile
import time

from pi4_mg2_terminal import JOURNAL_BYTES, TerminalRAM, need, read_json
from pi4_migration_capture import BASELINE, STREAM, FILES, FLAGS, STOCK_SHA, acquire, save, sha
from qemu_capture import request


POSITIONS = {"D1": ("migration-clustered", BASELINE), "D2": ("migration-global", STREAM)}
OBJECT = "pi4_handoff_terminal_v1"
PROFILE = "mg4-post-terminal-v1"
TOOLS = ("pi4_mg4_terminal.py", "pi4_mg4_replay.py", "pi4_mg3_symbols.py",
         "pi4_migration_capture.py", "pi4_mg2_terminal.py", "qemu_capture.py")
TERMINAL = re.compile(rb"(?:MG END [01](?: [0-9]+){4}|MG FAIL UNSEALED|PI4 MIG FAIL [0-9a-f]{16}|PI4 SMP BSP FAIL [0-9a-f]{8})")


def ranges(elf, raw):
    from elftools.elf.elffile import ELFFile
    from pi4_mg3_symbols import handoff_terminal

    terminal = handoff_terminal(elf, raw)
    e = ELFFile(io.BytesIO(elf))
    symbols = list(e.get_section_by_name(".symtab").iter_symbols())
    result = {OBJECT: [terminal["physical_address"], terminal["size"]]}
    for name, module, size in (("LOG", "17pi4_migration_log", 2 * JOURNAL_BYTES),
                               ("STACKS", "13pi4_migration", 7 * 16384)):
        fragment = module + str(len(name)) + name + "17h"
        hits = [s for s in symbols if fragment in s.name or s.name == name]
        need(len(hits) == 1, "missing or ambiguous " + name)
        s = hits[0]
        addr = s["st_value"]
        need(isinstance(s["st_shndx"], int) and s["st_info"]["type"] == "STT_OBJECT"
             and s["st_size"] == size, "incorrect " + name)
        section = e.get_section(s["st_shndx"])
        need(section["sh_flags"] & 7 == 3 and
             0 <= addr - section["sh_addr"] <= section["sh_size"] - size, "invalid section")
        loads = [p for p in e.iter_segments() if p["p_type"] == "PT_LOAD" and
                 0 <= addr - p["p_vaddr"] <= p["p_memsz"] - size]
        need(len(loads) == 1 and loads[0]["p_flags"] == 6, "invalid writable mapping")
        p = loads[0]
        pa = p["p_paddr"] + addr - p["p_vaddr"]
        result[name] = [pa, size]
    spans = sorted((pa, pa + n) for pa, n in result.values())
    need(all(0x80000 <= a < b <= 0x100000 for a, b in spans)
         and all(x[1] <= y[0] for x, y in zip(spans, spans[1:])), "invalid Pi RAM ranges")
    return result


def verify_case(folder, position, digest):
    from pi4_mg3_symbols import mg3_symbols, handoff_terminal, declared_wire
    need(sha(folder / "freeze.json") == digest, "case digest mismatch")
    r = read_json(folder / "freeze.json")
    need((r["case"], r["workload"]) == POSITIONS[position] and r["wire_version"] == 4
         and r["flags"] == FLAGS and r["platform"] == "raspi4b-tcg" and r["positions"] == [1, 2]
         and (r["ceiling_ns"], r["quiet_ns"]) == (30000000000, 1000000000)
         and r["qemu_sha256"] == STOCK_SHA and sha(Path(r["qemu"])) == STOCK_SHA
         and re.fullmatch(r"[0-9a-f]{40}", r["source"]), "unexpected scored model")
    need(all(sha(folder / n) == r["hashes"][n] for n in FILES), "changed release inputs")
    elf, raw = [(folder / n).read_bytes() for n in FILES[:2]]
    need(declared_wire(raw) == 4 and mg3_symbols(elf, raw, *POSITIONS[position]) == r["local_symbols"]
         and handoff_terminal(elf, raw) == r["handoff_terminal"], "linked image mismatch")
    return r, ranges(elf, raw)


def freeze(a):
    root = a.root.resolve()
    root.mkdir()
    cases = {}
    sources = set()
    for pos, src, digest in (("D1", a.short, a.short_sha), ("D2", a.stream, a.stream_sha)):
        folder = root / pos
        folder.mkdir()
        for n in (*FILES, "freeze.json"):
            shutil.copyfile(src / n, folder / n)
        r, scope = verify_case(folder, pos, digest)
        save(folder / "map.json", scope)
        sources.add(r["source"])
        cases[pos] = [digest, sha(folder / "map.json")]
    need(len(sources) == 1, "source differs across releases")
    save(root / "diagnostics.json", dict(cases=cases, tools=tool_hashes(), profile=PROFILE))
    print(sha(root / "diagnostics.json"))


def tool_hashes():
    return {n: sha(Path(__file__).with_name(n)) for n in TOOLS}


class Observer(TerminalRAM):
    profile = PROFILE

    def capture(self, dest, record, tids):
        lines = (dest / "uart.bin").read_bytes().splitlines()
        need(lines and TERMINAL.fullmatch(lines[-1]), "no exact terminal at UART tail")
        scope = ranges((dest.parent / "kernel.elf").read_bytes(), (dest.parent / "kernel8.img").read_bytes())
        need(scope == read_json(dest.parent / "map.json"), "physical map differs")
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as channel:
            channel.settimeout(3)
            channel.connect(str(self.path))
            with channel.makefile("rwb", buffering=0) as stream:
                need("QMP" in json.loads(stream.readline()), "missing QMP greeting")
                request(stream, "qmp_capabilities")
                prior = request(stream, "query-status")
                need(prior.get("running") is True, "guest already stopped")
                start = time.monotonic_ns()
                request(stream, "stop")
                after = request(stream, "query-status")
                stop = time.monotonic_ns()
                save(dest / "stop.json", dict(before=prior, after=after, start=start, end=stop))
                need(after.get("status") == "paused" and after.get("running") is False, "unconfirmed stop")
                cpus = request(stream, "query-cpus-fast")
                save(dest / "cpus.json", cpus)
                need(len(cpus) == 4 and {c["cpu-index"] for c in cpus} == set(range(4)) and
                     all(tids.get(str(c["thread-id"])) == f"CPU {c['cpu-index']}/TCG" for c in cpus),
                     "vCPU thread attribution failed")
                for name, (pa, size) in scope.items():
                    target = dest / (name + ".bin")
                    request(stream, "pmemsave", val=pa, size=size, filename=str(target))
                    need(target.stat().st_size == size, "short dump " + name)


                for cpu in range(4):
                    registers = request(stream, "human-monitor-command",
                                        **{"command-line": "info registers", "cpu-index": cpu})
                    (dest / f"cpu-{cpu}-registers.txt").write_text(registers)
                    need(re.search(r"\bPC=[0-9a-fA-F]+\b", registers), "missing stopped PC")


def ram_journals(data):
    need(len(data) == 2 * JOURNAL_BYTES, "short journal object")
    sections = {}
    for cpu in (0, 1):
        base = cpu * JOURNAL_BYTES
        used, lost, _, sealed = struct.unpack_from("<3QB", data, base + 96 * 72)
        need(used <= 96 and lost == 0 and sealed <= 1, "invalid journal count, loss or seal")
        rows = []
        for i in range(used):
            words = struct.unpack_from("<9Q", data, base + i * 72)
            need(1 <= words[8] <= 17, "invalid MG4 row kind")
            rows.append((words[8], words[:8]))
        sections[cpu] = rows
    return sections


def reopen(folder, position, digest):
    from pi4_mg4_replay import corroborate_terminal, decode_terminal
    dest = folder / "first-1"
    session = read_json(dest / "session.json")
    files = {p.name for p in dest.iterdir() if p.is_file()} - {"session.json", "receipt.json"}
    need(set(session["hashes"]) == files and all(sha(dest / n) == session["hashes"][n] for n in files),
         "session originals differ")
    need(session["freeze_sha256"] == digest and session["diagnostic"] == PROFILE and
         session["case"] == POSITIONS[position][0] and session["ordinal"] == 1, "misattributed session")
    result = dict(position=position, session=sha(dest / "session.json"), status="partial",
                  phases={"opening": "unknown", "reverse": "unknown"})
    if session.get("post_terminal", {}).get("status") != "captured":
        result["reason"] = session.get("post_terminal") or session["reason"]
        return result
    try:
        stop = read_json(dest / "stop.json")
        timing = session["observation_monotonic_ns"]
        need(session["reason"] == "terminal_quiet" and session["quiet_ns"] >= 1000000000 and
             timing["last_uart"] + 1000000000 <= timing["end"] <= stop["start"] <= stop["end"] and
             stop["after"].get("running") is False and stop["after"].get("status") == "paused"
             and stop["before"].get("running") is True, "missing confirmed stop")
        need(TERMINAL.fullmatch(session["terminal"].rstrip("\r").encode()) and
             (dest / "uart.bin").read_bytes().splitlines()[-1] == session["terminal"].rstrip("\r").encode(),
             "UART terminal mismatch")
        for name, (_, size) in read_json(folder / "map.json").items():
            need((dest / (name + ".bin")).stat().st_size == size, "incomplete " + name)
        threads = read_json(dest / "threads.json")
        for cpu in (0, 1):
            ids = [t for t, label in threads.items() if label == f"CPU {cpu}/TCG"]
            need(len(ids) == 1 and "observer-" + ids[0] + ".log" in files, "missing exception original")
        projection = decode_terminal((dest / (OBJECT + ".bin")).read_bytes())
        result.update(projection=projection, phases={k: s["phase"] for k, s in projection["slots"].items()})
        sections = ram_journals((dest / "LOG.bin").read_bytes())
        support = corroborate_terminal(projection, sections, 2 if position == "D1" else 0)
        result["corroboration"] = support
        for slot in support["slots"].values():
            need(slot["phase"] == "Empty" or slot["publication_observed"], "publication unobserved")
            need(slot["phase"] != "Consumed" or slot["consumption_observed"], "consumption unobserved")
        result["status"] = "captured"
    except (ValueError, OSError, KeyError, IndexError) as error:
        result["reason"] = str(error)
    return result


def run(a):
    root = a.root.resolve()
    need(sha(root / "diagnostics.json") == a.digest, "wrong cohort")
    cohort = read_json(root / "diagnostics.json")
    need(cohort["tools"] == tool_hashes() and cohort["profile"] == PROFILE, "collector mismatch")
    for pos in POSITIONS:
        folder = root / pos
        digest, map_sha = cohort["cases"][pos]
        _, scope = verify_case(folder, pos, digest)
        need(sha(folder / "map.json") == map_sha and read_json(folder / "map.json") == scope, "changed map")
        if pos == a.position:
            break
        need(read_json(folder / "first-1/receipt.json") == reopen(folder, pos, digest), "prior originals differ")
    with tempfile.TemporaryDirectory(prefix="jnox-mg4-") as tmp:
        status = acquire(argparse.Namespace(directory=folder, digest=digest, ordinal=1),
                         terminal_observer=Observer(Path(tmp) / "qmp.sock"))
    result = reopen(folder, pos, digest)
    save(folder / "first-1/receipt.json", result)
    return int(bool(status) or result["status"] != "captured")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(__doc__)
    sub = parser.add_subparsers(required=True)
    f = sub.add_parser("freeze")
    for key in ("root", "short", "short_sha", "stream", "stream_sha"):
        f.add_argument(key, type=str if key.endswith("sha") else Path)
    f.set_defaults(action=freeze)
    r = sub.add_parser("run")
    r.add_argument("root", type=Path)
    r.add_argument("digest")
    r.add_argument("position", choices=POSITIONS)
    r.set_defaults(action=run)
    args = parser.parse_args()
    raise SystemExit(args.action(args))
