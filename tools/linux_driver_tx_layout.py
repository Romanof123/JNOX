"""Generate a declaration-only view of the pinned Linux source for the native TX adapter."""
import re


def private_layout(source):
    prelude = source[:source.index("static int napi_weight")]
    if any(line.startswith("#include") for line in source[len(prelude):].splitlines()):
        raise ValueError("unexpected later upstream includes; review the layout view")
    parts = [prelude]
    for declaration in ("struct virtnet_sq_stats", "struct virtnet_interrupt_coalesce",
                        "struct send_queue", "struct virtnet_info", "enum virtnet_xmit_type"):
        matches = re.findall(r"^" + declaration + r" \{\n.*?^\};$", source, re.M | re.S)
        if len(matches) != 1:
            raise ValueError(f"expected one upstream declaration: {declaration}")
        parts.append(matches[0])
    return "#pragma once\n" + "\n\n".join(parts) + "\n"
