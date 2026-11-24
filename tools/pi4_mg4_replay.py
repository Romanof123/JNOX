"""Offline MG4 slot, installation and local journal interpretation."""
import io
import struct

EDGES = {1: ("opening", 1, 0), 2: ("reverse", 0, 1)}
PHASES = ("Empty", "Pending", "Consumed")
TASKS = ("M", "P0", "P1")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def payload_fields(edge, payload):
    route, epoch, task, lease, retired, ptr, version, origin = payload
    require(route == edge and epoch == edge - 1 and lease > 0, "edge/epoch/lease")
    if edge == 1:
        require(task == 0 and retired > 0 and retired != lease and
                (ptr, version, origin) == (0, 0, 0), "opening tuple")
    else:
        require(task in (0, 2) and retired == 0 and ptr > 0 and ptr % 16 == 0 and
                version == 2 and origin == (2 if task == 0 else 1), "reverse frame tuple")


def decode_terminal(data):
    """Decode exactly the frozen little-endian 256-byte object."""
    require(len(data) == 256, "terminal size")
    words = struct.unpack("<32Q", data)
    require(words[:4] == (0x3130524554464F48, 1, 256, 0) and words[-1] == 0,
            "terminal header/reserved")
    slots = {}
    for edge, (name, _, _) in EDGES.items():
        start = 4 + 9 * (edge - 1)
        phase, *payload = words[start:start + 9]
        require(phase < len(PHASES), "unknown slot phase")
        if phase:
            payload_fields(edge, payload)
        else:
            require(not any(payload), "nonzero empty slot")
        slots[name] = dict(phase=PHASES[phase], payload=payload)
    installations = {}
    for i, task in enumerate(TASKS):
        ptr, version, origin = words[22 + 3*i:25 + 3*i]
        if (ptr, version, origin) != (0, 0, 0):
            allowed = {(1, 1)}
            if i != 1:
                allowed.add((2, 2 if i == 0 else 1))
            require(ptr > 0 and ptr % 16 == 0 and (version, origin) in allowed,
                    "invalid installation " + task)
        installations[task] = [ptr, version, origin]
    if slots["opening"]["phase"] != "Empty":
        require(all(record[1] for record in installations.values()), "opening before installation")
    reverse = slots["reverse"]
    if reverse["phase"] != "Empty":
        p = reverse["payload"]
        require(slots["opening"]["phase"] == "Consumed" and
                installations["M"][1] == installations["P1"][1] == 2 and
                installations[TASKS[p[2]]] == p[5:], "reverse installation or predecessor")
    return dict(slots=slots, installations=installations)


def handoffs(sections, case):
    """Validate a complete pair for each edge against the inherited scored journal."""
    def one(cpu, kind, predicate=lambda d: True):
        matches = [(n, d) for n, (k, d) in enumerate(sections[cpu]) if
                   k == kind and predicate(d)]
        require(len(matches) == 1, f"missing or duplicate CPU{cpu} event {kind}")
        return matches[0]

    count = sum(k in (16, 17) for rows in sections.values() for k, _ in rows)
    require(count == 4, "handoff row count")
    report = {}
    for edge, (name, src, dst) in EDGES.items():
        pub, p = one(src, 16, lambda d: d[0] == edge)
        con, q = one(dst, 17, lambda d: d[0] == edge)
        payload_fields(edge, p)
        require(p == q, "publication and consumption differ")
        _, ep, task, lease, retired, ptr, _, _ = p
        send, _ = one(src, 7, lambda d: (d[0], d[2]) == (dst, ep))
        trap, irq = one(dst, 4, lambda d: d[6] != 65535)
        require(pub < send and trap < con and irq[:2] == (ep, 0 if edge == 1 else 3),
                "sender or receiver chronology")
        if edge == 1:
            finish, _ = one(src, 3, lambda d: d[:3] == (0, 2, retired) and d[7] == 0)
            started, _ = one(dst, 11, lambda d: d[:3] == (0, 0, lease))
            stopped, _ = one(dst, 3, lambda d: d[:3] == (0, 0, lease) and d[7] == 1)
            require(finish < pub and started < trap < con < stopped, "opening ownership")
        else:
            require(task == (2 if case else 0), "reverse selection")
            plan, _ = one(src, 6, lambda d: d[0] == 1 and d[4] == task)
            claims = [(n,d) for n,(k,d) in enumerate(sections[src]) if k == 2 and d[0] == 1 and plan < n < pub]
            require([d[1] for _,d in claims] == [1, task],
                    "reverse publication before placement")
            _, claim = one(src, 2, lambda d: d[:4] == (1, task, lease, ptr))
            dispatch, frame = one(dst, 11, lambda d: d[:4] == claim[:4])
            require(con < dispatch and frame[4:7] == (claim[4],claim[6],claim[5]),
                    "consumed descriptor differs from dispatch")
            if task == 0:
                _, saved = one(src, 4, lambda d: d[:2] == (0, 0) and d[6] != 65535)
                require(ptr == saved[2], "reverse frame differs from M IRQ")
            else:
                _, stack = one(src, 1, lambda d: d[0] == task)
                from pi4_migration_replay import FRAME
                require(ptr + FRAME == stack[2], "replacement frame differs from fresh stack")
            require(any(k == 5 and d[0:3] == (task, lease, lease)
                        for k,d in sections[dst][dispatch+1:]), "missing remote lease work")
        report[name] = dict(published=True, consumed=True, payload=list(p),
                            terminal_phase="unobserved")
    return report

def corroborate_terminal(decoded, sections, case):
    """Compare the RAM projection with available journal rows, including early stops."""
    from pi4_migration_replay import FRAME

    observed = {name: {} for name, _, _ in EDGES.values()}
    expected = {}
    stack_high = {}
    for cpu, rows in sections.items():
        for kind, words in rows:
            d = tuple(words)
            if kind == 1:
                stack_high[d[0]] = d[2]
            if kind in (16, 17):
                require(d[0] in EDGES, "unknown directed slot")
                name, sender, receiver = EDGES[d[0]]
                expected_cpu = sender if kind == 16 else receiver
                require(cpu == expected_cpu and kind not in observed[name], "slot row origin/count")
                payload_fields(d[0], d)
                if d[0] == 2:
                    require(d[2] == (2 if case else 0), "reverse task/selector")
                slot = decoded["slots"][name]
                require(PHASES.index(slot["phase"]) >= kind-15 and slot["payload"] == list(d),
                        "terminal phase or tuple contradicts journal")
                observed[name][kind] = True
            if kind == 2:
                epoch, task, _, ptr = d[:4]
                require(task in range(3) and epoch in (0, 1), "unknown installation claim")
                version = 2 if epoch == 1 and task != 1 else 1
                put = [ptr, version, 2 if task == 0 and epoch else 1]
                previous = expected.get((task, version), put)
                require(previous == put, "conflicting installation claims")
                expected[task, version] = put
    all_rows = [(cpu,k,tuple(d)) for cpu, rows in sections.items() for k,d in rows]
    for edge, (name, src, dst) in EDGES.items():
        slot = decoded["slots"][name]
        if slot["phase"] == "Empty":
            continue
        _, ep, task, lease, retired, ptr, _, _ = slot["payload"]
        if edge == 2:
            require(task == (2 if case else 0), "terminal reverse task/selector")
        for cpu,k,d in all_rows:
            if k in (2,11) and d[:2] == (ep, task):
                require(d[2] == lease and (edge == 1 or d[3] == ptr),
                        "terminal target differs from claim/dispatch")
                if k == 11:
                    require(cpu == dst, "terminal target dispatched on different CPU")
            if edge == 1 and k == 3 and d[:2] == (0,2):
                require(cpu == src and d[2] == retired and d[7] == 0,
                        "terminal opening differs from retirement")
    installs = {}
    for task, name in enumerate(TASKS):
        actual = decoded["installations"][name]
        ptr, ver, origin = actual
        versions = [v for t,v in expected if t == task]
        require(not versions or ver >= max(versions), "installed record predates claim")
        claimed = expected.get((task, ver))
        if claimed:
            require(actual == claimed, "installed tuple differs from claim")
        if origin == 1 and task in stack_high:
            require(ptr + FRAME == stack_high[task], "fresh installation outside initial frame")
        saved = [d for k,d in sections.get(0, []) if k == 4 and tuple(d[:2]) == (0,0)
                 and d[6] != 65535]
        if origin == 2 and saved:
            require(len(saved) == 1 and ptr == saved[0][2], "saved installation differs from trap")
        installs[name] = dict(claim_observed=bool(claimed), frame_observed=bool(
            (origin == 1 and task in stack_high) or (origin == 2 and saved)))
    return dict(slots={name: dict(phase=decoded["slots"][name]["phase"],
                                  publication_observed=16 in events, consumption_observed=17 in events)
                       for name,events in observed.items()}, installations=installs)


def linked_restore(elf, raw):
    """Recognize the straight-line stack restore using linked AArch64 instruction words."""
    from elftools.elf.elffile import ELFFile

    table = ELFFile(io.BytesIO(elf)).get_section_by_name(".symtab")
    hits = table.get_symbol_by_name("sched_start_frame") or []
    require(len(hits) == 1, "missing or ambiguous linked restore")
    start, size = hits[0]["st_value"], hits[0]["st_size"]
    offset = start - 0x80000
    require(0 <= offset and 0 < size <= 1024 and size % 4 == 0 and offset + size <= len(raw),
            "invalid restore range")
    words = struct.unpack_from(f"<{size//4}I", raw, offset)
    def pair(base, register, offset, scale):
        return base | ((offset//scale) << 15) | ((register+1) << 10) | (31 << 5) | register

    # MOV SP,X0; restore ELR/SPSR, FP control, SP_EL0 and the two task TLS registers.
    prefix = [0x9100001f, pair(0xa9400000,9,248,8), 0xd5184029, 0xd518400a,
              pair(0xa9400000,9,264,8), 0xd51b4409, 0xd51b442a,
              pair(0xa9400000,9,280,8), 0xd518410a,
              pair(0xa9400000,9,296,8), 0xd51bd049, 0xd51bd06a]
    prefix.extend(pair(0xad400000,r,320+16*r,16) for r in range(0,32,2))
    suffix = [pair(0xa9400000,r,8*r,8) for r in range(0,30,2)]
    suffix.extend([0xf9407bfe, 0x910d03ff, 0xd5033f5f, 0xd69f03e0])
    require(words[:len(prefix)] == tuple(prefix) and words[-len(suffix):] == tuple(suffix),
            "linked frame restore/ERET differs")
    # The Pi 4 return-tick sample sits between SIMD and integer restoration.
    stamp = words[len(prefix):-len(suffix)]
    require(len(stamp) == 7, "unrecognized return-tick stamp size")
    address_pair = ((stamp[2] & 0x9f00001f == 0x9000000a and
                     stamp[3] & 0xffc003ff == 0x9100014a) or
                    (stamp[2] == 0xd503201f and stamp[3] & 0x9f00001f == 0x1000000a))
    require(address_pair and stamp[0] == 0xd53800a9 and stamp[1] == 0x92400129
            and stamp[4:] == (0xd5033fdf, 0xd53be02b, 0xf829794b),
            "unrecognized intervening restore instructions")
    return dict(address=start, size=size)
