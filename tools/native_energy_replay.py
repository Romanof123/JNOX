"""Independently score one sealed Pi 4 modeled-placement first position."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

from native_energy_capture import expected
from native_energy_profile import parse_profile
from ordinary_amc_capture import sha
from pi4_native_smp_replay import Ram, one, require, symbol

NONE = 2**64 - 1


def model(profile, now):
    candidates = []
    selected = None
    for group, points in ((1, profile["points"]), (2, [profile["fallback"]] if profile["fallback"] else [])):
        feasible = []
        for p in points:
            cpu = p["cpu"]
            limit = profile["limits"][cpu]
            allowed = bool(profile["allowed_cpus"] & (1 << cpu)) and limit["available"]
            capacity = max(0, min(p["capacity"], limit["thermal_capacity"]) -
                           limit["shared_load"] - limit["reserve_capacity"]) if allowed else 0
            eligible = bool(profile["work"] and capacity and capacity >= profile["minimum_capacity"])
            run = (profile["work"] + capacity - 1) // capacity if eligible else None
            finish = now + p["wake_ticks"] + run if eligible else None
            slack = profile["deadline_ticks"] - finish if eligible else None
            cost = profile["work"] * p["energy_per_work"] + p["wake_energy"]
            item = dict(group=group, point=p, limits=limit, allowed=allowed, capacity=capacity,
                        eligible=eligible, run=run, finish=finish, slack=slack, energy=cost)
            candidates.append(item)
            if eligible and slack >= profile["deadline_reserve"]:
                feasible.append((cost, cpu, p["opp"], item))
        if selected is None and feasible:
            selected = min(feasible, key=lambda row: row[:3])[3]
    if not selected:
        return (3 if any(p["eligible"] for p in candidates) else 0, *([NONE] * 8)), candidates
    p = selected["point"]
    cost = selected["energy"]
    return (selected["group"], p["cpu"], p["opp"], selected["capacity"], selected["run"],
            p["wake_ticks"], selected["slack"], cost & NONE, cost >> 64), candidates


def inspect(args):
    root = args.capture
    require(sha(root / "session.json") == args.session_sha256, "session SHA-256")
    session = json.loads((root / "session.json").read_bytes())
    files = session["files"]
    for name, digest in files.items():
        require(Path(name).name == name and sha(root / name) == digest, f"artifact {name}")
    require({"position.json", "placement-source.json"} <= files.keys(), "unsealed position or source")
    pos = json.loads((root / "position.json").read_bytes())
    require(pos["case"] == args.case and pos["elf_sha256"] == args.elf_sha256, "predeclared position/ELF")
    source = (root / "placement-source.json").read_bytes()
    require(json.loads(source) == expected(args.case), "frozen profile")
    if args.case == "missing-limit":
        cmd = pos["command"]
        require({"stderr", "stdout"} <= files.keys() and session["reason"] == "cli-return" and
                pos["status"] == 2 and pos["package_created"] is False and
                len(cmd) == 12 and Path(cmd[1]).name == "build_native_el0.py" and
                cmd[2:7] == ["--board","pi4","--policy","energy-placement","--elf"] and
                cmd[8] == "--placement-profile" and Path(cmd[9]).name == "placement-source.json" and
                cmd[10] == "--out" and Path(cmd[11]).name == "package" and
                b"limits requires exactly CPU0 and CPU1" in (root / "stderr").read_bytes(),
                "missing-limit refusal")
        return dict(case=args.case, status=2, stage="package input validation")

    required = {"uart.bin", "ram.bin", "manifest.json", "launch.json", "cpus.json", "kernel8.img",
                "kernel.elf", "kernel.elf.nm", "placement-v1.json"}
    require(required <= files.keys() and session["reason"] == "terminal", "incomplete terminal capture")
    manifest = json.loads((root / "manifest.json").read_bytes())
    launch = json.loads((root / "launch.json").read_bytes())
    require(args.manifest_sha256 == pos["manifest_sha256"] == files["manifest.json"], "frozen manifest")
    require(manifest["policy"] == "energy-placement" and manifest["board"] == "pi4", "package mode")
    slot = one(manifest["slots"])
    elf = slot["elf"]
    require(elf in files and elf + ".nm" in files and files[elf] == args.elf_sha256 == slot["elf_sha256"]
            and files["kernel8.img"] == manifest["kernel8_img_sha256"]
            and files["kernel.elf"] == manifest["kernel_elf_sha256"], "packaged images")
    argv = json.dumps(slot["argv"], separators=(",", ":"), ensure_ascii=False).encode()
    require(hashlib.sha256(argv).hexdigest() == slot["argv_sha256"], "private argv identity")
    profile = parse_profile(source)
    require(profile.wire == (root / "placement-v1.json").read_bytes() and
            profile.source_sha256 == manifest["placement_input"]["sha256"] and
            profile.wire_sha256 == manifest["placement_canonical"]["sha256"], "profile provenance")
    require(bool(args.trace) == pos["trace"] == launch["trace"] and
            launch["argv"][1:5] == ["-M","raspi4b","-accel","tcg"] and
            {r["cpu-index"] for r in json.loads((root / "cpus.json").read_bytes())} == {0,1,2,3},
            "stock model, CPU inventory or plain/traced position")
    uart = (root / "uart.bin").read_text("ascii").splitlines()

    def rows(tag):
        prefix = "NATIVE SMP " + tag + " "
        return [tuple(map(int, line[len(prefix):].split())) for line in uart if line.startswith(prefix)]

    version, freq, q, epoch, count, idle = one(rows("CONFIG"))
    e = one(rows("ENERGY"))
    require(len(e) == 23 and (version, count, idle) == (1, 1, 1) and freq >= 50 and
            e[:4] == (1, freq, freq // 10, epoch) and q == freq // 50, "configuration/units")
    tick, release, start, end = e[2], *e[4:7]
    require(epoch <= release <= start <= end and e[7] == (start - epoch + tick - 1) // tick and
            e[8] == epoch + profile.normalized["deadline_ticks"] * tick <= NONE, "anchored timing")
    decision, candidates = model(profile.normalized, e[7])
    require(e[9:18] == decision, "independent model disposition and witness")
    disposition, cpu = decision[:2]
    executed = disposition in (1, 2)
    before, sample, after = one(rows("CAL"))
    require(0 < before <= after and sample > 0, "CPU1 counter bracket")
    offset = (before - sample, after - sample) if cpu == 1 else (0, 0)
    require(one(rows("END")) == (1, int(executed), 1, 1), "release and park acknowledgments")
    ram = Ram(root, launch["ram_base"])
    require(len(ram.data) == launch["ram_end"] - ram.base, "terminal RAM extent")
    ksym = (root / "kernel.elf.nm").read_text().splitlines()
    def address(name):
        fields = one([line.split() for line in ksym if len(line.split()) == 4 and name in line.split()[3]])
        return int(fields[0], 16), int(fields[1], 16)
    private_root = (1 << 48) | address("6SPACES")[0]
    events, event_size = address("native_smp_events")
    frames, frame_size = address("native_smp_frames")
    require(event_size == 2*256*37*8 and frame_size == 2*256*2*816, "linked journal extents")
    entry = struct.unpack_from("<Q", (root / elf).read_bytes(), 24)[0]
    release_rows, acquisitions, dispatches, user, exits, productive, mail = [], [], [], [], [], [], []
    work_pc, work_bytes = symbol(root, "count_low_primes", elf)
    charge = 0
    journals = sorted(rows("JOURNAL"))
    require(len(journals) == 2, "both CPU journals")
    for c, j in enumerate(journals):
        core, addr, used, words, drops, fa, size = j
        require((core, addr, words, drops, fa, size) ==
                (c, events+c*event_size//2, 37, 0, frames+c*frame_size//2, 816)
                and 0 < used <= 256, "journal bounds, identity or loss")
        owner, last = idle, 0
        continuation = None
        for i in range(used):
            r = ram.words(addr+i*37*8, 37)
            f = ram.words(fa+i*1632, 102)
            g = ram.words(fa+i*1632+816, 102)
            src, dst = r[6:8]
            require(r[:2] == (c, i) and src == owner and dst in (0, idle) and
                    last <= r[4] <= r[5] and r[2] in (1, 2), "event chain")
            if r[14]:
                require(c == 0 and r[14] == 1 and r[5] == release and r[15] >= end, "release event")
                release_rows.append(r)
            if r[18]:
                require(c == cpu and r[18] == 1 and dst == 0, "first acquisition")
                acquisitions.append(r)
            if r[17]:
                require(c == 1 and r[17] == 1 and r[2] == 1 and r[3] & 0x1fff == 1,
                        "remote release mailbox")
                mail.append(r)
            if src == 0:
                require(c == cpu and r[21] == private_root and 0 < r[8] <= r[4] and
                        r[8] >= last and r[9] == r[4]-r[8], "owner service and private root")
                charge += r[9]
                require(r[28] == charge and f[32:34] == r[30:32] and f[33] & 31 == 0,
                        "saved EL0 origin")
                pc_pa, pte = ram.walk(private_root, f[32])
                require(pc_pa and pte & 64 and pte & 128 and not pte & (1<<54), "EL0 executable mapping")
                user.append(r)
                if r[2] == 1 and r[3] & 1023 == 30 and work_pc <= f[32] < work_pc + work_bytes:
                    productive.append((r[4], f[32]))
                continuation = list(f)
                if r[2] == 2:
                    require(f[8] == r[3] and f[0] == r[23] and r[25] >> 26 == 0x15, "saved syscall")
                    continuation[0] = r[24]
                    if r[3] == 1:
                        require(dst == idle and f[0] == 12, "signed prime exit")
                        exits.append(r)
            else:
                require(r[2] == 1 and r[9] == 0 and r[31] & 31 in (4,5), "idle EL1 event")
            if dst == 0:
                require(c == cpu and r[22] == private_root and g[33] & 31 == 0, "target root/EL0")
                if continuation is None:
                    require(g[32] == entry and g[31] != 0, "first prepared ELF entry")
                    dispatches.append(r)
                else:
                    require(tuple(continuation) == g, "saved/prepared continuation")
            owner, last = dst, r[5]
        require(owner == idle, "last CPU owner")
        if c == 1:
            require(r[2] == 1 and r[3] & 0x1fff == 2, "CPU1 stop SGI")
    pub = one(release_rows)
    if cpu == 1:
        irq = one(mail)
        require(pub[15] <= pub[16] <= pub[36] and irq[4]+offset[1] >= pub[16],
                "impossible remote SGI chronology")
    else:
        require(not mail and pub[16] == pub[36] == 0, "unexpected remote publication")
    terminal = [line for line in uart if line.startswith("NATIVE EL0 EXIT PID ")]
    wanted = {"base": (1,0), "thermal": (1,1), "shared": (1,1), "fallback": (2,1),
              "empty": (0,NONE), "deadline": (3,NONE), "zero-mask": (0,NONE)}
    issues = []
    if (disposition,cpu) != wanted[args.case]:
        issues.append("actual disposition differs from the frozen case")
    summary = dict(case=args.case, candidates=candidates, disposition=disposition,
                   decision_counter=[start,end], publication_counter=pub[15], absolute_deadline=e[8])
    if not executed:
        require(not (user or acquisitions or dispatches or exits or terminal or rows("TASK")) and
                e[18:] == (NONE,)*5, "refusal entered EL0 or reported execution")
        summary["execution"] = None
    else:
        require(one(acquisitions) == one(dispatches) and user and one(exits) == user[-1],
                "singleton dispatch and terminal service")
        require(one(rows("TASK"))[0:2] == (0,cpu) and one(rows("TASK"))[3:] == (0,charge,private_root)
                and terminal == ["NATIVE EL0 EXIT PID 0 12"], "terminal task identity and status")
        first, upper, done = user[0][8], user[0][4], exits[0][4]
        nominal = e[8]-done
        require(e[18:] == (first,upper,done,int(nominal < 0),abs(nominal)) and first <= upper <= done,
                "first return, first EL0-origin exception and signed completion margin")
        require(first+offset[1] >= end, "dispatch before decision even at latest calibrated bound")
        if first+offset[0] < end:
            issues.append("calibration leaves decision-to-dispatch ordering ambiguous")
        va, size = symbol(root, "private_result", elf)
        pa, desc = ram.walk(private_root, va)
        require(size == 4 and pa >= ram.base and desc & 64 and desc & (1<<11) and not desc & 128 and
                struct.unpack_from("<I", ram.data, pa-ram.base)[0] == 78498, "private prime result")
        margin = [nominal-offset[1], nominal-offset[0]]
        if margin[0] < 0:
            issues.append("completion exceeded deadline or its calibration interval straddled deadline")
        summary.update(cpu=cpu, first_return=first, first_EL0_exception=upper, exit_counter=done,
                       private_result=dict(va=va,pa=pa,value=78498), service=charge,
                       nominal_signed_margin=nominal, calibrated_margin=margin,
                       completion="within deadline" if margin[0]>=0 else "miss" if margin[1]<0 else "ambiguous",
                       decision_to_return=[first+o-end for o in offset],
                       first_observed_productive_ppi=productive[0] if productive else None,
                       productive_work_upper_bound=productive[0][0] if productive else done)
    if args.trace:
        require("exceptions.log" in files, "unsealed external trace")
        trace = (root / "exceptions.log").read_text(errors="replace")
        owned = [(int(c),int(pc,16)) for c,pc in re.findall(
            r"Taking exception [^\n]+ on CPU ([0-3])\s*\n\.\.\.from EL0 to EL1\n"
            r"\.\.\.with ESR [^\n]+\n\.\.\.with ELR (0x[0-9a-fA-F]+)", trace)]
        require(len(owned) == trace.count("...from EL0 to EL1") and
                owned == [(cpu,r[30]) for r in user], "external EL0 exception ownership and PCs")
        if executed:
            require(re.search(r"Exception return from AArch64 EL1 to AArch64 EL0 PC 0x0*"+f"{entry:x}"+r"\b", trace),
                    "external ELF-entry return")
        summary["external_user_exceptions"] = len(owned)
    summary["unresolved_or_missed"] = issues
    return summary


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("case")
    p.add_argument("capture", type=Path)
    p.add_argument("--session-sha256", required=True)
    p.add_argument("--elf-sha256", required=True)
    p.add_argument("--manifest-sha256")
    p.add_argument("--trace", action="store_true")
    try:
        result = inspect(p.parse_args())
        print(json.dumps(result, indent=2))
        sys.exit(2 if result.get("unresolved_or_missed") else 0)
    except (ValueError, OSError, IndexError, TypeError, KeyError, struct.error) as exc:
        sys.exit(f"invalid energy-placement evidence: {exc}")
