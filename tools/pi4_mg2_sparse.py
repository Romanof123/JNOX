"""First off/on pair for the rebuilt #198 model and frozen clustered guest."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
from zipfile import ZipFile

from pi4_mg2_terminal import (ARCHIVE, ARCHIVE_SHA, QEMU_SHA, SOURCE, SCRIPTS, TerminalRAM,
                              need, observations, read_json, rows_and_frames, symbols_and_window)
from pi4_migration_capture import FLAGS, acquire, save, sha
from pi4_migration_replay import observed_threads, Unobserved

PROFILE = "mg2-clustered-sparse-v1"
CASE = "migration-clustered"
MODEL_SHA = "2e465436514c0dc203eb2e9c3440187252cb84ebcdcc3fee6de76c9fa1c95fe6"
MODEL_BUILD_ID = "ef73da8a108e97b7d11566630ac676893c2f3ef8"
HEADER_SHA = "335d4e472067e914add598b60d1f7cf0dc029cd48f62f66656d6a88c54d8d9f9"
INPUTS = {
    "kernel8.img": "cc3dbb8ececa7f00c01164b716310596d73bdad1c8eed49753f9f3a5ea60ce79",
    "kernel.elf": "8660010cca701e54f23a6af3c1782dd24bfd8da66d345147b5b4daba95f0002f",
    "disassembly.txt": "aaa48104cf61ea6d1de88b2441929cd2ff0528fa7d2b13f9c42a7ae5f24e683a",
    "linker.map": "28eef4b2abd972fd79f9ffb69267881aa659301b5f9288f94a209074296d9666",
}


def tool_hashes():
    return {name: sha(Path(__file__).with_name(name))
            for name in (*SCRIPTS, "pi4_mg2_sparse.py", "pi4_mg2_sparse.c")}


class Observer(TerminalRAM):
    profile = PROFILE
    log_items = "int,tid,plugin"

    def __init__(self, root, position):
        super().__init__(root / f"first-{position}" / "status.sock")
        self.plugin = root / "observer.so" if position == 2 else None

    def argv(self):
        return super().argv() + (["-plugin", str(self.plugin)] if self.plugin else [])


def command(root, qemu, pos):
    obs = Observer(root, pos)
    return [qemu, *FLAGS, *obs.argv(), "-kernel", str(root / "kernel8.img"), "-d", obs.log_items,
            "-D", str(root / f"first-{pos}" / "observer-%d.log")]


def build_id(path):
    from elftools.elf.elffile import ELFFile
    with path.open("rb") as stream:
        note = ELFFile(stream).get_section_by_name(".note.gnu.build-id")
        ids = [n["n_desc"] for n in note.iter_notes() if n["n_type"] == "NT_GNU_BUILD_ID"]
        need(len(ids) == 1, "model build ID")
        return ids[0]


def freeze(a):
    root, model = a.directory.resolve(), a.qemu.resolve(strict=True)
    need(sys.platform.startswith("linux") and "," not in str(root) and
         len(bytes(root / "first-2/status.sock")) < 108, "native Linux cohort path")
    need(sha(model) == a.model_sha == MODEL_SHA and build_id(model) == a.build_id == MODEL_BUILD_ID,
         "rebuilt model differs from Integration pins")
    need(sha(a.header) == HEADER_SHA, "matched API-7 public header")
    receipt = read_json(a.receipt)
    expected = {"model_sha256": a.model_sha, "model_build_id": a.build_id,
                "header_sha256": HEADER_SHA, "plugin_sha256": sha(a.plugin),
                "plugin_source_sha256": tool_hashes()["pi4_mg2_sparse.c"]}
    need(all(receipt.get(k) == v for k, v in expected.items()) and
         isinstance(receipt.get("compiler_version"), str) and receipt["compiler_version"].strip() and
         isinstance(receipt.get("compiler_argv"), list) and receipt["compiler_argv"] and
         all(isinstance(arg, str) and arg for arg in receipt["compiler_argv"]) and
         receipt.get("model_attestation_sha256") == sha(a.model_receipt),
         "integration build receipt disagrees with selected artifacts")
    need(sha(ARCHIVE) == ARCHIVE_SHA, "original archive")
    root.mkdir()
    with ZipFile(ARCHIVE) as z:
        original = json.loads(z.read("clustered/freeze.json"))
        need(original["source"] == SOURCE and original["case"] == CASE and original["flags"] == FLAGS
             and original["qemu_sha256"] == QEMU_SHA, "archived profile")
        for name, digest in INPUTS.items():
            data = z.read("clustered/" + name)
            need(hashlib.sha256(data).hexdigest() == digest, name)
            (root / name).write_bytes(data)
    extra = {}
    for name, src in (("observer.so", a.plugin), ("build-receipt", a.receipt),
                      ("model-attestation", a.model_receipt), ("qemu-plugin.h", a.header)):
        (root / name).write_bytes(src.read_bytes())
        extra[name] = sha(root / name)
    record = dict(profile=PROFILE, platform="raspi4b-tcg", case=CASE, flags=FLAGS, source=SOURCE,
                  qemu=str(model), qemu_sha256=a.model_sha, build_id=a.build_id, archive=ARCHIVE_SHA,
                  hashes=INPUTS, artifacts=extra, tools=tool_hashes(), ceiling_ns=30_000_000_000,
                  quiet_ns=1_000_000_000, commands=[command(root, str(model), p) for p in (1, 2)],
                  ram=symbols_and_window((root / "kernel.elf").read_bytes()))
    save(root / "freeze.json", record)
    print(sha(root / "freeze.json"))
    return 0


def frozen(a):
    root = a.directory.resolve()
    need(sha(root / "freeze.json") == a.digest, "freeze digest")
    f = read_json(root / "freeze.json")
    need(f["profile"] == PROFILE and f["case"] == CASE and f["source"] == SOURCE and
         f["qemu_sha256"] == MODEL_SHA and f["build_id"] == MODEL_BUILD_ID and
         f["archive"] == ARCHIVE_SHA and f["hashes"] == INPUTS and f["flags"] == FLAGS and
         f["tools"] == tool_hashes(), "fixed observer profile")
    for name, digest in (INPUTS | f["artifacts"]).items():
        need(sha(root / name) == digest, name)
    return root, f


def run(a):
    root, f = frozen(a)
    need(f["commands"][a.ordinal - 1] == command(root, f["qemu"], a.ordinal), "launch path changed")
    if a.ordinal == 2:
        prior = read_json(root / "first-1/session.json")
        need(prior["freeze_sha256"] == a.digest and prior["ordinal"] == 1 and
             prior["argv"] == f["commands"][0], "paired first attempt")
    return acquire(a, Observer(root, a.ordinal))


def causal_order(events):
    if set(events) != {0, 1}:
        return "missing actor"
    flags = (events[0][0], events[1][0])
    need(flags != (1, 1), "contradictory atomic handshakes")
    return {(0, 0): "unresolved", (1, 0): "SGIR before final M commit",
            (0, 1): "final M commit before SGIR"}[flags]


def reconcile(events, snap):
    need(not snap["flags"]["INVALID"] and all(j["lost"] == 0 for j in snap["journals"]),
         "invalid or lost journal")
    m, s = (j["rows"] for j in snap["journals"])
    final = [r for r in m if r["kind"] == 5 and r["words"][0] == 0 and r["words"][3] == 65536]
    sends = [r for r in s if r["kind"] == 7 and tuple(r["words"]) == (0, 0x18001, 0, 0, 0, 0, 0, 0)]
    if 0 in events:
        _, count, lease = events[0]
        need(len(final) == 1 and lease > 0, "unique final checkpoint")
        r = final[0]
        w = r["words"]
        dispatch = [d for d in m[:r["seq"]] if d["kind"] == 11]
        need(r["seq"] + 1 == count and w[1] == w[2] == lease and w[4] == 2147516416
             and w[6] == 0x801f0 and dispatch and tuple(dispatch[-1]["words"][:3]) == (0, 0, lease),
             "committed final M identity")
    if 1 in events:
        need(events[1][1:] == (0x18001, 0) and len(sends) == 1, "CPU1 opening transaction")
        earlier = s[:sends[0]["seq"]]
        retire = [r for r in earlier if r["kind"] == 3 and tuple(r["words"][:2]) == (0, 2)]
        need(len(retire) == 1 and retire[0]["words"][2] > 0, "opening P1 retirement")
    return causal_order(events)


def replay(a):
    root, f = frozen(a)
    folder = root / f"first-{a.ordinal}"
    need(sha(folder / "session.json") == a.session, "session digest")
    session = read_json(folder / "session.json")
    need(session["argv"] == f["commands"][a.ordinal-1] and session["diagnostic"] == PROFILE and
         session["ordinal"] == a.ordinal and session["case"] == CASE and session["freeze_sha256"] == a.digest,
         "session identity")
    hashes = session["hashes"]
    need(set(hashes) == {p.name for p in folder.iterdir() if p.is_file() and p.name != "session.json"}
         and all(sha(folder / n) == v for n, v in hashes.items()), "retained file identity")
    tids = read_json(folder / "threads.json")
    events, lines = {}, {}
    for name in hashes:
        tid = re.fullmatch(r"observer-(\d+)\.log", name)
        if not tid:
            continue
        need((folder / name).stat().st_size <= 64 * 1024 * 1024, "oversize observer stream")
        with (folder / name).open(encoding="ascii") as stream:
            for n, line in enumerate(stream, 1):
                if "MG2SP" not in line:
                    continue
                marker = re.fullmatch(r"MG2SP actor=([01]) peer=([01]) value=(\d+) lease=(\d+)\n", line)
                need(a.ordinal == 2 and marker, "invalid observer or unexpected control record")
                cpu, peer, value, lease = map(int, marker.groups())
                need(tids.get(tid[1]) == f"CPU {cpu}/TCG" and cpu not in events, "actor attribution/duplicate")
                events[cpu] = (peer, value, lease)
                lines[cpu] = (name, n)
    report = dict(position=a.ordinal, terminal=session["terminal"], reason=session["reason"],
                  elapsed_ns=session["elapsed_ns"], quiet_ns=session["quiet_ns"], events=events,
                  lines=lines, order="unresolved", qualification="diagnostic only")
    ok = False
    if session.get("post_terminal", {}).get("status") == "captured":
        need(session["reason"] == "terminal_quiet" and 10**9 <= session["quiet_ns"] <=
             session["elapsed_ns"] <= 30*10**9, "capture bounds")
        q = read_json(folder / "qmp.json")
        need(q["before"]["running"] and q["after"]["status"] == "paused" and
             session["observation_monotonic_ns"]["end"] <= q["stop_before_ns"] <= q["stop_after_ns"] and
             len(tids) == 4 and len(q["cpus"]) == 4 and
             {c["cpu-index"] for c in q["cpus"]} == set(range(4)) and
             all(tids.get(str(c["thread-id"])) == f"CPU {c['cpu-index']}/TCG" for c in q["cpus"]),
             "QMP attribution/stop")
        snap = rows_and_frames((folder / "terminal-ram.bin").read_bytes(), f["ram"])
        report["guest"] = observations(folder, session, f, snap)
        try:
            observed_threads(folder, hashes)
            report["order"] = reconcile(events, snap) if a.ordinal == 2 else "control"
            ok = a.ordinal == 1 or len(events) == 2
        except Unobserved as exc:
            report["missing"] = str(exc)
    print(json.dumps(report, indent=2))
    return 0 if ok else 2


def main():
    p = argparse.ArgumentParser(description=__doc__)
    commands = p.add_subparsers(required=True)
    for handler in (freeze, run, replay):
        sub = commands.add_parser(handler.__name__)
        sub.set_defaults(handler=handler)
        sub.add_argument("directory", type=Path)
        if handler == freeze:
            for name in ("qemu", "plugin", "receipt", "model_receipt", "header"):
                sub.add_argument(name, type=Path)
            sub.add_argument("model_sha")
            sub.add_argument("build_id")
        else:
            sub.add_argument("digest")
            sub.add_argument("ordinal", type=int, choices=(1, 2))
            if handler == replay:
                sub.add_argument("session")
    a = p.parse_args()
    try:
        return a.handler(a)
    except (ValueError, OSError, RuntimeError, KeyError, TypeError, ImportError) as exc:
        p.exit(1, f"{exc}\n")


if __name__ == "__main__":
    sys.exit(main())
