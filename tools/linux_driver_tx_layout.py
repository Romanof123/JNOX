"""Generate a declaration-only view of the pinned Linux source for the native TX adapter."""
import re


def upstream_prelude(source):
    prelude = source[:source.index("static int napi_weight")]
    if any(line.startswith("#include") for line in source[len(prelude):].splitlines()):
        raise ValueError("unexpected later upstream includes; review the layout view")
    return prelude


def private_layout(source, *, receive=False):
    parts = [upstream_prelude(source)]
    declarations = ["struct virtnet_sq_stats", "struct virtnet_interrupt_coalesce", "struct send_queue"]
    if receive:
        averages = re.findall(r"^DECLARE_EWMA\(pkt_len, [^\n]+\)$", source, re.M)
        if len(averages) != 1:
            raise ValueError("expected the pinned receive queue average declaration")
        parts.append(averages[0])
        declarations.extend(("struct virtnet_rq_dma", "struct virtnet_rq_stats", "struct receive_queue"))
    declarations.extend(("struct virtnet_info", "enum virtnet_xmit_type"))
    for declaration in declarations:
        matches = re.findall(r"^" + declaration + r" \{\n.*?^\};$", source, re.M | re.S)
        if len(matches) != 1:
            raise ValueError(f"expected one upstream declaration: {declaration}")
        parts.append(matches[0])
    return "#pragma once\n" + "\n\n".join(parts) + "\n"
