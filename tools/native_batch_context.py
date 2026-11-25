#!/usr/bin/env python3
"""Historical and separately observed native batch context. See native_batch_context.md."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import sys

from native_el0_capture import capture, command
from native_el0_replay import PA_MASK, Snapshot, argument_stacks, elf_file, symbol
from native_batch_replay import decode, ledger, product, require, slot_statuses
from ordinary_amc_capture import sha

CASES = ("arrival-srtf", "tie-sjf")
SOURCE = "25a41989368d4f76ec7151209e27a448b7b253e2"
QEMU = "0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8"
KERNEL = ("089ff0bc0ba855c6ff3cda7dd19af7243ed4af84017917b0ac5316e6f612da3b",
          "ab8257864df4ed0aa9c08fa18c552410b6c504ce88f39dd3054669ccbac00b2b")
MANIFEST = ("dd7ce8c45ffa06e37d93516b15aa1b1b9bc31687ca229e79c786ce40710051ae",
            "8b913d79629709fdf573841dbbd48088343be576ab952e38c323ac1d7cd090b2")
HISTORY = {
    "plain": ("51c7dffe8f519eee98bbb007f60d0cb2a1f2f2859707119990fe8f9f1790f9bf",
              "819f5c93c74f808d6a9469ed221a1f9b79dfb796097dff3edfe5807729c7b51a",
              "04a0bb2ce6b611068eabcca5c91cf839e285fb1fbc2a6cc4043d32269f44b9b4", 13721),
    "traced": ("b28ff5860ca0e5358dba3c9686e672162c46757ae8266aab6d4c7ffac2d6e4de",
               "09288bc13ba96cb13e5935a7aee033a2eb01b736ca7b87f514e485f8babdf43e",
               "4fa1e2123dbb565660b23183c7c80df60ea59364b27eeb12b6cdc4160f592d60", 13619),
}


def package(products, case):
    folder = products / case
    idx = CASES.index(case)
    require(sha(folder/"manifest.json") == MANIFEST[idx] and sha(folder/"kernel.elf") == KERNEL[idx],
            "original manifest/kernel mismatch")
    manifest = json.loads((folder/"manifest.json").read_bytes())
    for slot in manifest["slots"]:
        require(sha(folder/slot["elf"]) == slot["elf_sha256"], "original slot ELF mismatch")
    return folder, manifest


def acquisition(args):
    out = args.out.resolve()
    plugin = args.plugin.resolve(strict=True)
    require(sha(plugin) == args.plugin_sha256, "plugin digest mismatch")
    qemu = Path(shutil.which("qemu-system-aarch64") or "").resolve(strict=True)
    require(sha(qemu) == QEMU, "original QEMU mismatch")
    for case in CASES:
        package(args.products, case)
    probe = args.products/"first-arrival-srtf-traced"/"probe.c"
    require(sha(probe) == "ac5bf6aed8dc677df141d2a5e0029214f459b7c85162179fb99052360b532870", "probe source")
    require(not any("," in str(p) for p in (out, plugin)), "QEMU observer paths cannot contain commas")
    out.mkdir(mode=0o700)
    results = []
    for case in CASES:
        folder, _ = package(args.products, case)
        target = out/case
        def observed(qemu_path, root, plain):
            cmd = command(qemu_path, root, False)
            root.chmod(0o700)
            ram = root/"observer.ram"
            fd = os.open(ram, os.O_CREAT | os.O_EXCL | os.O_RDWR, 0o600)
            with os.fdopen(fd, "wb") as stream:
                stream.truncate(128 << 20)
            st = ram.stat()
            (root/"backing.json").write_text(json.dumps(dict(dev=st.st_dev,ino=st.st_ino,bytes=st.st_size)))
            shutil.copyfile(plugin, root/"observer.so")
            require(sha(root/"observer.so") == args.plugin_sha256, "copied plugin digest")
            cmd[cmd.index("-machine")+1] += ",memory-backend=obsram"
            cmd.extend(["-object", f"memory-backend-file,id=obsram,size=128M,mem-path={ram},share=on",
                        "-plugin", f"{root/'observer.so'},case={case},out={root/'context.jsonl'},ram={ram}"])
            return cmd
        rc = capture(argparse.Namespace(root=target,kernel=folder/"kernel.elf",user=folder/"user.elf",
             probe=probe,revision=SOURCE,manifest=folder/"manifest.json",arg=[],peer_arg=[],peer2_arg=[],
             expect_exit=17,timeout=90,uart_limit=None,qemu=str(qemu),nm="aarch64-linux-gnu-nm",
             plain=False), command_factory=observed)
        results.append(dict(case=case, session_sha256=sha(target/"session.json"), capture_exit=rc))
    print(json.dumps(results, indent=2))
    return int(any(r["capture_exit"] for r in results))


def inputs(products, case, target, session_hash):
    folder, manifest = package(products, case)
    require(sha(target/"session.json") == session_hash, "session mismatch")
    session = json.loads((target/"session.json").read_bytes())
    for name, digest in session["hashes"].items():
        require(Path(name).name == name and sha(target/name) == digest, "capture hash mismatch " + name)
    require(session["marker"] == "EXIT 17", "incomplete capture " + session["marker"])
    launch = json.loads((target/"launch.json").read_bytes())
    require(launch["argument_package"] == manifest and launch["revision"] == SOURCE and
            launch["qemu_sha256"] == QEMU and session["hashes"]["kernel.elf"] == manifest["kernel_elf_sha256"],
            "launch identity")
    mem = Snapshot(launch["ram_base"], (target/"ram.bin").read_bytes())
    uart = (target/"uart.bin").read_bytes()
    stacks = argument_stacks((target/"kernel.elf").read_bytes(), mem, manifest, uart)
    statuses = slot_statuses(uart, len(stacks))
    outputs = [product(role, elf_file((folder/s["elf"]).read_bytes()), mem, stacks[i], statuses[i])
               for i,(s,role) in enumerate(zip(manifest["slots"],("batch","urgent","checksum")))]
    timeline = ledger(decode(uart),manifest,stacks)
    require(timeline["frequency"] == 62500000 and not timeline["dropped"] and
            timeline["events"][-1]["dst"] == 4, "incomplete terminal ledger")
    return session, mem, stacks, outputs, timeline["events"]


def original(args):
    result = {}
    app = elf_file((args.products/CASES[0]/"user.elf").read_bytes())
    for pc,word in ((0x400808,0x3868692a),(0x400a20,0x52800035),(0x400a2c,0x97ffff73),
                    (0x400a38,0xf9000688),(0x400a3c,0xb90012d5),(0x400a40,0x110006b5)):
        require(any(seg["p_type"] == "PT_LOAD" and seg["p_flags"] & 1 and
                    0 <= pc-seg["p_vaddr"] <= seg["p_filesz"]-4 and
                    struct.unpack_from("<I",seg.data(),pc-seg["p_vaddr"])[0] == word
                    for seg in app.iter_segments()), "archived loop disassembly")
    for name,(pin,ram_digest,gpr_digest,blocks) in HISTORY.items():
        target = args.products / ("first-arrival-srtf-"+name)
        session, mem, _, outputs, events = inputs(args.products,CASES[0],target,pin)
        require(session["hashes"]["ram.bin"] == ram_digest, "original RAM")
        switch, = [e for e in events if (e["src"],e["dst"]) == (0,1)]
        restore, = [e for e in events if (e["src"],e["dst"]) == (1,0)]
        require(switch["kind"] == 1 and switch["op"] & 1023 == 30 and switch["releases"] == 2
                and switch["seq"] < restore["seq"] and events[-1]["src"] == 0, "original switch")
        addr = switch["src_frame"]
        require(addr == restore["dst_frame"] == 0x40210000 and addr-mem.base == 65536, "frame identity")
        frame = mem.read(addr,832)
        gprs = struct.unpack_from("<31Q",frame)
        pstate, pc, sp = struct.unpack_from("<3Q",frame,256)
        require((pc,sp,pstate) == (switch["src_pc"],switch["src_sp"],switch["src_pstate"]) ==
                (restore["dst_pc"],restore["dst_sp"],restore["dst_pstate"]) and pc == 0x400808,
                "original saved/selected PC, stack or PSTATE")
        require(hashlib.sha256(frame[:248]).hexdigest() == gpr_digest and
                (gprs[21] & 0xffffffff)-1 == blocks, "historical GPR bytes")
        if name == "traced":
            require(trace_matches(target,switch,switch["dst_pc"]) and trace_matches(target,restore,pc),
                    "archived exception/ERET sequence")
        result[name] = dict(session=pin, frame_pa=addr, gpr_sha256=gpr_digest,x0_to_x30=gprs,
                            pc=pc,sp=sp,pstate=pstate,inferred_completed_stores=blocks,products=outputs,
                            live_restore_operand=None, transient_private_word=None)
    print(json.dumps(result,indent=2))
    return 0

def trace_matches(target, event, next_pc):
    raw = (target/"exceptions.log").read_text(encoding="ascii")
    name = "5 [IRQ]" if event["kind"] == 1 else "2 [SVC]"
    pattern = (r"Taking exception " + re.escape(name) + r"(?: on CPU 0)?\n\.\.\.from EL0 to EL1\n"
               r"\.\.\.with ESR [^\n]+\n\.\.\.with ELR 0x0*" + f"{event['src_pc']:x}" +
               r"\n\.\.\.to EL1 PC [^\n]+\nException return from AArch64 EL1 to AArch64 EL0 PC 0x0*" +
               f"{next_pc:x}" + r"(?:\n|$)")
    return bool(re.search(pattern,raw))


def clock_return_recheck(event):
    # The vector calls step(..., 3, 0, 0, 0, frame, Keep) only when its
    # sampled counter has reached the armed compare. No new EL0 lease closes.
    return (event["kind"] == 3 and event["op"] == event["arg"] == event["result"] == 0
            and event["src"] == event["dst"] == 0 and event["src_pc"] == 0x40029c
            and event["transparent_count"] > 0 and event["transparent_ticks"] > 0
            and event["segment"] == event["last_return"] == event["eoi"] == 0
            and 0 < event["prior_compare"] == event["cval"] <= event["entry"]
            and event["ctl"] & 3 == 1 and event["entry"] <= event["decision"] <= event["returned"]
            and event["src_pstate"] & 31 == 0 and event["src_root"] >> 48 == 1
            and all(event["src_"+k] == event["dst_"+k] for k in ("pc","sp","pstate","frame","root")))


def timed_sjf_retention(target, app_path, events, release, first, terminal, result, measured=False):
    # This inference belongs to the pinned no-argument batch.elf. Its one setup
    # clock precedes the loop, then each sum/progress store precedes one clock.
    app = elf_file(app_path.read_bytes())
    words = {0x400854:0x97fffe8c, 0x400948:0x97ffffac, 0x40095c:0xf90006c8,
             0x400960:0xb9001314, 0x400964:0x97fffe48, 0x400294:0x528000c8,
             0x400298:0xd4094021}
    for pc, word in words.items():
        require(any(seg["p_type"] == "PT_LOAD" and seg["p_flags"] & 1 and
                    0 <= pc-seg["p_vaddr"] <= seg["p_filesz"]-4 and
                    struct.unpack_from("<I",seg.data(),pc-seg["p_vaddr"])[0] == word
                    for seg in app.iter_segments()), "pinned timed-loop instruction mismatch")
    recheck = clock_return_recheck(release)
    require(release["dst"] == 0 and (recheck or (release["kind"],release["op"],release["arg"]) == (2,6,1))
            and release["result"] == 0 and
            release["src_pc"] == 0x40029c and release["releases"] == 6 and
            release["queued_slots"] == [1,2] and release["returned"] >= release["decision"] and
            all(release["src_"+k] == release["dst_"+k] for k in ("pc","sp","pstate","frame","root")),
            "clock release context or retention mismatch")
    exit_order = [e["src"] for e in events if e["kind"] == 2 and e["op"] == 1]
    require(exit_order == [0,1,2], "equal queued completion order mismatch")
    result["queued_exit_order"] = exit_order
    boundary = dict(cause="EL1 return recheck after clock SVC" if recheck else "clock_gettime SVC 6", owner=0, retained=True, peer_queue=[1,2],
                    pc=release["src_pc"], sp=release["src_sp"], pstate=release["src_pstate"],
                    root=release["src_root"], frame=release["src_frame"],
                    first_private=first, terminal_private=terminal, irq_delivery=False,
                    simultaneous_private_word=None, immediate_post_return_private_word=None)
    result["clock_release"] = boundary
    if not measured:
        result["gaps"].append("no simultaneous private word at the SVC release or first publication after its return")
    a_dispatch = next(e for e in events if e["dst"] == 0)
    a_exit = next(e for e in events if e["kind"] == 2 and e["op"] == 1 and e["src"] == 0)
    if (release["seq"] != a_dispatch["seq"]+1 or a_exit["seq"] != release["seq"]+1 or
            a_dispatch["src"] != 3 or a_dispatch["ready"] != 1):
        result["gaps"].append("additional scheduling boundaries prevent attributing transparent calls")
        return
    prior = release["transparent_count"]
    # A recheck follows the last already-counted transparent clock SVC. A
    # direct clock decision adds the deciding SVC to the transparent count.
    ordinal = prior if recheck else prior + 1
    completed = ordinal - 1  # the first clock is the pre-loop setup call
    clocks = ordinal + a_exit["transparent_count"]
    trace = (target/"exceptions.log").read_text(encoding="ascii").splitlines()
    prefix = "Exception return from AArch64 EL1 to AArch64 EL0 PC "
    start = next((n+1 for n,line in enumerate(trace) if line.startswith(prefix)), None)
    syscall = ["Taking exception 2 [SVC]", "...from EL0 to EL1", "...with ESR 0x15/0x56004a01",
               "...with ELR 0x40029c", "...to EL1 PC 0x40200c00 PSTATE 0x3c5", prefix+"0x40029c"]
    if (start is None or trace[start-1] != prefix+"0x400000" or
            clocks != terminal["count"]+1 or
            start+6*clocks+6 > len(trace) or
            any(trace[start+6*i:start+6*i+6] != syscall for i in range(clocks)) or
            trace[start+6*clocks:start+6*clocks+6] !=
            syscall[:3]+["...with ELR 0x400020", syscall[4], prefix+"0x400000"]):
        result["gaps"].append("external SVC/ERET sequence does not corroborate the clock-call inference")
        return
    boundary.update(transparent_calls_recorded=prior, transparent_clocks_before=ordinal-1,
                    release_clock_ordinal=ordinal, separate_recheck_exception=False if recheck else None,
                    total_user_clock_returns=clocks, inferred_completed_blocks=completed,
                    inferred_sum=completed*522240, prior_useful_work=completed>0)
    if completed == 0:
        result["gaps"].append("release occurred at the setup clock before useful publication")


def measured_sjf(target, rows, arrivals, events, progress_pa, result):
    observations = []
    result["measured_peer_admissions"] = observations
    result["queued_exit_order"] = [e["src"] for e in events if e["kind"] == 2 and e["op"] == 1]
    published = [r for r in rows if r["kind"] == "release"]
    later = [r for r in rows if r["kind"] == "release-post"]
    for group in (published,later):
        require(len({r["decision"] for r in group}) == len(group) and
                all(r["decision"] in {e["seq"] for e in arrivals} for r in group),
                "duplicate or unknown peer-release identity")
    for event in arrivals:
        seq = event["seq"]
        samples = [r for r in published if r["decision"] == seq]
        posts = [r for r in later if r["decision"] == seq]
        if not samples:
            result["gaps"].append(f"no sampled release for UART decision {seq}")
            continue
        sample = samples[0]
        for row in samples+posts:
            require([row[k] for k in ("decision","event_kind","op","src","dst","releases")] ==
                    [event[k] for k in ("seq","kind","op","src","dst","releases")],
                    "sampled release identity vs UART")
        require(0 <= seq < 1024 and sample["pc"] == 0x402036b8 and
                sample["pa"] == 0x4021a328 + 312*seq + 224, "actual decision publication address")
        active = event["src"] == 0
        irq = event["kind"] == 1 and event["op"] & 1023 == 30
        clock = event["kind"] == 2 and event["op"] == 6
        recheck = clock_return_recheck(event)
        entry_kind = ("timer IRQ" if irq else "clock SVC" if clock else
                      "EL1 return recheck after clock SVC" if recheck else "other")
        item = dict(decision=seq, cause=entry_kind, source=event["src"],target=event["dst"],
                    release=sample, next_publication=posts[0] if posts else None,
                    incumbent_retained=active and event["dst"] == 0,
                    prior_stores=sample["count"], prior_private_word=sample["value"])
        observations.append(item)
        if sample["value"] is None:
            result["gaps"].append(f"no qualified private word at release {seq}")
        if posts:
            post = posts[0]
            require(post["seq"] > sample["seq"] and post["pc"] == 0x400964 and
                    post["pa"] == progress_pa and post["count"] == sample["count"]+1,
                    "first subsequent A publication identity/count")
            if post["value"] is None:
                result["gaps"].append(f"no qualified private word after release {seq}")
            else:
                require(post["sum"] == 522240*post["value"], "uncompleted post-release publication")
        else:
            result["gaps"].append(f"no subsequent A publication sampled for decision {seq}")
        if not active or event["dst"] != 0 or not sample["count"]:
            result["gaps"].append(f"running A with useful prior work was not retained at decision {seq}")
        elif not (irq or clock or recheck):
            result["gaps"].append(f"unqualified exception cause at decision {seq}")
        elif not recheck and not trace_matches(target,event,event["dst_pc"]):
            result["gaps"].append(f"no matching exception/ERET for decision {seq}")
        if active and event["dst"] == 0:
            require(all(event["src_"+k] == event["dst_"+k] for k in ("pc","sp","pstate","frame","root")),
                    "SJF incumbent context differs on return")
    if result["queued_exit_order"] != [0,1,2]:
        result["gaps"].append("expected A/B/C completion order was not observed")


def observed(args):
    target = args.out / args.case
    session, mem, stacks, outputs, events = inputs(args.products,args.case,target,args.session_sha256)
    require(session["hashes"].get("observer.so") == args.plugin_sha256, "observer binary mismatch")
    launch = json.loads((target/"launch.json").read_bytes())
    cmd = launch["argv"]
    require(cmd[cmd.index("-machine")+1] == "virt,gic-version=2,memory-backend=obsram" and
            cmd[cmd.index("-d")+1] == "int" and "-plugin" in cmd and "--icount" not in cmd and
            "-icount" not in cmd, "observer clock/backing/trace")
    path = target/"context.jsonl"
    notes = []
    result = dict(case=args.case,session_sha256=args.session_sha256,products=outputs,
                  source=SOURCE,clock_scope="external observer, file-backed RAM",gaps=notes)
    def gap(condition, text):
        if not condition:
            notes.append(text)
        return condition
    if not path.exists() or "context.jsonl" not in session["hashes"]:
        notes.append("no retained plugin records")
        return result
    require(path.stat().st_size <= 4 << 20, "observer exceeded bounded output")
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    result["boundaries"] = rows
    if not gap(bool(rows), "no plugin records"):
        return result
    if not gap(not any(r.get("invalid") or r.get("dropped") or r["kind"] == "invalid-site" for r in rows),
               "unqualified callback or dropped records"):
        return result
    mapping = [r for r in rows if r["kind"] == "mapping"]
    if gap(len(mapping) == 1, "read-only backing could not be qualified"):
        backing = json.loads((target/"backing.json").read_bytes())
        require({k:mapping[0][k] for k in backing} == backing and backing["bytes"] == 128 << 20,
                "observer RAM mapping identity")
    def select(kind, low=-1, high=None):
        return [r for r in rows if r["kind"] == kind and low < r["seq"] and
                (high is None or r["seq"] < high)]
    require(all(a["seq"] < b["seq"] for a,b in zip(rows,rows[1:])), "plugin sequence regression")
    first = select("first")
    terminal = select("terminal")
    progress = outputs[0]["private_progress"]
    if not gap(len(terminal) == 1 and len(first) == 1, "missing first or terminal publication boundary"):
        return result
    require(first[0]["count"] == 1 and terminal[0]["count"] == progress,
            "store count vs private terminal progress")
    progress_pa = (mem.leaf(stacks[0]["root"],0x403010) & PA_MASK) + 0x10
    require(all(r["pa"] == progress_pa for r in first+select("post-progress")), "private A backing")
    for r in rows:
        if r.get("value") is not None:
            require(r["value"] == r["count"] and
                    r["sum"] in (522240*r["value"],522240*(r["value"]+1)), "snapshot product phase")
    for r in (first[0],terminal[0]):
        if r.get("value") is not None:
            require(r["sum"] == 522240*r["value"], "completed publication sum")
    gap(all(r.get("value") is not None for r in (first[0],terminal[0])), "numeric publication reads unavailable")
    release_mask = 2 if args.case == CASES[0] else 6
    arrivals = [e for e in events if e["releases"] & release_mask]
    result["release_events"] = arrivals
    corrected = "accepted" in terminal[0]
    if corrected and args.case == CASES[1]:
        measured_sjf(target,rows,arrivals,events,progress_pa,result)
        if not gap(len(arrivals) == 1 and arrivals[0]["releases"] & 6 == 6,
                   "peers did not arrive in one decision"):
            return result
        e = arrivals[0]
        if e["src"] == e["dst"] == 0 and ((e["kind"],e["op"]) == (2,6) or clock_return_recheck(e)):
            timed_sjf_retention(target,args.products/args.case/"user.elf",events,e,first[0],terminal[0],
                                result, measured=True)
            direct = result["measured_peer_admissions"]
            if direct:
                clock_row = result["clock_release"]
                clock_row["simultaneous_private_word"] = direct[0]["prior_private_word"]
                post = direct[0]["next_publication"]
                clock_row["immediate_post_return_private_word"] = post["value"] if post else None
                if direct[0]["prior_private_word"] is not None and "inferred_completed_blocks" in clock_row:
                    require(clock_row["inferred_completed_blocks"] == direct[0]["prior_private_word"],
                            "measured release word differs from clock reconstruction")
        return result
    if not gap(len(arrivals) == 1, "release grouping differs from the frozen position"):
        return result
    e = arrivals[0]
    if args.case == CASES[1] and e["src"] == 0 and e["kind"] == 2 and e["op"] == 6:
        timed_sjf_retention(target,args.products/args.case/"user.elf",events,e,first[0],terminal[0],result)
        return result
    if e["src"] != 0:
        gap(False, "A was not running at peer admission; incumbent retention unavailable")
        result["initial_dispatch"] = next((x for x in events if x["dst"] == 0),None)
        result["idle_irq_records"] = select("idle-irq")
        result["initial_store_count"] = select("idle-irq")[0]["count"] if select("idle-irq") else None
        if args.case == CASES[1]:
            result["queued_exit_order"] = [x["src"] for x in events if x["kind"] == 2 and x["op"] == 1]
        return result
    if not gap(e["kind"] == 1 and e["op"] & 1023 == 30,
               "active A release was outside the sampled lower-EL IRQ path"):
        return result
    expected_dst = 1 if args.case == CASES[0] else 0
    require(e["dst"] == expected_dst, "release choice")
    saves = select("irq-save")
    if not gap(len(saves) == 1, "missing or ambiguous interrupted-frame publication"):
        return result
    saved = saves[0]
    gap(saved["count"] > 0, "no prior useful progress")
    if args.case == CASES[0]:
        back = [x for x in events if x["src"] == 1 and x["dst"] == 0 and x["seq"] > e["seq"]]
        if not gap(len(back) == 1, "no unique B-to-A restore"):
            return result
        back = back[0]
    else:
        back = e
    gap(trace_matches(target,e,e["dst_pc"]) and trace_matches(target,back,back["dst_pc"]),
        "external exception/ERET chain unavailable")
    if corrected and args.case == CASES[0]:
        transfers = [r for r in rows if r["seq"] > saved["seq"] and r["kind"] in ("restore","eret")]
        if not gap([r["kind"] for r in transfers] == ["restore","eret","restore","eret"],
                   "incomplete pair of accepted B/A returns"):
            return result
        require(saved["pc"] == 0x40201290 and 0x40ab1000 <= saved["pa"] <= 0x40ab5000-832
                and saved["pa"] % 16 == 0, "IRQ frame save location")
        for i, dest_event in enumerate((e,back)):
            restore, eret = transfers[2*i:2*i+2]
            n = i+1
            require(restore["pc"] == 0x402013e8 and restore["accepted"] == n and
                    restore["restore_execs"] >= n and restore["erets"] == i and
                    0x40ab1000 <= restore["pa"] <= 0x40ab5000-832 and restore["pa"] % 16 == 0 and
                    eret["pc"] == 0x40201480 and eret["erets"] == eret["accepted"] == n and
                    eret["restore_execs"] == restore["restore_execs"], "accepted LDP/ERET ordering")
            operand = restore.get("frame")
            if not gap(operand is not None, "accepted operand unavailable for selection " + str(n)):
                return result
            require(len(operand) == 35 and operand[32:] ==
                    [dest_event["dst_pstate"],dest_event["dst_pc"],dest_event["dst_sp"]],
                    "accepted operand vs selected context")
        app = elf_file((args.products/args.case/"user.elf").read_bytes())
        reduction, length = symbol(app,"batch_reduce")
        if not gap(reduction <= e["src_pc"] < reduction+length and e["src_root"] == back["dst_root"] and
                   saved.get("value") is not None and
                   trace_matches(target,e,e["dst_pc"]) and trace_matches(target,back,back["dst_pc"]),
                   "productive A interruption, private sample or external B/A returns unavailable"):
            return result
    before = saved.get("frame")
    if not gap(before is not None, "saved integer operand unavailable"):
        return result
    require(len(before) == 35 and before[32:35] == [e["src_pstate"],e["src_pc"],e["src_sp"]],
            "interrupt frame vs UART")
    if corrected and args.case == CASES[0]:
        require(saved["value"] == (before[21] & 0xffffffff)-1 and
                saved["sum"] == 522240*saved["value"], "productive W21 vs sampled completed work")
    restores = [r for r in select("restore",saved["seq"]) if r.get("frame") is not None and
                r["frame"][32:35] == [back["dst_pstate"],back["dst_pc"],back["dst_sp"]]]
    if not gap(len(restores) == 1, "no unique independently sampled A restore operand"):
        return result
    restored = restores[0]
    after = restored["frame"]
    require(len(after) == 35 and before[:31] == after[:31], "saved vs restored GPR mismatch")
    erets = select("eret",restored["seq"])
    if not gap(bool(erets) and erets[0]["pc"] == 0x40201480, "restore ERET unavailable"):
        return result
    post = select("post-progress",erets[0]["seq"])
    if gap(bool(post), "first post-return publication unavailable"):
        observation = post[0]
        if corrected:
            require(len(post) == 1 and observation["pc"] == 0x400a40 and observation["pa"] == progress_pa,
                    "first post-ERET A publication identity")
            if observation["value"] is not None:
                require(observation["sum"] == 522240*observation["value"], "first post-ERET result")
        gap(observation["count"] == saved["count"]+1 and observation.get("value") is not None and
            saved.get("value") is not None and observation["value"] == saved["value"]+1,
            "numeric first post-return advance unavailable")
    result["integer_operands"] = dict(saved_pa=saved["pa"],restored_pa=restored["pa"],
        saved_gpr_sha256=hashlib.sha256(struct.pack("<31Q",*before[:31])).hexdigest(),
        restored_gpr_sha256=hashlib.sha256(struct.pack("<31Q",*after[:31])).hexdigest(),
        all_31_match=True, register_file_sample=False)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="command",required=True)
    hist = subs.add_parser("historical")
    hist.add_argument("products",type=Path)
    for name in ("acquire","observed"):
        sub = subs.add_parser(name)
        sub.add_argument("products",type=Path)
        sub.add_argument("out",type=Path)
        if name == "acquire":
            sub.add_argument("plugin",type=Path)
        else:
            sub.add_argument("case",choices=CASES)
            sub.add_argument("session_sha256")
        sub.add_argument("plugin_sha256")
    args = parser.parse_args()
    try:
        if args.command == "historical":
            return original(args)
        if args.command == "acquire":
            return acquisition(args)
        result = observed(args)
        result["status"] = "observed_with_gaps" if result["gaps"] else "observed"
        print(json.dumps(result,indent=2))
        return 2 if result["gaps"] else 0
    except (OSError,ValueError,KeyError,IndexError,TypeError,struct.error) as exc:
        print(json.dumps(dict(status="incomplete_or_mismatch",detail=str(exc)),indent=2))
        return 1


if __name__ == "__main__":
    sys.exit(main())
