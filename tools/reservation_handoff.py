"""Decode reservation opening receipts without attributing synthesized rows to the IRQ."""
import argparse
import json
from pathlib import Path

NA = (1 << 64) - 1


def require(value, reason):
    if not value:
        raise ValueError(reason)


def decode(raw):
    lines = raw.splitlines()
    markers = [(i, l) for i, l in enumerate(lines) if l.startswith(b"RSRV RAW")]
    require(
        len(markers) == 1
        and lines.count(b"RSRV END") == 1
        and lines[-1:] == [b"RSRV END"],
        "truncated or mixed stream",
    )
    start, marker = markers[0]
    require(marker in (b"RSRV RAW 1", b"RSRV RAW 3"), "unsupported schema")
    rows = []
    for line in lines[start + 1 : -1]:
        f = line.split()
        require(
            len(f) == 8 and all(w.isdigit() and len(w) <= 20 for w in f),
            "invalid numeric row",
        )
        row = list(map(int, f))
        require(max(row) <= NA and len(rows) < 1024, "counter or storage overflow")
        rows.append(row)

    def events(tag):
        return [r for r in rows if r[0] == tag]

    def one(tag):
        found = events(tag)
        require(len(found) == 1, f"missing or duplicate {tag}")
        return found[0]

    result = one(13)
    require(rows[-1] == result, "misplaced terminal")
    if marker == b"RSRV RAW 1":
        return {
            "wire_version": 1,
            "opening": "v2 preparation present"
            if events(22)
            else "legacy/unidentified",
            "result": result[4:],
            "ledger_rows": events(5),
            "commit": events(23),
        }
    receipts = events(24)
    require(all(r[2] in (NA, 0) for r in receipts), "unknown tag 24 slot")
    terminal = [r for r in receipts if r[2] == 0]
    anchors = [r for r in receipts if r[2] == NA]
    require(
        len(terminal) == 1 and len(anchors) <= 1, "missing or duplicate receipt role"
    )
    receipt = terminal[0]
    rid, flags, authority, owner = receipt[4:]
    sealed = rid != NA
    require(
        receipt[3] == (0 if sealed else 2) and receipt[1] <= result[1],
        "invalid terminal receipt",
    )
    if not anchors:
        # Before the boot anchor only clock/configuration or Jobs construction can fail.
        tags = [r[0] for r in rows]
        require(
            tags in ([24, 33, 13], [1, 24, 33, 13])
            and receipt[4:] == [NA, 0, 0, NA]
            and result[2:5] == [NA, 2, 3]
            and result[5] in (1, 2)
            and one(33)[1:4] == [receipt[1], 0, 2],
            "missing boot anchor for claimed opening",
        )
        return {
            "wire_version": 3,
            "result": result[4:],
            "stages": 0,
            "boot_anchor": None,
            "descriptor": NA,
            "authority": 0,
            "ledger_provenance": "absent",
            "actual_worker_return": [],
            "descriptive_allocations": [],
            "descriptive_demand": [],
            "live_receipts": {},
        }
    anchor = anchors[0]
    cfg = one(1)
    require(
        cfg[6] in (0, 1) and cfg[7] in (0, 1) and cfg[4] > 0 and cfg[4] % 1000 == 0,
        "unexpected compact configuration",
    )
    boot, epoch, zero1, zero2 = anchor[4:]
    require(
        cfg[2:4] == [NA, 2]
        and cfg[5] == cfg[4] // 20
        and anchor[3] == 2
        and zero1 == zero2 == 0
        and boot <= epoch - cfg[5] <= cfg[1] <= anchor[1] <= receipt[1]
        and rows.index(cfg) < rows.index(anchor) < rows.index(receipt),
        "invalid boot anchor",
    )
    setup = events(2)
    require(
        len(setup) <= 1 and (not (sealed or flags or result[4] == 0) or setup),
        "missing or duplicate clock setup",
    )
    if setup:
        row = setup[0]
        require(
            row[2:] == [NA, 2, epoch, 34 if cfg[7] == 0 else 10, 0, 0]
            and anchor[1] <= row[1] <= receipt[1]
            and rows.index(anchor) < rows.index(row) < rows.index(receipt),
            "setup conflicts with boot anchor",
        )
    for tag in range(25, 38):
        for r in events(tag):
            require(r[2] == 0, f"unexpected slot for compact tag {tag}")
            if tag not in (31, 32, 34, 35):
                require(
                    r[3] == (0 if tag in (27, 28, 29, 30) else 2),
                    f"unexpected owner for tag {tag}",
                )
    require(one(33)[1] == receipt[1], "storage row differs from terminal receipt")
    require(flags in (0, 1, 3, 7, 15, 31), "impossible receipt stages")
    for mask, tag in ((1, 25), (1, 37), (2, 26), (4, 27), (8, 28), (16, 29), (16, 30)):
        require(
            len(events(tag)) == int(bool(flags & mask)),
            f"stage {tag} validity mismatch",
        )
    published = bool(flags & 4)
    require(
        authority == (3 if published else 0) and owner == (0 if published else NA),
        "bank/owner readback mismatch",
    )
    require(not (flags & 2) or sealed, "precommit without witness")
    require(not published or sealed, "publication without its witness")
    if sealed:
        require(rid == 1 + 2 * cfg[6] + cfg[7], "wrong descriptor for policy/manifest")
    for tag in (31, 32, 34, 35):
        require(
            [r[3] for r in events(tag)] == ([0, 1] if sealed else []),
            "missing or duplicate sealed client",
        )
    if sealed:
        require(
            all(r[1] == receipt[1] for tag in (31, 32, 34, 35) for r in events(tag)),
            "wrong descriptor decode timestamp",
        )
        w = cfg[7] == 0
        cbs = cfg[6] == 1
        # B has no work at slot zero of W. Its initial aligned start or
        # dormant CBS allocation is obtained from the manifest's backend.
        expected_a = [2, 8, 1, 8] if w else [1, 2, 0, 2]
        require(events(31)[0][4:] == expected_a, "incorrect prepared A debit")
        expected_b = [1, 2, 1, 2] if not w else ([0, 0, 0, 0] if cbs else [0, 1, 0, 1])
        require(events(31)[1][4:] == expected_b, "incorrect prepared B allocation")
        require(
            events(32)[0][4:] == ([1, 0, 1, 2] if w else [4, 3, 1, 1]),
            "incorrect A release/demand",
        )
        require(
            events(32)[1][4:] == ([0, 0, 0, 0] if w else [4, 4, 1, 0]),
            "incorrect B release/demand",
        )
    require(
        not any(r[2] == 0 for r in events(5) + events(6) + events(7)),
        "slot zero has unclassified live decision rows",
    )
    if flags & 1:
        irq = one(25)
        end = one(37)
        require(
            end[4:] == [irq[1], end[1], irq[6], irq[4]] and irq[1] <= end[1],
            "source sample interval invalid",
        )
        require(
            irq[4] & 1023 == 30 and irq[5] & 7 == 5 and irq[6] == irq[7] == epoch,
            "timer source or opening readback invalid",
        )
    if flags & 2:
        pre = one(26)
        require(
            pre[4:] == [rid, 0, epoch, 0] and one(37)[1] <= pre[1],
            "precommit receipt invalid",
        )
    if published:
        commit = one(27)
        require(
            commit[4:] == [3, 1, 1, 0] and one(26)[1] <= commit[1],
            "invalid commit order/identity",
        )
    if flags & 8:
        arm = one(28)
        require(
            arm[4] == arm[6] == one(26)[6] + cfg[5] - cfg[4] // 1000
            and arm[5] & 3 == 1
            and commit[1] <= arm[1],
            "bad cutoff readback",
        )
    returns = [r for r in events(12) if r[2:4] == [0, 0]]
    if flags & 16:
        plan = one(29)
        sequence = one(30)
        f = events(34)[0]
        require(
            plan[4:] == [f[4], f[5], events(35)[0][4], f[7]],
            "selected frame differs from sealed identity",
        )
        require(
            sequence[4] == 1
            and sequence[5:] == [epoch, epoch + cfg[5], rid]
            and arm[1] <= plan[1] == sequence[1],
            "wrong return plan",
        )
        if returns:
            require(
                len(returns) == 1
                and returns[0][4:6] == plan[4:6]
                and returns[0][6] == 1
                and returns[0][1] >= plan[1],
                "actual return mismatch",
            )
    else:
        require(
            not returns and not events(10) and not any(r[2] > 0 for r in events(5)),
            "service without a selection",
        )
    if result[4] == 0:
        require(
            flags == 31 and len(returns) == 1, "successful horizon lacks worker return"
        )
        opening = [r for r in events(10) if r[2:4] == [0, 0]]
        require(
            len(opening) == 1
            and opening[0][4] == returns[0][1]
            and opening[0][4] <= opening[0][5] <= one(26)[6] + cfg[4] // 1000,
            "missing/late first Rust marker",
        )
    return {
        "wire_version": 3,
        "result": result[4:],
        "stages": flags,
        "boot_anchor": anchor,
        "ledger_provenance": "committed descriptor"
        if published
        else "prepared only"
        if sealed
        else "absent",
        "descriptor": rid,
        "authority": authority,
        "actual_worker_return": returns,
        "descriptive_allocations": events(31),
        "descriptive_demand": events(32),
        "live_receipts": {t: events(t) for t in (25, 37, 26, 27, 28, 29, 30)},
    }


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("uart", type=Path)
    args = p.parse_args()
    try:
        print(json.dumps(decode(args.uart.read_bytes()), indent=2))
    except (ValueError, OSError) as exc:
        print(json.dumps({"outcome": "invalid", "reason": str(exc)}))
        raise SystemExit(1)
