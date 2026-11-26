"""Score #401 open-gate expiry coverage from the extracted #403 three-role packet."""

import argparse
from collections import Counter
import csv
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile


REVISION = "a69bc6948c1d57c6df89d395e9307de5e0c6d6dc"
PUBLIC_REVISION = "4ae1c136988b8cd811f472267bfad3b4eb27d2f3"
TREE = "a851e772eff77659b16d9a372de6d353be803757"
PACKET_SUMS = "509b5365288e9d75106657f084ab0204c5c75c4911753da4ac29db42d5ae8635"
REPLAY_SHA256 = "9802c66bf711efa7ce786a2ed30a2b4604e86078914fd87d2c3d3d0384d3abca"
DECODERS = {
    "check_native.py": "78a004fd82bd9828c0ef8649787bd040ea66738a19de39555f61c45a5d1b819d",
    "check_queue.py": "c27d0d68a5e17ee034186fe0a21b05cfc31b6de11e4d1a4d59b64036061c2d63",
    "raw_capture.py": "dc0bda0e0c008c4fc702d1b5b988aac8e1a5a1ec3a7bce9236e792ac4a5e7e2f",
}
HZ = 62_500_000
CLOCKS = ("default", "count0")
POLICIES = ("fifo", "rr")
CASES = ("arrival-before", "arrival-same", "arrival-after")
YES, NO, UNKNOWN = "observed", "not-observed", "unresolved"


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def pinned_bytes(path, digest):
    data = path.read_bytes()
    require(hashlib.sha256(data).hexdigest() == digest, f"SHA-256 mismatch: {path}")
    return data


def packet_ledger(root):
    # The released manifest covers raw RAM/UART, receipts, ELFs, source and metadata.
    sums = pinned_bytes(root / "SHA256SUMS", PACKET_SUMS)
    for line in sums.decode("ascii").splitlines():
        digest, name = line.split("  ", 1)
        pinned_bytes(root / name, digest)
    with (root / "ledger.tsv").open() as f:
        ledger = list(csv.DictReader(f, delimiter="\t"))
    expected = [(c, p, a) for c in CLOCKS for p in POLICIES for a in CASES]
    require([(r["clock"], r["policy"], r["case"]) for r in ledger] == expected, "incomplete first-boot matrix")
    attempts = {r["attempt"] for r in ledger}
    require(attempts == {p.name for p in (root / "runs").iterdir() if p.is_dir()}, "unledgered/missing boot")
    pids, last_finish = set(), 0
    for n, row in enumerate(ledger, 1):
        name = row["attempt"]
        require(name == f"{n:03}-{row['clock']}-{row['policy']}-{row['case']}" and
                row["capture_rc"] == row["build_rc"] == "0" and row["revision"] == REVISION, f"ledger: {name}")
        receipt = json.loads((root / "runs" / name / "cap/session.json").read_bytes())
        require(receipt["pid"] not in pids and
                last_finish < receipt["started_unix_ns"] <= receipt["finished_unix_ns"] and
                receipt["qemu_returncode"] == 0, f"independent completed boot: {name}")
        pids.add(receipt["pid"])
        last_finish = receipt["finished_unix_ns"]
    return ledger


def replay(root, ledger):
    # -I excludes the calling directory and PYTHONPATH. The private import directory
    # contains only the exact source bytes checked here (also excludes packet .pyc files).
    with tempfile.TemporaryDirectory() as temp:
        for name, digest in DECODERS.items():
            (Path(temp) / name).write_bytes(pinned_bytes(root / "verification" / name, digest))
        runner = ("import runpy,sys; script, directory, *args = sys.argv[1:]; "
                  "sys.path.insert(0,directory); sys.argv=[script,*args]; runpy.run_path(script,run_name='__main__')")
        output = subprocess.check_output(
            [sys.executable, "-I", "-B", "-c", runner, str(Path(temp) / "check_native.py"),
             temp, str(root), REVISION], cwd=temp)
    # Hash the fresh stdout, allowing platform newline conversion only.
    require(hashlib.sha256(output.replace(b"\r\n", b"\n")).hexdigest() == REPLAY_SHA256, "frozen decoder output mismatch")
    rows = json.loads(output)
    require([r["run"] for r in rows] == [r["attempt"] for r in ledger] and
            len({r["receipt_sha256"] for r in rows}) == len(ledger), "missing or duplicate decoded attempt")
    return rows


def loan_order(row, uart):
    """Join serial journal positions even when different operations carry the same tick."""
    events = [list(map(int, line.split()[3:])) for line in uart.read_text().splitlines()
              if line.startswith("NATIVE SAFETY EVENT ")]
    handle = row["frame_handle"]
    def position(kind, match):
        index, = (i for i, e in enumerate(events) if e[0] == kind and match(e))
        return index

    pub = position(11, lambda e: e[4:10] == [handle, 0, 1, 1, 4, row["issue"]])
    enqueue = position(6, lambda e: e[2:5] == [0, 34, 0] and e[6:8] == [handle, 44])
    snapshot = position(7, lambda e: e[2:4] == [handle, 1])
    receive = position(6, lambda e: e[2:5] == [1, 35, 0] and e[7:9] == [handle, 44])
    release = position(6, lambda e: e[2:5] == [1, 36, 0] and e[6] == handle)
    require(pub < enqueue < snapshot < receive < release, "seq4 publish/receive/release order")
    h = row["expiry_opening"]
    if h is None:
        return None
    opening = position(17, lambda e: e[1:4] == [h["journal_tick"], h["operand"], 1])
    fault = position(3, lambda e: e[2:6] == [h["operand"], 1, 3, h["old_issue"]])
    falls = [i for i, e in enumerate(events) if e[0] == 2 and e[4:6] == [1, 0]]
    return dict(published_before_check=enqueue < fault,
                published_after_check=opening + 1 < pub,
                check_return_tick=events[opening + 1][1],
                received_after_check=opening < snapshot,
                fall_inside_check=len(falls) == 1 and fault < falls[0] < opening - 1)


def publication_kind(lo, hi, deadline):
    require(lo <= hi, "reversed publication bounds")
    if hi < deadline:
        return "before"
    if lo > deadline:
        return "after"
    return "at" if lo == hi == deadline else "unresolved"


def conjunction(*facts):
    if False in facts:
        return NO
    return UNKNOWN if None in facts else YES


def definitely_before(bounds, tick):
    if bounds[1] < tick:
        return True
    return False if bounds[0] >= tick else None


def not_before(bounds, tick):
    if bounds[0] >= tick:
        return True
    return False if bounds[1] < tick else None


def score(row, order):
    """Only replay() supplies release inputs. Unit controls call this arithmetic in memory."""
    d, h = row["D"], row["expiry_opening"]
    c = h["operand"] if h else None
    publication = [d + t for t in row["publication_minus_D"]]
    kind = publication_kind(*publication, d)
    head = "unobserved"
    if h:
        if h["occupancy"] == 0 and (h["handle"], h["length"]) == (0, 0):
            head = "empty"
        elif h["occupancy"] > 0 and (h["handle"], h["length"]) == (row["frame_handle"], 44):
            head = "seq4"
        else:
            head = "different-head"
    require(row["seq4_at_D"] in ("unpublished", "queued", "already-consumed", "publication-overlaps-D",
                                 "receive-overlaps-D"), "unknown queue evidence at D")
    transitions = row["output_transitions"]
    for t in transitions:
        lo, hi = t["store_bounds"]
        require(0 <= lo <= hi and (t["before"], t["after"]) in ((0, 1), (1, 0)), "invalid device transition")
    rises = [t["store_bounds"] for t in transitions if t["after"] == 1]
    falls = [t["store_bounds"] for t in transitions if t["after"] == 0]
    fall = falls[0] if len(falls) == 1 else None
    edges = [t["after"] for t in transitions] == [1, 0]
    delay_d = [t - d for t in fall] if fall else None
    delay_c = [t - c for t in fall] if fall and h else None

    # Order in the serialized service and counter bounds are separate witnesses.
    # A straddling store bracket alone cannot decide which side of the operand it occupies.
    resolved_fall = None
    prior = None
    if h and edges:
        prior = definitely_before(rises[0], min(c, d))
        resolved_fall = not_before(fall, c)
        if fall[0] > h["journal_tick"]:
            resolved_fall = False
        elif fall[1] > h["journal_tick"] and resolved_fall is True:
            resolved_fall = None
    terminal_safe = (row["terminal_output"] == 0 and row["ended_on_seq3"] and row["commit_skipped"] and
                     row["receiver_decision_status"] == 17 and not row["post_seq3_commit_calls"] and
                     row["authority_confirmed_fault_and_seq3"] and row["authority_post_expiry_flags"] == [5, 5])
    protected = (h is not None and
                 (h["generation"], h["old_sequence"], h["old_fault"], h["new_fault"], h["gate"],
                  h["active_after"], h["flags"]) == (1, 3, 0, 1, 1, 1, 3) and
                 d == h["old_issue"] + HZ // 50 and h["old_issue"] < row["issue"] <= d < row["issue"] + HZ // 50)
    functional = conjunction(protected, row["frame_length"] == 44, h is not None and c >= d,
                             row["prior_output_high"], edges, prior, resolved_fall,
                             bool(order and order["fall_inside_check"] and order["received_after_check"]),
                             row.get("private_entry_newly_expired"), terminal_safe,
                             h is not None and row["receiver_decision_minus_C"] >= 0)
    queue_d = row["seq4_at_D"]

    def candidate(target, queue_condition, ordered_head):
        if kind == "unresolved":
            possible = {"before": publication[0] < d, "after": d < publication[1], "at": True}[target]
            timing = None if possible else False
        else:
            timing = kind == target
        return conjunction({YES: True, NO: False, UNKNOWN: None}[functional], c == d,
                           timing, queue_condition, ordered_head)

    ambiguous_queue = queue_d in ("publication-overlaps-D", "receive-overlaps-D")
    published_first = bool(order and order["published_before_check"] and not order["published_after_check"])
    checked_first = bool(order and order["published_after_check"] and not order["published_before_check"])
    queued = published_first and head == "seq4"
    empty = checked_first and head == "empty" and order["check_return_tick"] <= publication[1]
    before = candidate("before", None if ambiguous_queue else queue_d == "queued", queued)
    after = candidate("after", None if ambiguous_queue else queue_d == "unpublished", empty)
    # One tick may contain both serialized operations in either order. The direct
    # head must match that order, and the controller must subsequently receive seq4.
    equality = candidate("at", True, queued or (empty and order["check_return_tick"] <= d))
    return dict(run=row["run"], receipt_sha256=row["receipt_sha256"], clock=row["clock"], policy=row["policy"],
                calendar=row["calendar"], D=d, C=c, check_minus_D=c - d if h else None,
                publication=kind, publication_bounds=publication, queue_at_D=queue_d, queue_at_C=head,
                prior_output_high=row["prior_output_high"], output_transitions=transitions, loan_order=order,
                fall_store_bounds=fall, fall_minus_D=delay_d, fall_minus_C=delay_c, functional_expiry=functional,
                exact_before=before, exact_after=after, exact_equality=equality)


def clock_coverage(rows):
    require(len({r["receipt_sha256"] for r in rows}) == len(rows), "duplicate receipt")
    matrix = {}
    for clock in CLOCKS:
        clock_rows = [r for r in rows if r["clock"] == clock]
        policies = {}
        for policy in POLICIES:
            boots = [r for r in clock_rows if r["policy"] == policy]
            require([r["calendar"] for r in boots] == list(CASES), "incomplete clock/policy matrix")
            witnesses = {key: [r["run"] for r in boots if r[key] == YES]
                         for key in ("exact_before", "exact_after", "exact_equality")}
            pair = any(a != b for a in witnesses["exact_before"] for b in witnesses["exact_after"])
            policies[policy] = dict(witnesses=witnesses, before_after_at_D="covered" if pair else "pending",
                                    exact_equality="covered" if witnesses["exact_equality"] else "pending")
        section = {key: "covered" if all(p[key] == "covered" for p in policies.values()) else "pending"
                   for key in ("before_after_at_D", "exact_equality")}
        section.update(by_policy=policies, boots=len(clock_rows),
                       publication_counts=dict(Counter(r["publication"] for r in clock_rows)),
                       exact_check_count=sum(r["C"] == r["D"] for r in clock_rows),
                       functional_expiries=sum(r["functional_expiry"] == YES for r in clock_rows))
        matrix[clock] = section
    return matrix


def evaluate(root):
    ledger = packet_ledger(root)
    rows = [score(r, loan_order(r, root / "runs" / r["run"] / "cap/uart.bin")) for r in replay(root, ledger)]
    return dict(source="frozen-403-native-first-boots", built_revision=REVISION, public_revision=PUBLIC_REVISION,
                tree=TREE, packet_manifest_sha256=PACKET_SUMS, decoder_output_sha256=REPLAY_SHA256,
                counter_hz=HZ, runs=rows, by_clock=clock_coverage(rows))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", type=Path, help="extracted #403 native arrivals packet")
    args = parser.parse_args()
    try:
        print(json.dumps(evaluate(args.packet.resolve()), indent=2))
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        parser.exit(1, f"Invalid or incomplete #403 evidence: {exc}\n")
