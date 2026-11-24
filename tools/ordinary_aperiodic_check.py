"""Replay one frozen POL-007 ordinary server capture using only the standard library."""
import argparse
from collections import defaultdict, deque
import hashlib
import json
from pathlib import Path
import re
import sys
import struct

APPS = {f"{p}-{k}" for p in "ab" for k in ("ps", "ds", "ss1", "ss2")}
WIDTH = {"EVENT": 10, "STEP": 4, "RETURN": 3, "MASK": 2, "END": 1}
LIMIT = {"EVENT": 192, "STEP": 4096, "RETURN": 64, "MASK": 1, "END": 1}


class Invalid(ValueError):
    """Capture cannot substantiate the requested interpretation."""


def require(condition, message):
    if not condition:
        raise Invalid(message)


def rows_from_uart(text):
    rows = defaultdict(list)
    last = -1
    for number, line in enumerate(text.splitlines(), 1):
        if line in ("JNOX M1 BOOT", "JNOX M1 READY", "") and last < 0:
            continue
        parts = line.split()
        require(len(parts) >= 2 and parts[0] == "AP" and parts[1] in WIDTH,
                f"line {number}: unexpected UART data")
        tag = parts[1]
        section = list(WIDTH).index(tag)
        require(section >= last and len(parts) == WIDTH[tag] + 2 and
                len(rows[tag]) < LIMIT[tag] and
                all(re.fullmatch(r"[0-9a-fA-F]{16}", w) for w in parts[2:]),
                f"line {number}: malformed, out-of-order or excess {tag}")
        last = section
        rows[tag].append([int(w, 16) for w in parts[2:]])
    require(rows["END"] and rows["MASK"] and rows["EVENT"] and rows["RETURN"],
            "missing setup, return, mask or end")
    return rows


def exception_sequence(text):
    result = []
    lines = iter(text.splitlines())
    for line in lines:
        if not line.strip():
            continue
        if line.startswith("Exception return"):
            match = re.fullmatch(r"Exception return from AArch64 EL1 to AArch64 EL1 PC 0x([0-9a-fA-F]+)", line)
            require(match, "malformed external return")
            result.append(("ERET", int(match[1], 16)))
            continue
        match = re.fullmatch(r"Taking exception (2|5) \[(SVC|IRQ)\]", line)
        require(match and (match[1],match[2]) in (("2","SVC"),("5","IRQ")), "unexpected exception")
        kind = match[2]
        require(next(lines, "") == "...from EL1 to EL1", "wrong exception level")
        syndrome = re.fullmatch(r"\.\.\.with ESR 0x([0-9a-fA-F]+)/0x([0-9a-fA-F]+)", next(lines,""))
        require(syndrome, "missing exception syndrome")
        if kind == "SVC":
            require(int(syndrome[1],16)==0x15, "wrong SVC syndrome")
            kind += str(int(syndrome[2],16) & 0xffff)
        elr = re.fullmatch(r"\.\.\.with ELR 0x([0-9a-fA-F]+)",next(lines,""))
        vector = re.fullmatch(r"\.\.\.to EL1 PC 0x[0-9a-fA-F]+ PSTATE 0x[0-9a-fA-F]+",next(lines,""))
        require(elr and vector, "missing ELR or exception vector")
        result.append((kind,int(elr[1],16)))
    return result


def replay(case, rows, trace=None):
    app = case.removesuffix("-late")
    require(app in APPS, "unknown case")
    late = case.endswith("-late")
    pattern, policy = app.split("-")
    arrivals = ({1: (1, 1), 3: (2, 2), 4: (3, 1)} if pattern == "a"
                else {0: (1, 4), 4: (2, 2)})
    events, returns = deque(rows["EVENT"]), deque(rows["RETURN"])
    steps = deque(rows["STEP"])
    verdict, = rows["END"][0]
    mask = rows["MASK"][0]
    exception = []
    last = 0

    def take(kind, slot):
        nonlocal last
        require(events and events[0][:2] == [kind, slot], f"slot {slot}: expected event {kind}")
        _, _, tick, *fields = events.popleft()
        require(tick >= last, f"slot {slot}: counter reversed")
        last = tick
        return fields

    freq, epoch, q, base, stride, cap, initial = take(0, 0)
    require(freq > 0 and freq % 1000 == 0 and q == freq // 20 and
            epoch > last and epoch + 17*q < 2**64 and stride == 16384 and base % 16 == 0
            and cap == (1 if policy == "ss1" else 2), "invalid setup or credit capacity")

    def stack(owner, frame, pc):
        low = base + owner * stride
        require(0 <= owner <= 3 and low <= frame <= low + stride - 832 and
                frame % 16 == 0 and pc > 0 and pc % 4 == 0, "invalid private frame or PC")

    def restored(owner, frame, pc):
        nonlocal last
        stack(owner, frame, pc)
        require(returns and returns[0][:2] == [frame, pc], "missing or mismatched return")
        _, _, when = returns.popleft()
        require(when >= last, "return precedes preparation")
        last = when
        exception.append(("ERET", pc))

    require(initial == base + stride - 832, "wrong initial idle frame")
    restored(0, initial, returns[0][1])
    fifo = deque()
    submitted_at = {}
    quota, credits = 2, []
    charged = retired = submitted = owner = 0
    issue = None
    max_irq = max_prep = 0
    server, periodic = [], []
    category = None
    reason = None
    work_return = 0

    def handoff(slot, destination, preparation):
        nonlocal owner, max_prep, category, reason
        old, new, frame, pc, nominal, z1, z2 = take(4, slot)
        require((old, new, nominal, z1, z2) == (owner, destination, target, 0, 0),
                f"slot {slot}: invalid owner transition")
        if destination == 0:
            require((frame, pc) == idle_resume, "wrong idle resume frame or PC")
        else:
            require(frame == base + (destination+1)*stride - 832, "wrong fresh task frame")
        if not returns and not events and verdict == 4 and preparation:
            require(last-target > freq//200, "preparation fault lacks counter evidence")
            max_prep = max(max_prep, last-target)
            category, reason = "inhibited", "preparation-over-5ms"
            return False
        restored(new, frame, pc)
        if preparation:
            delta = last - target
            max_prep = max(max_prep, delta)
            require(delta <= freq // 200, f"slot {slot}: dispatch return exceeded 5 ms")
        owner = new
        return True

    def svc(slot):
        actual, frame, pc, *args = take(5, slot)
        require(owner == actual, "wrong SVC owner")
        stack(owner, frame, pc)
        exception.append(("SVC1" if owner == 1 else "SVC2", pc))
        return args

    def progress(slot, count, seed, start, end):
        before = start
        for i in range(count):
            require(steps and steps[0][:2] == [slot, i], f"slot {slot}: missing progress {i}")
            _, _, when, result = steps.popleft()
            seed = (3 * seed + 1) & 0xffffffff
            require(before <= when <= end and result == seed, f"slot {slot}: bad progress {i}")
            before = when
        return seed

    for slot in range(17):
        target = epoch + slot*q
        nominal, actual, frame, pc, missed, done, pending = take(1, slot)
        delta = last - target
        require((nominal, actual, missed, done, pending) ==
                (target, owner, max(0, delta)//q, retired, len(fifo)) and
                submitted == retired + pending, f"slot {slot}: IRQ ledger or ownership")
        stack(owner, frame, pc)
        exception.append(("IRQ", pc))
        max_irq = max(max_irq, delta)
        if delta < 0 or delta > freq//1000 or owner != 0 or issue is not None:
            require(verdict == 1, "IRQ must inhibit before arbitration")
            reason = "early-irq" if delta < 0 else ("late-irq" if delta > freq//1000 else "unfinished-owner")
            category = "inhibited"
            if mask != [0, 0]:
                lo, hi = mask
                require(late and owner == 2 and issue and charged == 1 and retired == 0
                        and missed >= 1 and work_return <= lo < target
                        and hi >= target + q + q//2 and hi <= last,
                        "missing prescribed mid-service mask or missed boundary")
                progress(slot-1, 128, sum(issue), work_return, lo)
                # IRQ recognition after unmask may follow further complete steps.
                i = 128
                seed = sum(issue)
                for _ in range(128):
                    seed = (3*seed+1) & 0xffffffff
                previous = hi
                while steps:
                    s, j, when, value = steps.popleft()
                    seed = (3*seed+1) & 0xffffffff
                    require(s == slot-1 and j == i and i < 256 and
                            previous <= when <= last and value == seed, "invalid post-unmask progress")
                    i += 1
                    previous = when
                category = "prescribed-late-exercised"
            elif owner in (2, 3):
                seed = sum(issue) if issue else 1+(slot-1)//4
                require(len(steps) <= 256, "excess partial progress")
                progress(slot-1, len(steps), seed, work_return, last)
            break
        idle_resume = (frame, pc)
        if slot == 16:
            require(verdict == 0 and not fifo and submitted == (4 if pattern == "a" else 6)
                    and len(periodic) == 4 and not late, "invalid final closure or fault not exercised")
            category = "completed"
            break
        if slot in arrivals:
            ident, units = arrivals[slot]
            if not handoff(slot, 1, True):
                break
            if events and events[0][:2] == [1, slot+1]:
                continue
            producer_return = last
            rid, size, when, _ = svc(slot)
            require((rid, size) == (ident, units) and producer_return <= when <= last, "invalid submission")
            if last-target > freq//200:
                require(verdict == 5, "producer missed preparation cutoff")
                category, reason = "inhibited", "submission-over-5ms"
                break
            submitted += units
            require(take(2, slot) == [ident, units, when, submitted, 0, 0, 0], "invalid submit ack")
            submitted_at[ident] = when
            fifo.extend((ident, i) for i in range(units))
        due_periodic = slot % 4 == 2
        if policy.startswith("ss"):
            quota += sum(n for due, n in credits if due <= slot)
            credits = [(due,n) for due,n in credits if due > slot]
        elif slot % 4 == 0:
            quota = 2
        issue = fifo[0] if fifo and quota and not due_periodic else None
        if issue:
            charged += 1
            quota -= 1
            if policy.startswith("ss"):
                amount = 1 + (credits.pop()[1] if len(credits) == cap else 0)
                credits.append((slot+4, amount))
        if policy == "ps" and len(fifo) == int(bool(issue)):
            quota = 0
        require(take(3, slot) == [quota, len(fifo), charged, retired, issue[0] if issue else 0,
                                  len(credits), int(due_periodic)], f"slot {slot}: decision ledger")
        for due, amount in credits:
            require(take(8, slot) == [due,amount,0,0,0,0,0], "incorrect dated credit")
        selected = 3 if due_periodic else (2 if issue else 0)
        if not handoff(slot, selected, True):
            break
        if selected:
            work_return = last
            if events and events[0][:2] == [1, slot+1]:
                continue
            expected, unit = issue if issue else (slot//4+1, 0)
            ident, checksum, start, finish = svc(slot)
            require(ident == expected and work_return <= start <= finish <= last <= target+q,
                    "wrong completed identity or deadline")
            require(progress(slot, 256, ident+unit, start, finish) == checksum, "bad final checksum")
            if issue:
                fifo.popleft()
                retired += 1
                issue = None
            latency = finish - (target if due_periodic else submitted_at[ident])
            require(take(6, slot) == [selected,ident,checksum,start,finish,latency,retired], "invalid retirement")
            (periodic if due_periodic else server).append([slot,ident,latency])
            handoff(slot, 0, False)
    require(category and not events and not steps and not returns, "unconsumed or missing evidence")
    require(mask == [0, 0] or category == "prescribed-late-exercised", "unsupported mask evidence")
    if trace is not None:
        require(exception_sequence(trace) == exception, "external exception/ERET sequence mismatch")
    return dict(outcome=category, reason=reason, guest_end=verdict, frequency=freq,
                irq_max_ticks=max_irq, preparation_return_max_ticks=max_prep, charged=charged,
                retired=retired, pending=len(fifo), in_flight=issue, periodic=periodic, server=server,
                exception_count=len(exception),
                prescribed_fault=("exercised" if category=="prescribed-late-exercised" else
                                  "unexercised") if late else "not-requested")


def metadata(meta, clock, uart, exceptions, elf):
    require(isinstance(meta, dict) and meta.get("exit") == 0, "missing clean capture metadata")
    args = meta.get("args")
    require(isinstance(args, list) and all(isinstance(s, str) for s in args), "missing QEMU argv")
    count = args.count("-icount")
    require(count == int(clock == "icount") and (not count or
            args[args.index("-icount")+1:args.index("-icount")+2] == ["shift=0,align=off,sleep=off"]),
            "clock disagrees with metadata")
    for key, value in (("-cpu","cortex-a72"),("-accel","tcg"),("-smp","1"),("-m","128M")):
        require(args.count(key) == 1 and args[args.index(key)+1:args.index(key)+2] == [value],
                f"metadata requires {key} {value}")
    machine = [i for i,arg in enumerate(args) if arg in ("-M","-machine")]
    require(len(machine)==1 and args[machine[0]+1:machine[0]+2] == ["virt,gic-version=2"],
            "metadata requires virt/GICv2")
    raw = meta.get("raw")
    require(isinstance(raw, dict), "missing metadata raw hashes")
    for name, data in (("uart",uart), ("int",exceptions)):
        if data is not None:
            require(hashlib.sha256(data).hexdigest()==raw.get(name), f"{name} digest mismatch")
    if elf is not None:
        require(hashlib.sha256(elf).hexdigest()==meta.get("elf"), "ELF digest mismatch")


def executable_ranges(image):
    require(len(image)>=64 and image[:6]==b"\x7fELF\x02\x01" and
            struct.unpack_from("<HH",image,16)==(2,183), "expected executable little-endian AArch64 ELF")
    offset, = struct.unpack_from("<Q",image,32)
    size, count = struct.unpack_from("<HH",image,54)
    require(size>=56 and offset+count*size<=len(image), "truncated ELF program headers")
    ranges = []
    for i in range(count):
        kind, flags, start, addr, _, filesz, memsz, _ = struct.unpack_from("<IIQQQQQQ",image,offset+i*size)
        if kind==1 and flags&1:
            require(filesz<=memsz and start+filesz<=len(image), "invalid ELF code segment")
            ranges.append((addr,addr+filesz))
    require(ranges,"no executable ELF segment")
    return ranges


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=sorted(APPS | {n+"-late" for n in APPS}), required=True)
    parser.add_argument("--clock", choices=["default","icount"], required=True)
    parser.add_argument("--uart", type=Path, required=True)
    parser.add_argument("--exceptions", type=Path, help="QEMU -d int output")
    parser.add_argument("--elf", type=Path)
    parser.add_argument("--meta", type=Path, help="optional retained metadata JSON")
    args = parser.parse_args()
    try:
        uart = args.uart.read_bytes()
        exceptions = args.exceptions.read_bytes() if args.exceptions else None
        elf = args.elf.read_bytes() if args.elf else None
        if args.meta:
            metadata(json.loads(args.meta.read_bytes()),args.clock,uart,exceptions,elf)
        rows = rows_from_uart(uart.decode("ascii"))
        if elf is not None:
            ranges = executable_ranges(elf)
            pcs = [r[1] for r in rows["RETURN"]]
            pcs += [r[6] if r[0]==1 else r[5] for r in rows["EVENT"] if r[0] in (1,5)]
            require(all(any(lo<=pc and pc+4<=hi for lo,hi in ranges) for pc in pcs),
                    "PC outside file-backed ELF code")
        result = replay(args.case, rows, exceptions.decode("ascii") if exceptions is not None else None)
        result.update(case=args.case, clock=args.clock,
                      clock_basis="metadata argv" if args.meta else "caller declaration",
                      elf_sha256=hashlib.sha256(elf).hexdigest() if elf is not None else None,
                      elf_scope=("metadata digest and executable PC ranges" if args.meta else "executable PC ranges")
                                if elf is not None else "not supplied",
                      eret_scope="external sequence matched" if exceptions is not None else "guest samples only")
        print(json.dumps(result, sort_keys=True))
        return 0 if result["outcome"] in ("completed","prescribed-late-exercised") else 1
    except (ValueError, OSError, struct.error) as error:
        print(json.dumps({"outcome":"invalid","error":str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
