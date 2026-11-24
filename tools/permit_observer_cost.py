"""Freeze and collect CHG-061 using only the approved read-only OFF workloads."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import struct
import subprocess
import sys
import time

from permit_cost_retention import exact_files, fingerprint, publish, request_stop, sync_dir, termination_output, verify
from permit_cost_guard import ContainmentError, DEVICE, FORMATS, Guard, Lines, REDUCED

QEMU = "0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8"
CATALOG = "6cc8a670c6f85d1e8f77cb495b4b1b28573518f1f68c8bef684b4c53a2b27d5d"
IMAGES = {
    "rm-off-1": "576623513c85e0e4355d24e56278e21259a22f0a0f245fdcd38dc5078f89d1a8",
    "rm-off-2": "28d6d97b150bd469ca3a99b3f372d660b33abef3e60900392f623a95065aeecb",
    "dm-off-1": "ba717cbaa75c001fb662dff175523d85c76ae48fa2fb9899df9dcdcf0418c35f",
    "dm-off-2": "5cf396115ec95777d6ee455c612ecb5da9ea5a5dd1a596506ccae0bec2333f92",
}
OPTIONS = "-machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -nic none -display none -monitor none -no-reboot".split()
LIMIT = 8 * 1024 * 1024
STREAMS = ("uart", "mechanism", "stderr", "stdout")


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def save(path, data):
    with path.open("x", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
        f.write("\n")


def check(path, expected):
    if digest(path) != expected:
        raise ValueError("digest mismatch: " + str(path))


def cohort():
    result = []
    for group, key in enumerate(IMAGES):
        order = "FRRF" if group % 2 == 0 else "RFFR"
        for i, arm in enumerate(order):
            result.append((f"{len(result)+1:02d}-{key}-{arm}", key, arm, i // 2 + 1))
    return result


def mount(root):
    if (sys.platform != "linux" or Path("/home") not in root.parents
            or root.parent.stat().st_uid != os.getuid()):
        raise ValueError("use an owned persistent staging parent below /home")
    kind = subprocess.check_output(["findmnt", "-n", "-o", "FSTYPE", "-T", str(root.parent)], text=True).strip()
    if kind != "ext4":
        raise ValueError("EVD-054 comparison requires native ext4")
    return kind


def freeze(args):
    root = args.root.resolve()
    filesystem = mount(root)
    q = args.qemu.resolve()
    catalog = args.catalog.resolve()
    check(q, QEMU)
    check(catalog, CATALOG)
    names = [l for l in catalog.read_text().splitlines() if l.startswith("pl061_")]
    if {s.split("(")[0] for s in names} != {"pl061_" + n for n in FORMATS}:
        raise ValueError("installed PL061 event dictionary differs")
    replica = args.replica.resolve()
    if (replica == root or root in replica.parents or replica in root.parents
            or replica.parent.stat().st_dev == root.parent.stat().st_dev
            or Path("/tmp") in replica.parents or Path("/run") in replica.parents):
        raise ValueError("select a separate durable mounted destination")
    replica_kind = subprocess.check_output(["findmnt","-n","-o","FSTYPE","-T",str(replica.parent)],text=True).strip()
    if replica_kind in ("tmpfs","ramfs","overlay"):
        raise ValueError("replica must survive host session cleanup")
    replica.mkdir()
    sync_dir(replica.parent)
    root.mkdir()
    sync_dir(root.parent)
    (root / "images").mkdir()
    for key, sha in IMAGES.items():
        target = root / "images" / key
        original = args.archive / key / "kernel.elf"
        check(original, sha)
        shutil.copyfile(original, target)
        check(target, sha)
    shutil.copyfile(catalog, root / "trace-events-all")
    if shutil.disk_usage(root).free < len(cohort()) * len(STREAMS) * LIMIT:
        raise ValueError("insufficient capture capacity for every frozen position")
    positions = []
    for name, key, arm, pair in cohort():
        folder = root / name
        flags = "int," + ("trace:pl061*" if arm == "F" else REDUCED)
        argv = [str(q), *OPTIONS, "-serial", "file:" + str(folder / "uart"), "-d", flags,
                "-D", str(folder / "mechanism"), "-kernel", str(root / "images" / key)]
        positions.append(dict(name=name, key=key, arm=arm, pair=pair, argv=argv))
    save(root / "freeze.json", dict(cohort_version=3, replica=str(replica), positions=positions, qemu=QEMU, catalog=CATALOG, events=names,
         device=DEVICE, images=IMAGES, filesystem=filesystem, affinity=sorted(os.sched_getaffinity(0)),
         timeout_s=10, bytes_per_stream=LIMIT, uname=list(os.uname()), scripts=script_hashes()))
    inputs = ["freeze.json","trace-events-all"] + ["images/" + k for k in IMAGES]
    publish(root, replica/"frozen-inputs", inputs, inputs)
    print(digest(root / "freeze.json"))


def script_hashes():
    here = Path(__file__).resolve().parent
    return {p: digest(here / p) for p in ("permit_observer_cost.py", "permit_cost_guard.py",
                                          "permit_phase_observations.py", "permit_cost_retention.py")}


def file_limit():
    import resource
    resource.setrlimit(resource.RLIMIT_FSIZE, (LIMIT, LIMIT))


def verify_executable_pcs(elf, guard):
    data = elf.read_bytes()
    off = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    executable = []
    for i in range(count):
        typ, flags, pos, va, _, n, _, _ = struct.unpack_from("<II6Q", data, off + i * size)
        if typ == 1 and flags & 1:
            executable.append((va, va+n, pos))
    for pc in guard.irqs + guard.returns + guard.svcs:
        if pc % 4 or not any(a <= pc < b for a,b,_ in executable):
            raise ValueError("exception PC outside frozen executable")
    for pc in guard.svcs:
        for a,b,pos in executable:
            if a <= pc-4 and pc <= b:
                insn = struct.unpack_from("<I", data, pos + pc-4-a)[0]
                if insn & 0xffe0001f == 0xd4000001:
                    break
        else:
            raise ValueError("SVC return address does not follow a linked SVC")


def run_one(root, pos, interrupted):
    folder = root / pos["name"]
    folder.mkdir()
    guard = Guard(pos["arm"])
    parsers = {"uart": Lines(guard.uart), "mechanism": Lines(guard.mechanism)}
    offsets = dict.fromkeys(parsers, 0)
    start = time.monotonic_ns()
    result = dict(position=pos["name"], start_ns=start, first_seen_ns={}, disposition="incomplete")
    proc = None
    def drain():
        moved = False
        for name, parser in parsers.items():
            path = folder / name
            if not path.exists():
                continue
            with path.open("rb") as f:
                f.seek(offsets[name])
                chunk = f.read(65536)
            if chunk:
                result["first_seen_ns"].setdefault(name, time.monotonic_ns())
                moved = True
                offsets[name] += len(chunk)
                parser.feed(chunk)
        for name in STREAMS:
            path = folder / name
            if path.exists() and path.stat().st_size >= LIMIT:
                raise ValueError("stream capacity reached: " + name)
        return moved
    def stop(reason):
        if proc is None or proc.poll() is not None:
            return
        record = request_stop(proc, reason, pos["argv"][0])
        result["termination"] = record
        try:
            proc.wait(1)
        except subprocess.TimeoutExpired:
            record["escalated"] = True
            proc.kill()
            proc.wait()
    try:
        with (folder/"stdout").open("xb") as out, (folder/"stderr").open("xb") as err:
            proc = subprocess.Popen(pos["argv"], stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                    preexec_fn=file_limit, start_new_session=True)
            while True:
                moved = drain()
                if interrupted:
                    raise InterruptedError(str(interrupted[0]))
                if guard.terminal:
                    result["terminal_seen_ns"] = time.monotonic_ns()
                    break
                if proc.poll() is not None and not moved:
                    raise ValueError("guest exited before terminal")
                if time.monotonic_ns() - start >= 10_000_000_000:
                    raise TimeoutError("10 s host ceiling")
                if not moved:
                    time.sleep(0.001)
            if (folder/"stderr").stat().st_size or (folder/"stdout").stat().st_size:
                raise ValueError("unexpected emulator output before collector termination")
            stop("guest-terminal")
            while drain():
                pass
            for parser in parsers.values():
                parser.finish()
            if interrupted:
                raise InterruptedError(str(interrupted[0]))
            result["termination_output"] = termination_output(
                (folder/"stdout").read_bytes(), (folder/"stderr").read_bytes(),
                result.get("termination"), proc.returncode)

            result["observation"] = guard.join(folder/"uart", pos["key"])
            verify_executable_pcs(root/"images"/pos["key"], guard)
            result["disposition"] = "observed-off-only"
    except (OSError, ValueError, UnicodeError, RuntimeError) as error:
        result.update(first_failure=type(error).__name__ + ": " + str(error),
                      alarm_ns=time.monotonic_ns(),
                      disposition="containment" if isinstance(error, ContainmentError) else "incomplete")
    finally:
        try:
            stop(result.get("first_failure", "cleanup"))
        except (OSError, ValueError) as err:
            result["cleanup_error"] = str(err)
            result.setdefault("first_failure", str(err))
            result["disposition"] = "incomplete"
            if proc and proc.poll() is None:
                proc.kill()
                proc.wait()
        result.update(stop_ns=time.monotonic_ns(), host_status=proc.returncode if proc else None,
                      consumed=offsets, files={name: fingerprint(folder/name) for name in STREAMS
                                              if (folder/name).is_file() and not (folder/name).is_symlink()})
        save(folder/"result.json", result)
    return result


def paired_deltas(positions, results):
    by_name = {r["position"]: r for r in results}
    pairs = []
    for i in range(0, len(positions), 2):
        arms = {p["arm"]: by_name.get(p["name"], {}) for p in positions[i:i+2]}
        values = {}
        for metric in ("first_service", "first_phase_delay", "first_irq_delay"):
            arm_values = []
            for arm in ("F", "R"):
                obs = arms[arm].get("observation", {})
                value = next(iter(obs.get("irq_delays", [])), None) if metric == "first_irq_delay" else obs.get(metric)
                arm_values.append(value)
            values[metric] = arm_values[1] - arm_values[0] if None not in arm_values else None
        pairs.append({"positions": [p["name"] for p in positions[i:i+2]], "R_minus_F_ticks": values})
    return pairs


def collect(args):
    root = args.root.resolve()
    mount(root)
    check(root/"freeze.json", args.sha256)
    data = json.loads((root/"freeze.json").read_bytes())
    if data["scripts"] != script_hashes() or data["affinity"] != sorted(os.sched_getaffinity(0)):
        raise ValueError("monitor code or host CPU affinity changed")
    check(root/"trace-events-all", CATALOG)
    replica = Path(data["replica"])
    if data.get("cohort_version") != 3 or replica.stat().st_dev == root.stat().st_dev:
        raise ValueError("wrong cohort version or replica mount")
    frozen = replica/"frozen-inputs"
    inputs = ["freeze.json","trace-events-all"] + ["images/" + k for k in IMAGES]
    verify(root, frozen, inputs, exact_files(root, inputs))
    expected = cohort()
    if [(p["name"],p["key"],p["arm"],p["pair"]) for p in data["positions"]] != expected:
        raise ValueError("unexpected cohort")
    interrupted = []
    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, lambda num, frame: interrupted.append(num))
    (root/"started").touch(exist_ok=False)
    close = root/"cohort-close"
    close.mkdir()
    results = []
    receipts = []
    for p in data["positions"]:
        q = Path(p["argv"][0])
        check(q, QEMU)
        check(root/"images"/p["key"], IMAGES[p["key"]])
        folder = root/p["name"]
        flags = "int," + ("trace:pl061*" if p["arm"] == "F" else REDUCED)
        if p["argv"] != [str(q), *OPTIONS, "-serial", "file:"+str(folder/"uart"), "-d", flags,
                         "-D", str(folder/"mechanism"), "-kernel", str(root/"images"/p["key"])]:
            raise ValueError("noncanonical frozen invocation")
        if interrupted:
            break
        if shutil.disk_usage(root).free < len(STREAMS) * LIMIT:
            raise ValueError("insufficient reserved position headroom")
        r = run_one(root,p,interrupted)
        results.append(r)
        names = [*r["files"], "result.json"]
        required = [*STREAMS, "result.json"] if r["disposition"] == "observed-off-only" else ["result.json"]
        receipt = dict(position=p["name"], local=str(folder), replica=str(replica/p["name"]),
                       present=names, missing=[n for n in STREAMS if n not in names],
                       transfer_start_ns=time.monotonic_ns(), replica_verified=False)
        try:
            expected_files = exact_files(folder, names)
            if any(expected_files[n] != record for n, record in r["files"].items()):
                raise ValueError("post-stop stream identity changed")
            receipt["files"] = publish(folder, replica/p["name"], names, required, expected_files)
            receipt["replica_verified"] = True
        except (OSError, ValueError) as err:
            receipt["retention_error"] = str(err)
        receipt["transfer_end_ns"] = time.monotonic_ns()
        receipts.append(receipt)
        save(close/(p["name"] + "-retention.json"), receipt)
        fingerprint(close/(p["name"] + "-retention.json"))
        sync_dir(close)
        if not receipt["replica_verified"] or r["disposition"] != "observed-off-only":
            break
    save(close/"cohort.json", dict(version=3, intended=16, observed=results, recoverability=receipts,
         pairs=paired_deltas(data["positions"], results), unlaunched=[p["name"] for p in data["positions"][len(results):]], operational_credit=False))
    final = ["cohort.json"] + [r["position"]+"-retention.json" for r in receipts]
    publish(close, replica/"cohort-close", final, final)
    return 0 if all(r["replica_verified"] for r in receipts) and len(results)==16 and all(r["disposition"]=="observed-off-only" for r in results) else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    f = sub.add_parser("freeze")
    f.add_argument("archive", type=Path, help="EVD-054 extracted build directory")
    f.add_argument("root", type=Path)
    f.add_argument("replica", type=Path, help="new session on a separate persistent filesystem")
    f.add_argument("--qemu", type=Path, default=Path("/usr/bin/qemu-system-aarch64"))
    f.add_argument("--catalog", type=Path, default=Path("/usr/share/qemu/trace-events-all"))
    r = sub.add_parser("collect")
    r.add_argument("root", type=Path)
    r.add_argument("sha256", help="precommitted digest printed by freeze")
    opts = parser.parse_args()
    if opts.action == "freeze":
        freeze(opts)
    else:
        sys.exit(collect(opts))
