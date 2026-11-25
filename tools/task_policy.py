"""Compile a J-NOX v1 task policy with embedded static AArch64 executables."""
import argparse
from dataclasses import dataclass, field
from hashlib import sha256
import json
from pathlib import Path
import re
import shlex
from struct import Struct

HEADER = Struct("<4sIII")
APP = Struct("<32sIIII")
TASK = Struct("<32sIIQIIII")
ELF = Struct("<16sHHIQQQIHHHHHH")
PH = Struct("<IIQQQQQQ")
SH = Struct("<IIQQQQIIQQ")
SYM = Struct("<IBBHQQ")
U32 = 0xffffffff


def require(condition, message):
    if not condition:
        raise ValueError(message)


def within(data, offset, size):
    require(offset + size <= len(data), "truncated ELF range")
    return memoryview(data)[offset:offset + size]


def executable_segments(blob, offset, count, entry):
    # Match kernel/native/elf.rs::plan for the native user address window.
    require(offset >= ELF.size and 1 <= count <= 16, "invalid program header table")
    previous_end = 0x400000
    loads = 0
    stack = False
    executable = []
    for kind, flags, start, va, _, size, memsz, align in PH.iter_unpack(within(blob, offset, count * PH.size)):
        if kind == 0:
            continue
        if kind == 0x6474e551:
            require(not stack and flags & ~6 == 0 and size == memsz == 0, "invalid stack header")
            stack = True
            continue
        require(kind == 1, "unsupported ELF program header (static PT_LOAD required)")
        loads += 1
        require(loads <= 8 and flags in (4, 5, 6), "unsupported load count or permissions")
        require(align >= 4096 and align & (align - 1) == 0 and
                va % 4096 == start % 4096 == 0 and va % align == start % align,
                "unsupported segment alignment")
        require(0 < memsz and size <= memsz and previous_end <= va and va + memsz <= 0x800000,
                "invalid native load address or size")
        within(blob, start, size)
        previous_end = (va + memsz + 4095) & ~4095
        if flags == 5:
            executable.append((va, start, size))
    require(entry % 4 == 0 and any(va <= entry and entry + 4 <= va + size
                                   for va, _, size in executable), "invalid ELF entry point")
    return executable


def function_symbols(blob, wanted):
    fields = ELF.unpack(within(blob, 0, ELF.size))
    ident, kind, arch, version, entry, phoff, shoff, flags, ehsize, phsize, phnum, shsize, shnum, _ = fields
    require(ident[:9] == b"\x7fELF\x02\x01\x01\0\0" and
            (kind, arch, version, flags) == (2, 183, 1, 0), "expected static little-endian AArch64 ET_EXEC")
    require((ehsize, phsize, shsize) == (ELF.size, PH.size, SH.size) and shnum and shoff >= ELF.size,
            "invalid ELF header sizes or missing sections")
    segments = executable_segments(blob, phoff, phnum, entry)
    sections = list(SH.iter_unpack(within(blob, shoff, shnum * SH.size)))
    symbols = {name.encode("ascii"): [] for name in wanted}
    for _, typ, _, _, offset, size, link, _, _, entsize in sections:
        if typ != 2:  # SHT_SYMTAB
            continue
        require(entsize == SYM.size and size % entsize == 0 and link < shnum, "invalid symbol table")
        strings = sections[link]
        require(strings[1] == 3, "invalid symbol string table")
        names = bytes(within(blob, strings[4], strings[5]))
        for idx, info, _, section, addr, length in SYM.iter_unpack(within(blob, offset, size)):
            if info & 15 != 2 or section == 0:  # Defined STT_FUNC
                continue
            end = names.find(b"\0", idx)
            require(end >= idx, "invalid symbol name offset")
            name = names[idx:end]
            if name not in symbols:
                continue
            valid = False
            if section < shnum and addr % 4 == 0:
                _, stype, sflags, base, fileoff, section_size, *_ = sections[section]
                bound = addr + max(4, length)
                valid = (stype == 1 and sflags & 6 == 6 and base <= addr and bound <= base + section_size
                         and any(va <= addr and bound <= va + size and fileoff + addr - base == start + addr - va
                                 for va, start, size in segments))
            symbols[name].append(addr if valid else None)
    return symbols


def identifier(value, bounded=True):
    require(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", value) and (not bounded or len(value) <= 31),
            f"invalid identifier {value!r}")
    return value


def integer(value, maximum):
    require(re.fullmatch(r"[0-9]+", value), f"expected a decimal integer, got {value!r}")
    result = int(value)
    require(1 <= result <= maximum, f"value outside 1..{maximum}: {value}")
    return result


@dataclass
class Application:
    line: int
    path: str
    tasks: dict = field(default_factory=dict)


def parse(source, path):
    apps = {}
    pending = []
    version = False
    for line, raw in enumerate(source.splitlines(), 1):
        try:
            words = shlex.split(raw, comments=True, posix=True)
            if not words:
                continue
            if not version:
                require(words == ["version", "1"], "expected version 1")
                version = True
            elif words[0] == "app" and len(words) == 3:
                name = identifier(words[1])
                require(name not in apps, f"duplicate app {name}")
                apps[name] = Application(line, words[2])
            elif words[0] == "task" and len(words) == 8:
                _, app, name, symbol, period, pri, deadline, watchdog = words
                period = integer(period, U32)
                task = dict(name=identifier(name), symbol=identifier(symbol, False), period_ms=period,
                            priority=integer(pri, 255), deadline_ms=integer(deadline, period),
                            watchdog_ms=0 if watchdog == "off" else integer(watchdog, U32))
                pending.append((line, identifier(app), task))
            else:
                raise ValueError("unexpected declaration or argument count")
        except ValueError as exc:
            raise ValueError(f"{path}:{line}: {exc}") from exc
    require(version and apps, f"{path}:{len(source.splitlines()) or 1}: expected version 1 and at least one app")
    for line, app, task in pending:
        require(app in apps, f"{path}:{line}: unknown app {app}")
        name = task["name"]
        require(name not in apps[app].tasks, f"{path}:{line}: duplicate task {app}.{name}")
        apps[app].tasks[name] = (line, task)
    return apps


def compile_policy(path: Path) -> tuple[bytes, dict]:
    path = Path(path)
    source = path.read_bytes()
    try:
        policy_text = source.decode("utf-8")
    except UnicodeDecodeError as exc:
        line = source[:exc.start].count(b"\n") + 1
        raise ValueError(f"{path}:{line}: invalid UTF-8") from exc
    apps = parse(policy_text, path)
    count = sum(len(app.tasks) for app in apps.values())
    offset = HEADER.size + APP.size * len(apps) + TASK.size * count
    require(offset <= U32, f"{path}:1: policy tables exceed wire size")
    app_table, task_table, images = bytearray(), bytearray(), []
    manifest = dict(version=1, source_sha256=sha256(source).hexdigest(), apps=[])
    first = 0
    for app_id, (name, app) in enumerate(apps.items()):
        try:
            require(app.tasks, f"app {name} has no tasks")
            blob = (path.parent / app.path).read_bytes()
            require(offset + len(blob) <= U32, "policy image exceeds wire size")
            symbols = function_symbols(blob, (task["symbol"] for _, task in app.tasks.values()))
        except (ValueError, OSError) as exc:
            raise ValueError(f"{path}:{app.line}: {name}: {exc}") from exc
        app_table.extend(APP.pack(name.encode(), offset, len(blob), first, len(app.tasks)))
        record = dict(id=app_id, name=name, path=app.path, elf_offset=offset, elf_size=len(blob),
                      elf_sha256=sha256(blob).hexdigest(), first_task=first, tasks=[])
        for slot, (line, task) in enumerate(app.tasks.values()):
            symbol = task["symbol"]
            hits = symbols[symbol.encode()]
            require(len(hits) == 1 and hits[0] is not None,
                    f"{path}:{line}: {name}: missing, ambiguous, or non-executable function {symbol}")
            entry = hits[0]
            task_table.extend(TASK.pack(task["name"].encode(), app_id, slot, entry, task["period_ms"],
                                        task["priority"], task["deadline_ms"], task["watchdog_ms"]))
            record["tasks"].append(dict(task, slot=slot, entry=entry))
        manifest["apps"].append(record)
        images.append(blob)
        offset += len(blob)
        first += len(app.tasks)
    binary = b"".join([HEADER.pack(b"JNP1", 1, len(apps), count), app_table, task_table, *images])
    manifest["policy_sha256"] = sha256(binary).hexdigest()
    return binary, manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("policy", type=Path)
    parser.add_argument("--out", type=Path, required=True, help="output directory")
    args = parser.parse_args()
    try:
        binary, manifest = compile_policy(args.policy)
        args.out.mkdir(parents=True, exist_ok=True)
        (args.out / "policy.bin").write_bytes(binary)
        (args.out / "policy-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    except (ValueError, OSError) as exc:
        parser.exit(1, f"{exc}\n")
