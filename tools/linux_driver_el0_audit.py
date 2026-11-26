"""Inspect system instructions in the direct branch closures of AArch64 ET_REL functions."""
import re
from collections import Counter

from linux_driver_relocations import ObjectGraph


def classify(op, operands):
    op = op.lower()
    operands = operands.lower().split("//", 1)[0].strip()
    if op in ("mrs", "msr", "mrs_s", "msr_s"):
        fields = [field.strip() for field in operands.split(",")]
        reg = fields[-1] if op.startswith("mrs") else fields[0]
        if reg in {"tpidr_el1", "tpidr_el2", "sp_el0"}:
            return "privileged_" + reg
        if reg in {"daif", "daifset", "daifclr"}:
            return "interrupt_mask"
        if reg in {"cntvct_el0", "cntpct_el0", "cntfrq_el0", "ctr_el0", "dczid_el0"}:
            return "platform_controlled_user_register"
        if reg in {"tpidr_el0", "tpidrro_el0", "nzcv", "fpcr", "fpsr"}:
            return "user_register_access"
        if re.search(r"_el[123]$", reg) or reg in {"spsel", "pan", "uao", "currentel", "spsr_irq"}:
            return "privileged_system_register"
        return "unknown_system_register"
    if op in {"tlbi", "at", "cfp", "dvp", "cpp"}:
        return "privileged_maintenance"
    if op in {"dc", "ic"}:
        operand = operands.split(",")[0].strip()
        if (op, operand) in {("dc", "zva"), ("dc", "cvau"), ("dc", "cvac"),
                              ("dc", "civac"), ("dc", "cvap"), ("ic", "ivau")}:
            return "platform_controlled_cache_maintenance"
        return "privileged_or_unknown_maintenance"
    if op in {"sys", "sysl", "msrr", "mrrs"}:
        return "unknown_system_operation"
    if op in {"eret", "eretaa", "eretab", "drps", "hvc", "smc", "dcps1", "dcps2", "dcps3"}:
        return "privileged_exception_instruction"
    if op in {"svc", "brk", "hlt", "udf"}:
        return "exception_instruction"
    if op in {"wfe", "wfi", "wfet", "wfit"}:
        return "platform_controlled_wait"
    if op.startswith(".") or op in {"unknown", "undefined", "hint"}:
        return "unknown_instruction_or_data"
    return None


def inspect_closures(path, objdump, relocations):
    graph = ObjectGraph(path)
    graph.disassemble(objdump, path)
    results = {}
    for entry, closure in relocations["entries"].items():
        sites = []
        undecoded = []
        for ident in closure["direct_nodes"]:
            sec, pos, size = ident.split(":")
            sec, pos, size = int(sec), int(pos, 16), int(size, 16)
            node = relocations["nodes"][ident]
            decoded = 0
            for address, op, operands in graph.instructions[sec]:
                if not pos <= address < pos + size:
                    continue
                decoded += 1
                category = classify(op, operands)
                if category:
                    sites.append(dict(function=node["name"], section=node["section"],
                                      offset=address - pos, instruction=op,
                                      operands=operands, category=category))
            if decoded * 4 != size:
                undecoded.append(dict(function=node["name"], section=node["section"],
                                      symbol_bytes=size, disassembled_rows=decoded))
        results[entry] = {
            "incomplete_disassembly": undecoded,
            "direct_function_count": len(closure["direct_nodes"]),
            "counts": dict(sorted(Counter(site["category"] for site in sites).items())),
            "sites": sites,
        }
    return results
