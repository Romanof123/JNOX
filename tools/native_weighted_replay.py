"""Reconstruct two private native process histories from the raw physical snapshot."""
from dataclasses import dataclass
import hashlib
import struct

from native_el0_replay import need, symbol
from native_weighted_math import product

WORDS = struct.Struct("<16Q")
WIDTHS = {1: 5, 2: 13, 3: 13, 4: 6, 5: 9, 6: 10, 7: 2, 8: 8, 9: 7, 10: 7, 11: 13, 12: 5}
BILLION = 10**9


@dataclass(frozen=True)
class Row:
    tag: int
    tick: int
    pid: int
    data: tuple


def journal(mem, kernel):
    address, size = symbol(kernel, "native_vruntime_journal")
    need(size == 513 * WORDS.size, "weighted journal size")
    words = list(WORDS.iter_unpack(mem.read(address, size)))
    head = words[0]
    (
        version,
        state,
        arm,
        freq,
        q,
        epoch,
        count,
        lost,
        fail,
        flag,
        wakes,
        closures,
        a,
        b,
        result,
        reserved,
    ) = head
    need(
        version == 1
        and state in (1, 2, 3, 4)
        and arm in (1, 2)
        and count <= 512
        and not reserved,
        "journal header",
    )
    need(state != 4 and not fail and not lost, "guest failure or overflow")
    if state != 3:
        raise FileNotFoundError("no sealed terminal state")
    need(
        freq == 100 * q and q > 0 and epoch > 0 and flag >= epoch and a and b,
        "clock, service and terminal flag",
    )
    need(not any(any(r) for r in words[count + 1 :]), "uncommitted rows")
    rows = []
    for raw in words[1 : count + 1]:
        tag, tick, pid = raw[:3]
        need(tag in WIDTHS, "unknown event")
        end = 3 + WIDTHS[tag]
        need(not any(raw[end:]), "nonzero reserved event words")
        rows.append(Row(tag, tick, pid, raw[3:end]))
    return head, rows


def ns(seconds, nanos):
    need(seconds < 1 << 63 and nanos < BILLION, "normalized nonnegative timespec")
    return seconds * BILLION + nanos


def absolute_tick(epoch, freq, nanoseconds):
    return epoch + (nanoseconds * freq + BILLION - 1) // BILLION


def stable_work(pid, seq, chunks, samples, value, aux):
    need(
        seq == 2 * chunks and chunks < 1 << 28 and samples == 256 * chunks,
        "stable chunk sequence",
    )
    if samples:
        need((samples, value, aux) == product(pid, samples), "task ring checksums")
    else:
        need(value == aux == 0, "zero-work checksums")


def check_images(kernel_bytes, user_bytes, mem, head, rows):
    from native_el0_replay import elf_file, permissions, vread, PAGE, PA_MASK
    from native_weighted_math import private_page, root_info

    k, u = elf_file(kernel_bytes), elf_file(user_bytes)
    entry = u["e_entry"]
    result, size = symbol(u, "native_weighted_result")
    need(
        result == head[14] and size == 224 and entry == symbol(u, "_start")[0],
        "linked entry and private result symbol",
    )
    maps = [row for row in rows if row.tag == 1]
    need([r.pid for r in maps] == [0, 1], "two distinct map records")
    ttbrs = [r.data[0] for r in maps]
    need([root_info(t)[0] for t in ttbrs] == [1, 2], "configured process ASIDs")
    roots = [root_info(t)[1] for t in ttbrs]
    segments = [
        s for s in u.iter_segments() if s["p_type"] == "PT_LOAD" and s["p_memsz"]
    ]
    need([s["p_flags"] for s in segments] == [5, 4, 6], "user RX/RO/RW loads")
    note = u.get_section_by_name(".note.jnox.abi")
    note_bytes = struct.pack("<III8sI", 5, 4, 1, b"JNOX", 1)
    ro = segments[1]
    need(
        note is not None
        and note["sh_type"] == "SHT_NOTE"
        and note["sh_flags"] == 2
        and note.data() == note_bytes
        and 0 <= note["sh_addr"] - ro["p_vaddr"] <= ro["p_filesz"] - len(note_bytes)
        and note["sh_addr"] - ro["p_vaddr"] == note["sh_offset"] - ro["p_offset"],
        "allocated RO native ABI v1",
    )
    flag_leaves = [mem.leaf(root, 0x01200000) for root in roots]
    need(
        all(leaf is not None and leaf & (1 << 11) for leaf in flag_leaves)
        and flag_leaves[0] == flag_leaves[1],
        "same read-only nG completion flag",
    )
    private_vas = set()
    for s in segments:
        begin, end = s["p_vaddr"], s["p_vaddr"] + s["p_memsz"]
        need(
            0x400000 <= begin < end <= 0x800000 and begin % PAGE == 0,
            "linked user segment range",
        )
        for va in range(begin, end, PAGE):
            if s["p_flags"] == 6:
                private_vas.add(va)
            for root in roots:
                permissions(
                    mem,
                    root,
                    va,
                    {5: "user rx", 4: "user r", 6: "user rw"}[s["p_flags"]],
                )
        if s["p_flags"] != 6:
            for root in roots:
                need(
                    vread(mem, root, begin, s["p_filesz"]) == s.data(),
                    "immutable mapped segment contents",
                )
    private_vas.update(range(0x01001000, 0x01011000, PAGE))
    seen = set()
    for va in sorted(private_vas):
        pages = private_page(mem, ttbrs, va)
        need(not seen.intersection(pages), "aliased data/stack pages")
        seen.update(pages)
    start = symbol(k, "_start")[0]
    limit = symbol(k, "__stack_top")[0]
    split = symbol(k, "__native_rw_start")[0]
    need(
        start == 0x40200000 and start == mem.base and limit - start == len(mem.raw),
        "captured RAM extent",
    )
    products = []
    for row, root in zip(maps, roots):
        for va in range(start, limit, PAGE):
            permissions(mem, root, va, "kernel rx" if va < split else "kernel rw")
        for va in (0x8000000, 0x8010000, 0x9000000):
            permissions(mem, root, va, "kernel rw", device=True)
        for va in (0x01000000, 0x01011000):
            permissions(mem, root, va, "guard")
        permissions(mem, root, 0x01200000, "user r")
        need(
            vread(mem, root, 0x01200000, 8) == struct.pack("<Q", 1),
            "final completion flag",
        )
        _, result_pa, stack_pa, map_entry, sp = row.data
        need(
            entry == map_entry and 0x01001000 < sp < 0x01011000 and sp % 16 == 0,
            "initial owner entry/stack",
        )
        need(
            result_pa == (mem.leaf(root, result) & PA_MASK)
            and stack_pa == (mem.leaf(root, sp) & PA_MASK),
            "private physical page addresses",
        )
        stack = struct.unpack("<5Q", vread(mem, root, sp, 40))
        need(stack[0] == 3 and stack[4] == 0, "process bootstrap arguments")
        for ptr, arg in zip(
            stack[1:4],
            (
                b"weighted",
                b"AB"[row.pid : row.pid + 1],
                b"share" if head[2] == 1 else b"wake",
            ),
        ):
            need(
                sp < ptr < 0x01011000
                and vread(mem, root, ptr, len(arg) + 1) == arg + b"\0",
                "process role/arm argument",
            )
        raw = struct.unpack("<28Q", vread(mem, root, result, 224))
        need(
            raw[:3] == (1, row.pid + 1, head[2]) and raw[-1] == 41 + row.pid,
            "sealed private task result",
        )
        stable_work(row.pid, *raw[3:8])
        need(raw[4] > 0, "positive useful work")
        need(
            raw[8] == (3 if head[2] == 2 and row.pid == 1 else 0), "private wake count"
        )
        if raw[8]:
            need(raw[4:8] == (6, 1536, 7680, 58560), "wake receiver totals")
        else:
            need(not any(raw[9:27]), "unused private wake times")
        products.append(raw)
    return k, u, ttbrs, products


def history(head, rows, mem, k, u, ttbrs, products, trace):
    from native_el0_replay import PAGE, SVC, vread
    from native_weighted_math import Service, root_info
    from native_weighted_trace import transitions

    arm, freq, q, epoch = head[2:6]
    service = Service(q)
    data = iter(rows[2:])
    observed = iter(transitions(trace))
    index = 0
    last_tick = epoch

    def take(tag, pid=None, tick=None):
        item = next(data, None)
        need(
            item
            and item.tag == tag
            and (pid is None or item.pid == pid)
            and (tick is None or item.tick == tick),
            f"lease {index}: event {tag}",
        )
        return item

    def external(kind, pc, vector=0):
        seen = next(observed, None)
        need(
            seen and (seen.kind, seen.pc, seen.vector) == (kind, pc, vector),
            "external " + kind,
        )

    def virtual(whole, rem, weight):
        need(weight in (1, 2) and rem < weight, "canonical virtual time")
        return whole * 2 + rem * (2 // weight)

    roots = [root_info(t)[1] for t in ttbrs]
    vbar, vsize = symbol(k, "native_vruntime_vectors")
    need(vsize == 2048 and not vbar % 2048, "vector table extent")
    entry = symbol(u, "_start")[0]
    rx = next(
        s for s in u.iter_segments() if s["p_type"] == "PT_LOAD" and s["p_flags"] == 5
    )
    pcmin, pcmax = rx["p_vaddr"], rx["p_vaddr"] + rx["p_filesz"]
    frame_ptr = {}
    saved = {}
    private_trace = []
    prefix = []
    preempted = set()
    resumed = set()
    products_seen = [0, 0]
    blocks = []
    pending = None
    wakes = closures = 0
    flag = False
    exits = set()
    calls = []
    grants = []
    wake_data = []
    while service.ready:
        owner = service.owner()
        weight = owner + 1
        index += 1
        sel = take(2, owner)
        (
            lease,
            space,
            rpc,
            rsp,
            rpsr,
            x0,
            x19,
            tls,
            deadline,
            sa,
            sb,
            whole,
            rem,
        ) = sel.data
        need(
            lease == index
            and space == ttbrs[owner]
            and [sa, sb] == service.service
            and virtual(whole, rem, weight) == service.vr[owner]
            and sel.tick >= last_tick,
            "dispatch",
        )
        need(
            deadline == (pending["target"] if pending else (1 << 64) - 1),
            "outstanding deadline",
        )
        restore = take(11, owner, sel.tick)
        f = restore.data
        need(
            f[:2] == (lease, 1)
            and f[2] % 16 == 0
            and mem.base <= f[2] <= mem.base + len(mem.raw) - 832,
            "restore frame",
        )
        extra = take(12, owner, sel.tick).data
        need(extra == (lease, 1, x0, x19, tls), "restore register bank")
        if owner in saved:
            prior = saved[owner]
            need(
                (rpc, rsp, rpsr) == prior[:3] and f[2:] == prior[3],
                "saved/restored architectural frame",
            )
            expected_x0 = 0 if prior[4] == "svc" else prior[5][0]
            need((x0, x19, tls) == (expected_x0, *prior[5][1:]), "retained x0/x19/TLS")
        else:
            need(
                (rpc, rsp, rpsr, x0, x19, tls)
                == (
                    entry,
                    rows[owner].data[4],
                    0x340,
                    0,
                    0,
                    0x01011000 - 96 - 16 * owner,
                )
                and not any(f[3:]),
                "initialized process frame",
            )
        if owner in preempted:
            resumed.add(owner)
        if owner in frame_ptr:
            need(f[2] == frame_ptr[owner], "private frame identity")
        frame_ptr[owner] = f[2]
        if wakes and blocks[-1].get("wake") and "select" not in blocks[-1]:
            record = blocks[-1]
            if owner == 1:
                record["select"] = sel.tick
                if (
                    sel.tick - record["wake"] > 5 * q // 4 + 2 * (q // 4)
                    or record["intervening"] > 1
                ):
                    service.misses.append("wake-to-B dispatch")
            else:
                record["intervening"] += 1
        external("eret", rpc)
        stop = take(3, owner)
        seq, vector, start, compare, ctl, iar, pc, sp, pstate, x8, dt, a, b = stop.data
        if owner == 1 and blocks and blocks[-1].get("select") == sel.tick:
            blocks[-1]["return"] = start
        kind = "irq" if vector == 0x480 else "svc"
        need(
            seq == lease
            and vector in (0x480, 0x400)
            and sel.tick <= start < stop.tick
            and dt == stop.tick - start
            and compare == min(start + q, deadline),
            "timer programming/elapsed",
        )
        need(
            pcmin <= pc < pcmax
            and pc % 4 == 0
            and 0x01001000 <= sp <= 0x01011000
            and sp % 16 == 0
            and pstate & 0x3FF == 0x340,
            "lower EL PC/SP/SPSR",
        )
        service.interval(owner, start, stop.tick)
        need(service.service == [a, b], "charged physical service")
        if kind == "irq":
            need(
                iar == 30 and ctl & 7 == 5 and stop.tick >= compare,
                "actual physical timer IRQ",
            )
            preempted.add(owner)
        else:
            need(
                iar == (1 << 64) - 1
                and int.from_bytes(vread(mem, roots[owner], pc - 4, 4), "little")
                == SVC,
                "native SVC origin",
            )
        external(kind, pc, vbar + vector)
        trap = take(11, owner, stop.tick).data
        need(trap[:3] == (lease, 0, f[2]), "trap frame identity")
        if kind == "svc":
            need(trap[-1] == 0x56004A01, "architectural syscall syndrome")
        trap_extra = take(12, owner, stop.tick).data
        need(trap_extra[:2] == (lease, 0), "trap register bank")
        saved[owner] = (pc, sp, pstate, trap[2:], kind, trap_extra[2:])
        work = take(9, owner, stop.tick).data
        need(
            work[0] == lease
            and work[1] >= products_seen[owner]
            and work[1] <= products[owner][3],
            "published progress sequence",
        )
        products_seen[owner] = work[1]
        if work[1] % 2 == 0:
            stable_work(owner, *work[1:6])
        grants.append(
            {
                "lease": lease,
                "pid": owner,
                "start": start,
                "stop": stop.tick,
                "compare": compare,
                "cause": kind,
                "service": list(service.service),
                "vr2": list(service.vr),
                "work": list(work[1:]),
            }
        )
        last_tick = stop.tick
        if kind == "svc":
            if x8 == 7:
                need(
                    arm == 2 and owner == 1 and pending is None and len(blocks) < 3,
                    "sleep request",
                )
                block = take(5, 1)
                due, accept, svc_b, fw, fr, fd, bw, br, bd = block.data
                service.remove(1)
                need(
                    block.tick == accept
                    and stop.tick <= accept < due
                    and svc_b == b
                    and virtual(fw, fr, fd) == service.floor
                    and bd == 2
                    and virtual(bw, br, bd) == service.vr[1],
                    "blocked state",
                )
                pending = {
                    "target": due,
                    "block": accept,
                    "a_start": a,
                    "b_start": b,
                    "full": 0,
                    "intervening": 0,
                }
                blocks.append(pending)
                last_tick = accept
            elif x8 == 1:
                ex = take(8, owner, stop.tick)
                final = products[owner]
                need(
                    flag
                    and ex.data == (41 + owner, *final[3:9], final[27])
                    and work[1:] == final[3:9],
                    "sealed exit tuple",
                )
                if arm == 2 and owner == 1:
                    for r in range(3):
                        wd = take(10, 1, stop.tick)
                        need(
                            wd.data == (r, *final[9 + 6 * r : 15 + 6 * r]),
                            "sealed wake times",
                        )
                        wake_data.append(wd.data[1:])
                service.remove(owner)
                exits.add(owner)
            else:
                need(x8 == 6 and arm == 2 and owner == 1, "unexpected native call")
            sc = take(4, owner, stop.tick)
            ident, arg0, arg1, arg2, arg3, res = sc.data
            need(
                ident == x8
                and arg0 == trap_extra[2]
                and (arg1, arg2, arg3) == trap[3:6]
                and res == 0,
                "SVC/architectural arguments",
            )
            if x8 == 1:
                need(arg0 == 41 + owner, "exit call status")
            else:
                need(
                    arg0 == 1 and (x8 != 7 or (arg1, arg3) == (1, 0)),
                    "clock ID/absolute flag",
                )
                ptr = arg1 if x8 == 6 else arg2
                for addr in (ptr, ptr + 15):
                    desc = mem.leaf(roots[owner], addr)
                    need(
                        desc is not None and desc >> 6 & 3 == 1,
                        "private writable timespec",
                    )
                calls.append((x8, stop.tick))
        elif pending:
            need(owner == 0 and b == pending["b_start"], "B charged while asleep")
            if compare == start + q:
                pending["full"] += 1
            if stop.tick >= pending["target"]:
                wk = take(6, 1)
                due, irq, sv, fw, fr, fd, bw, br, pw, pr = wk.data
                need(
                    due == pending["target"]
                    and wk.tick >= irq == stop.tick
                    and sv == b
                    and virtual(fw, fr, fd) == service.floor
                    and virtual(bw, br, 2) == service.vr[1],
                    "wake input",
                )
                service.wake_b()
                need(virtual(pw, pr, 2) == service.vr[1], "wake floor placement")
                pending.update(wake=wk.tick, irq=irq, a_solo=a - pending["a_start"])
                if not (3 <= pending["full"] <= 4 and pending["a_solo"] >= 3 * q):
                    service.misses.append("sleep A-only service/full closures")
                wakes += 1
                last_tick = wk.tick
                pending = None
        elif arm == 1 and not flag:
            closures += 1
            prefix.append(service.prefix(closures))
        if not flag and (closures == 36 or wakes == 3):
            mark = take(7, owner)
            need(
                mark.tick >= last_tick
                and mark.tick == head[9]
                and mark.data == (closures, wakes),
                "horizon event",
            )
            last_tick = mark.tick
            flag = True
        private_trace.append((owner, sel.tick, start, stop.tick))
    need(
        next(data, None) is None and next(observed, None) is None and exits == {0, 1},
        "post-exit ownership",
    )
    frame_hashes = []
    for pid, ptr in sorted(frame_ptr.items()):
        raw_frame = mem.read(ptr, 832)
        g = struct.unpack("<104Q", raw_frame)
        pc, sp, psr, frame, kind, extra = saved[pid]
        need(
            kind == "svc"
            and (g[0], g[8], g[19], g[31], g[32], g[33], g[34], g[35], g[102], g[103])
            == (0, 1, extra[1], 0x56004A01, psr, pc, sp, 4, extra[2], frame[-2]),
            "sealed private exception frame",
        )
        need(
            tuple(g[n] for n in (1, 2, 3, 20, 21, 22, 29, 30)) == frame[1:9],
            "sealed private GPRs",
        )
        frame_hashes.append(hashlib.sha256(raw_frame).hexdigest())
    need(
        abs(frame_ptr[0] - frame_ptr[1]) >= 832
        and head[10:14] == (wakes, closures, *service.service),
        "terminal frames/accounting",
    )
    need(({0, 1} if arm == 1 else {0}) <= resumed, "IRQ preemption and return")
    if arm == 2:
        need(
            [call for call, _ in calls] == [6, 7, 6] * 3
            and len(wake_data) == len(blocks) == 3,
            "three absolute sleeps",
        )
        for i, (times, record) in enumerate(zip(wake_data, blocks)):
            before, target, after = (ns(*times[n : n + 2]) for n in (0, 2, 4))
            need(
                target == before + 40_000_000
                and after >= target
                and absolute_tick(epoch, freq, target) == record["target"],
                "absolute sleep conversion",
            )
            for pos, sample in ((0, before), (2, after)):
                trapped = calls[3 * i + pos][1]
                following = next(
                    (start for _, t, start, _ in private_trace if t >= trapped), None
                )
                need(following is not None, "missing return after clock read")
                low = max(trapped, absolute_tick(epoch, freq, sample))
                high = min(following, absolute_tick(epoch, freq, sample + 1) - 1)
                need(low <= high, "clock sample outside producing syscall interval")
                if pos == 2:
                    record.update(sample_low=low, sample_high=high)
            d, irq, w, r, low, high = (
                record[key]
                for key in (
                    "target",
                    "irq",
                    "wake",
                    "return",
                    "sample_low",
                    "sample_high",
                )
            )
            h, g = 5 * q // 4, q // 4
            if not (
                d <= irq <= w <= r <= low
                and irq - d <= g
                and w - irq <= g
                and r - w <= h + 2 * g
                and high - r <= g
                and high - d <= h + 5 * g
            ):
                service.misses.append(f"round {i+1}: D/I/W/R/S chronology or bound")
            record.update(
                before_ns=before,
                target_ns=target,
                resumed_ns=after,
                target_to_sample_low=low - d,
                target_to_sample_high=high - d,
            )
    return {
        "frame_sha256": frame_hashes,
        "frame_address": frame_ptr,
        "closures": grants,
        "prefix": prefix,
        "wake_rounds": blocks,
        "gaps": service.gaps,
        "timing_misses": service.misses,
    }


def replay(folder, digest):
    import json
    import re
    from pathlib import PurePosixPath
    from native_el0_replay import Snapshot, elf_file
    from native_weighted_capture import ARMS, CLOCKS, SUCCESS, qemu_command
    from ordinary_amc_capture import sha
    from ordinary_amc_replay import bounded, obj

    need(sha(folder / "session.json") == digest, "trusted session digest")
    session = json.loads(bounded(folder / "session.json", 65536), object_pairs_hook=obj)
    required = {
        "launch.json",
        "kernel.elf",
        "user.elf",
        "probe.c",
        "stderr",
        "ram.bin",
        "uart.bin",
        "exceptions.log",
    }
    run = json.loads(bounded(folder / "launch.json", 65536), object_pairs_hook=obj)
    instrumented = "timer_observation" in run
    if instrumented:
        from native_weighted_timer import PIN, split_log, timing_report

        need(
            run["timer_observation"] == 1
            and run["arm"] == "share"
            and run["clock"] == "default"
            and run["qemu_sha256"] == PIN,
            "timer cohort identity",
        )
        required.add("host-timing.json")
    hashes = session["hashes"]
    need(set(hashes) <= required, "unexpected capture file")
    for name, val in hashes.items():
        need(sha(folder / name) == val, "capture hash: " + name)
    if session["marker"] == "FAIL":
        raise ValueError("retained guest failure")
    if set(hashes) != required or session["marker"] != SUCCESS:
        raise FileNotFoundError(
            "incomplete terminal capture: " + str(session["marker"])
        )

    need(
        run["version"] == 1
        and session["pid"] > 0
        and run["arm"] in ARMS
        and run["clock"] in CLOCKS
        and re.fullmatch("[0-9a-f]{40}", run["revision"])
        and re.fullmatch("[0-9a-f]{64}", run["qemu_sha256"])
        and bool(run["qemu_version"].strip()),
        "launch identity",
    )
    argv = run["argv"]
    original = PurePosixPath(argv[-1].replace(chr(92), "/")).parent
    actual = [part.replace(chr(92), "/") for part in argv]
    need(
        actual == qemu_command(actual[0], original, run["clock"], instrumented),
        "QEMU configuration and artifact paths",
    )
    for src, name in (
        ("kernel", "kernel.elf"),
        ("user", "user.elf"),
        ("probe", "probe.c"),
    ):
        need(run[src + "_sha256"] == hashes[name], "launch asset hash")
    need(
        bounded(folder / "uart.bin", 1 << 20)
        == b"JNOX M1 BOOT\nJNOX M1 READY\nNATIVE VRUNTIME END\n",
        "complete UART",
    )
    kbytes = bounded(folder / "kernel.elf", 64 << 20)
    ubytes = bounded(folder / "user.elf", 16 << 20)
    ram = bounded(folder / "ram.bin", 16 << 20)
    need(len(ram) == run["ram_bytes"], "RAM extent")
    mem = Snapshot(run["ram_base"], ram)
    kernel = elf_file(kbytes)
    embedded = []
    for s in kernel.iter_segments():
        if s["p_type"] == "PT_LOAD":
            area = s.data()
            pos = area.find(ubytes)
            if pos >= 0:
                need(area.find(ubytes, pos + 1) < 0, "ambiguous packaged user ELF")
                embedded.append(s["p_paddr"] + pos)
    need(
        len(embedded) == 1 and mem.read(embedded[0], len(ubytes)) == ubytes,
        "packaged and loaded ELF identity",
    )
    head, rows = journal(mem, kernel)
    need(ARMS[head[2] - 1] == run["arm"], "guest/launch arm")
    k, u, ttbrs, products = check_images(kbytes, ubytes, mem, head, rows)
    log = bounded(folder / "exceptions.log", 64 << 20)
    if instrumented:
        log, events = split_log(log)
    report = history(
        head,
        rows,
        mem,
        k,
        u,
        ttbrs,
        products,
        log,
    )
    if instrumented:
        host = json.loads(
            bounded(folder / "host-timing.json", 4 << 20), object_pairs_hook=obj
        )
        report["timer_observation"] = timing_report(events, report["closures"], host)
    report.update(
        arm=run["arm"],
        clock=run["clock"],
        source=run["revision"],
        session_sha256=digest,
        kernel_sha256=hashes["kernel.elf"],
        user_sha256=hashes["user.elf"],
        frequency=head[3],
        quantum=head[4],
        ttbrs=ttbrs,
        private_results=products,
        private_result_sha256=[
            hashlib.sha256(struct.pack("<28Q", *v)).hexdigest() for v in products
        ],
    )
    return report


if __name__ == "__main__":
    import argparse
    import json
    from pathlib import Path

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("trusted_session_sha256")
    args = parser.parse_args()
    try:
        report = replay(args.folder, args.trusted_session_sha256)
        print(json.dumps(report, indent=2))
        raise SystemExit(1 if report["timing_misses"] else 0)
    except (FileNotFoundError, ImportError) as exc:
        parser.exit(2, f"UNOBSERVED {exc}\n")
    except (ValueError, OSError, KeyError, IndexError, TypeError) as exc:
        parser.exit(1, f"INVALID {exc}\n")
