"""Validate a native hierarchy topology and encode its raw Cargo representation."""
import json


def unique_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate topology key {key!r}")
        result[key] = value
    return result


def parse_topology(source, slot_count):
    """Return the canonical object in ID order and the direct-build settings."""
    topology = json.loads(source.decode("utf-8"), object_pairs_hook=unique_keys)
    keys = ("period_ticks", "parents", "children", "leaves")
    if not isinstance(topology, dict) or set(topology) != set(keys):
        raise ValueError("expected exactly period_ticks, parents, children and leaves")
    period, parents, children, leaves = (topology[key] for key in keys)
    if type(period) is not int or not 0 < period < 2**64:
        raise ValueError("period_ticks must be a positive u64")
    for name, rows, maximum in (("parents", parents, 2), ("children", children, 3),
                                ("leaves", leaves, 3)):
        if not isinstance(rows, list) or not 1 <= len(rows) <= maximum:
            raise ValueError(f"{name} must be an array of 1..{maximum} entries")
    if len(leaves) != slot_count:
        raise ValueError("supply exactly one leaf per application slot")

    def quota(value):
        if type(value) is not int or not 1 <= value <= period:
            raise ValueError("quota must be an integer in 1..period_ticks")
        return str(value)

    def pairs(rows, owner_count):
        values = []
        for row in rows:
            if not isinstance(row, list) or len(row) != 2:
                raise ValueError("expected [owner ID, quota]")
            owner, budget = row
            if type(owner) is not int or not 0 <= owner < owner_count:
                raise ValueError("owner ID must reference the preceding level")
            values.append(f"{owner}:{quota(budget)}")
        return ",".join(values)

    raw = ";".join((str(period), ",".join(map(quota, parents)), pairs(children, len(parents)),
                    pairs(leaves, len(children))))
    return {key: topology[key] for key in keys}, raw
