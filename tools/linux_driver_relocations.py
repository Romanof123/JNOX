"""Inspect AArch64 ET_REL call sites, address references and data-linked callbacks."""
from collections import defaultdict
import re
import struct

from import_linux_driver import run


# ELF64 little-endian records. Relocation targets are identified by symbol index,
# section index and addend; local symbol names alone are not unique.
SHDR = struct.Struct("<IIQQQQIIQQ")
SYM = struct.Struct("<IBBHQQ")
RELA = struct.Struct("<QQq")
BRANCH_RELOCS = {279, 280, 282, 283}  # TSTBR14, CONDBR19, JUMP26, CALL26


class ObjectGraph:
    def __init__(self, path):
        self.data = path.read_bytes()
        if (self.data[:6] != b"\x7fELF\x02\x01" or
                struct.unpack_from("<HH", self.data, 16) != (1, 183)):
            raise ValueError("inventory requires a little-endian AArch64 ELF64 relocatable object")
        start = struct.unpack_from("<Q", self.data, 40)[0]
        stride, count, names = struct.unpack_from("<HHH", self.data, 58)
        if stride != SHDR.size or not count or names >= count:
            raise ValueError("unsupported ELF section layout")
        self.sections = [SHDR.unpack_from(self.data, start + stride * i) for i in range(count)]
        self.names = [self.string(names, s[0]) for s in self.sections]
        self.nodes = {}
        self.owners = defaultdict(list)
        self.function_names = defaultdict(list)
        self.relocations = defaultdict(list)
        self.instructions = defaultdict(list)
        self.symbol_tables = {}
        for sec, row in enumerate(self.sections):
            if row[1] != 2:  # SHT_SYMTAB
                continue
            syms = []
            for off in range(row[4], row[4] + row[5], row[9]):
                name, info, _, idx, pos, size = SYM.unpack_from(self.data, off)
                name = self.string(row[6], name)
                kind = info & 15
                syms.append((name, kind, idx, pos, size))
                if kind not in (1, 2) or not (0 < idx < count) or not size:
                    continue
                key = (idx, pos, size)
                node = self.nodes.setdefault(key, {"name": name, "section": self.names[idx],
                                                   "kind": "function" if kind == 2 else "data"})
                if kind == 2:
                    self.function_names[name].append(key)
                if key not in self.owners[idx]:
                    self.owners[idx].append(key)
                if node["name"] != name:
                    node.setdefault("aliases", []).append(name)
            self.symbol_tables[sec] = syms
        for row in self.sections:
            if row[1] != 4:  # SHT_RELA
                continue
            syms = self.symbol_tables[row[6]]
            for off in range(row[4], row[4] + row[5], row[9]):
                where, info, addend = RELA.unpack_from(self.data, off)
                self.relocations[row[7]].append((where, info & 0xffffffff, syms[info >> 32], addend))

    def string(self, sec, off):
        start = self.sections[sec][4] + off
        return self.data[start:self.data.index(0, start)].decode("utf-8")

    def target(self, idx, pos):
        for key in self.owners[idx]:
            if key[1] <= pos < key[1] + key[2]:
                return key
        # An anonymous constant/table may have no sized object symbol. Linkers
        # retain the input section; report this granularity in the node identity.
        key = (idx, 0, self.sections[idx][5])
        self.nodes.setdefault(key, {"name": self.names[idx], "section": self.names[idx],
                                    "kind": "section"})
        return key

    def disassemble(self, objdump, path):
        dump = run([objdump, "-d", "--no-show-raw-insn", path.name], path.parent)
        indexes = {name: i for i, name in enumerate(self.names)}
        sec = None
        for line in dump.splitlines():
            header = re.fullmatch(r"Disassembly of section (.+):", line)
            if header:
                sec = indexes[header[1]]
                continue
            ins = re.fullmatch(r"\s*([0-9a-fA-F]+):\s+(\S+)\s*(.*?)\s*", line)
            if sec is not None and ins:
                self.instructions[sec].append((int(ins[1], 16), ins[2], ins[3]))

    def edges(self, key):
        idx, start, size = key
        links, imports, indirect = [], [], []
        relocated = set()
        function = self.nodes[key]["kind"] == "function"
        for off, rtype, sym, addend in self.relocations[idx]:
            if not start <= off < start + size:
                continue
            name, kind, target_idx, pos, target_size = sym
            branch = function and rtype in BRANCH_RELOCS
            site = {"offset": off - start, "relocation": rtype, "addend": addend}
            if branch:
                relocated.add(off)
            if target_idx == 0:
                if name:
                    imports.append({**site, "symbol": name, "branch": branch})
                continue
            if target_idx >= len(self.sections):
                continue  # Absolute symbols have no input section to retain.
            # For named object/function relocations, the symbol identifies the
            # owner even when the addend points beyond a field or at an end marker.
            if kind in (1, 2) and target_size:
                dest = (target_idx, pos, target_size)
            else:
                dest = self.target(target_idx, pos + addend)
            if dest == key:
                continue
            site["target_offset"] = pos + addend - dest[1]
            links.append((dest, "branch" if branch else "address", site))
        if function:
            for off, op, operands in self.instructions[idx]:
                if not start <= off < start + size:
                    continue
                if op in {"br", "blr", "braa", "brab", "braaz", "brabz",
                          "blraa", "blrab", "blraaz", "blrabz"}:
                    indirect.append({"offset": off - start, "instruction": op, "operands": operands})
                # Same-section local branches can be fully resolved by the assembler.
                elif off not in relocated and (op in {"b", "bl", "cbz", "cbnz", "tbz", "tbnz"}
                                                or op.startswith("b.")):
                    match = re.search(r"(?:^|,\s*)([0-9a-fA-F]+)\s+<", operands)
                    if match:
                        pos = int(match[1], 16)
                        if not start <= pos < start + size:
                            dest = self.target(idx, pos)
                            links.append((dest, "branch", {"offset": off - start, "instruction": op}))
        return links, imports, indirect


def inventory(path, objdump, entries):
    graph = ObjectGraph(path)
    graph.disassemble(objdump, path)
    reports = {}
    node_reports = {}
    table_entries = defaultdict(list)
    # Find actual stored 64-bit pointers to exact function starts in allocated,
    # non-executable data. PC-relative jump-table offsets and metadata that
    # reference an interior instruction do not constitute callback pointers.
    for sec, rels in graph.relocations.items():
        flags = graph.sections[sec][2]
        if not flags & 2 or flags & 4:
            continue
        for off, rtype, (name, kind, idx, pos, size), addend in rels:
            if rtype != 257 or not (0 < idx < len(graph.sections)):
                continue
            if not graph.sections[idx][2] & 4:
                continue
            dest = graph.target(idx, pos + addend)
            if graph.nodes[dest]["kind"] == "function" and pos + addend == dest[1]:
                owner = graph.target(sec, off)
                table_entries[dest].append({"table": graph.nodes[owner]["name"],
                                            "section": graph.names[sec], "offset": off - owner[1],
                                            "section_offset": off, "relocation": "R_AARCH64_ABS64"})

    def node_id(key):
        return f"{key[0]}:{key[1]:x}:{key[2]:x}"

    def walk(root, direct_only):
        seen = set()
        queue = [root]
        while queue:
            key = queue.pop()
            if key in seen:
                continue
            seen.add(key)
            links, imports, indirect = graph.edges(key)
            ident = node_id(key)
            node_reports[ident] = {**graph.nodes[key], "offset": key[1], "size": key[2],
                                   "references": [{**site, "kind": kind, "to": node_id(dest)}
                                                  for dest, kind, site in links],
                                   "undefined_relocations": imports, "indirect_sites": indirect}
            for dest, kind, _ in links:
                if not direct_only or (kind == "branch" and graph.nodes[dest]["kind"] == "function"):
                    queue.append(dest)
        return sorted(node_id(key) for key in seen)

    for name in entries:
        roots = graph.function_names[name]
        if len(roots) != 1:
            raise ValueError(f"expected one original function named {name}, got {len(roots)}")
        direct = walk(roots[0], True)
        linked = walk(roots[0], False)
        def imports(ids):
            return sorted({item["symbol"] for ident in ids for item in node_reports[ident]["undefined_relocations"]})
        tables = []
        for ident in linked:
            node = node_reports[ident]
            if node["kind"] == "function":
                continue
            for ref in node["references"]:
                target = node_reports[ref["to"]]
                if (target["kind"] == "function" and ref.get("relocation") == 257
                        and ref.get("target_offset") == 0):
                    tables.append({"table": ident, "offset": ref["offset"], "callback": target["name"]})
        reports[name] = {
            "root": node_id(roots[0]), "direct_nodes": direct,
            "entry_pointer_locations": table_entries[roots[0]],
            "direct_functions": sorted({node_reports[i]["name"] for i in direct}),
            "undefined_from_direct_functions": imports(direct),
            "transitive_relocation_nodes": linked, "transitive_relocation_imports": imports(linked),
            "data_callback_relocations": tables,
            "indirect_sites": [{"function": node_reports[i]["name"], **site} for i in direct
                               for site in node_reports[i]["indirect_sites"]],
        }
    return {"object": path.name, "entries": reports, "nodes": node_reports,
            "scope": "Static ARM64 branches and input relocations. Address references describe link "
                     "dependencies. Register branches can target callbacks, tail calls or jump tables; "
                     "runtime traces establish their destinations and actual execution."}
