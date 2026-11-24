"""Read scheduling trace links and local bilingual evidence using Python 3.9+.

Run python tools/scheduling_evidence.py [--root REPOSITORY]. JSON goes to
stdout. Exit 0 means the inspected links are consistent; 1 means findings,
and 2 means the trace or a required catalog could not be read.

Classification follows the policy's CSV relationship, even when two policies
share the same record. A catalog citation alone carries no evidence class.
This checks references and JSON readability; it does not execute a workload,
validate measurements, assess physical timing, or change release status.
"""

import argparse
import csv
import json
import re
from collections import defaultdict
from graphlib import CycleError, TopologicalSorter
from pathlib import Path


POLICIES = tuple("POL-%03d" % i for i in range(1, 19))
CLAIMS = {
    "has-partial-policy-evidence": "decision",
    "has-switch-evidence": "live_switch",
    "has-analysis-evidence": "analysis",
}
# Only these relationships participate in the scheduling evidence graph.
# Other domains share some relationships, e.g. CAP -> EVD and ADR -> VER.
TYPES = {
    "allocated-to-policy": {"REQ:POL"},
    "partially-implemented-by": {"REQ:IMP", "POL:IMP"},
    "partially-verified-by": {"IMP:VER"},
    "tested-in": {"IMP:EVD"},
    "has-partial-evidence": {"VER:EVD", "CAP:EVD"},
    "implemented-by": {"DES:IMP"},
    "implemented-by-design": {"REQ:DES"},
    "verified-by": {"REQ:VER", "ADR:VER"},
    "affects": {"CHG:REQ", "CHG:OBJ"},
    **{relation: {"POL:EVD"} for relation in CLAIMS},
}
ID = re.compile(r"[A-Z]{3}-[0-9]{3}")
DEFINITION = re.compile(r"^(?:\|\s*|#{1,6}\s+)\x60?([A-Z]{3}-[0-9]{3})\b", re.M)
EVD_LINK = re.compile(r"\[(EVD-[0-9]{3})\]\(")
JSON_LINK = re.compile(r"\]\(([^()\s:#?]+\.json)(?:#[^()]*)?\)", re.I)


class Trace:
    def __init__(self, path):
        self.edges = []
        self.outgoing = defaultdict(set)
        self.incoming = defaultdict(set)
        with path.open(encoding="utf-8-sig", newline="") as stream:
            rows = csv.reader(stream, strict=True)
            if next(rows, None) != ["source_id", "relationship", "target_id"]:
                raise ValueError("unexpected trace/links.csv header")
            for row in rows:
                if (len(row) != 3 or not ID.fullmatch(row[0]) or
                        not ID.fullmatch(row[2]) or not row[1].strip()):
                    raise ValueError("invalid trace row %d" % rows.line_num)
                source, relation, target = row
                self.edges.append((source, relation, target))
                self.outgoing[source, relation].add(target)
                self.incoming[target, relation].add(source)


def read_evidence(root, evd, findings):
    """Follow the record's own capture links (SMP has multiple files)."""
    prefix = "docs/evidence/" + evd.lower()
    documents = []
    attachments = set()
    provenance = []
    for suffix in (".md", ".en.md"):
        name = prefix + suffix
        documents.append(name)
        try:
            text = (root / name).read_text(encoding="utf-8-sig")
        except (OSError, UnicodeError) as exc:
            findings.add("unreadable evidence document %s (%s)" % (name, type(exc).__name__))
            continue
        if not re.search(r"^#\s+\x60?" + evd + r"\b", text, re.M):
            findings.add("wrong evidence title in " + name)
        provenance.append(set(re.findall(r"\b(?:CHG|IMP)-[0-9]{3}\b", text)))
        for link in JSON_LINK.findall(text):
            if link.startswith("//"):
                continue  # A protocol-relative web link is outside this offline audit.
            if Path(link).anchor:
                findings.add("absolute capture path in " + name)
                continue
            path = (root / name).parent / link
            if not path.resolve().is_relative_to(root):
                findings.add("capture outside repository in " + name)
                continue
            attachments.add(path.resolve().relative_to(root).as_posix())
    if len(provenance) == 2 and provenance[0] != provenance[1]:
        findings.add("bilingual change/implementation references disagree for " + evd)
    for name in sorted(attachments):
        try:
            json.loads((root / name).read_text(encoding="utf-8-sig"))
        except (OSError, UnicodeError, ValueError) as exc:
            findings.add("unreadable JSON capture %s (%s)" % (name, type(exc).__name__))
    references = set().union(*provenance)
    return {
        "documents": documents, "json_attachments": sorted(attachments),
        "cited_changes": sorted(x for x in references if x.startswith("CHG-")),
        "cited_implementations": sorted(x for x in references if x.startswith("IMP-")),
    }


def catalogs(root, findings):
    definitions = {}
    cited = defaultdict(set)
    for stem, kind in (("scheduling-policies", "POL"), ("requirements", "REQ"),
                       ("architecture", "DES"), ("verification-plan", "VER")):
        languages = []
        for suffix in (".md", ".en.md"):
            text = (root / "docs" / (stem + suffix)).read_text(encoding="utf-8-sig")
            languages.append({x for x in DEFINITION.findall(text) if x.startswith(kind + "-")})
            if kind == "POL":
                for line in text.splitlines():
                    if line.startswith("|"):
                        ids = set(re.findall(r"\bPOL-[0-9]{3}\b", line))
                        if len(ids) == 1:
                            cited[next(iter(ids))].update(EVD_LINK.findall(line))
        # Only definitions present in both sources count as bilingual records.
        definitions[kind] = languages[0] & languages[1]
    for policy in POLICIES:
        if policy not in definitions["POL"]:
            findings.add("missing bilingual scheduling catalog definition for " + policy)
    release_citations = defaultdict(set)
    for suffix in (".md", ".en.md"):
        matrix = root / "docs" / ("scheduling-release-cases" + suffix)
        for line in matrix.read_text(encoding="utf-8-sig").splitlines():
            cells = line.split("|", 3)
            if len(cells) == 4 and not cells[0].strip():
                policy = cells[1].strip()
                if policy in POLICIES:
                    release_citations[policy].update(EVD_LINK.findall(cells[2]))
    return definitions, cited, release_citations


def check_graph(trace, scope, findings):
    graph = TopologicalSorter()
    endpoint_types = set().union(*TYPES.values())
    for source, relation, target in trace.edges:
        if source not in scope and target not in scope:
            continue
        signature = source[:3] + ":" + target[:3]
        if relation in TYPES and signature not in TYPES[relation]:
            findings.add("wrong endpoint types: %s %s %s" % (source, relation, target))
        elif relation not in TYPES and signature in endpoint_types:
            findings.add("unclassified relation: %s %s %s" % (source, relation, target))
        if relation in TYPES and source in scope and target in scope:
            graph.add(target, source)
    try:
        graph.prepare()
    except CycleError as exc:
        findings.add("cycle in scheduling trace: " + " -> ".join(exc.args[1]))


def trace_record(root, evd, trace, scope, findings):
    record = read_evidence(root, evd, findings)
    outgoing, incoming = trace.outgoing, trace.incoming
    owners = incoming[evd, "tested-in"]
    cited_owners = record["cited_implementations"]
    if not owners:
        findings.add("no tested-in implementation for " + evd)
    if cited_owners and not owners.intersection(cited_owners):
        findings.add("implementation provenance disagrees for " + evd)
    paths = {}
    for imp in sorted(owners):
        verified = outgoing[imp, "partially-verified-by"] & incoming[evd, "has-partial-evidence"]
        if not verified:
            findings.add("no matching IMP -> VER -> EVD path for %s %s" % (imp, evd))
        designs = incoming[imp, "implemented-by"]
        requirements = {req for ver in verified for req in incoming[ver, "verified-by"]
                        if req.startswith("REQ-")}
        paths[imp] = {
            "verifications": sorted(verified),
            "designs": sorted(designs),
            "requirements_for_verifications": {
                req: sorted(outgoing[req, "implemented-by-design"] & designs)
                for req in sorted(requirements)
            },
        }
        scope.update({imp} | verified | designs | requirements)
    record["tested_implementations"] = paths
    record["affected_requirements"] = {}
    for change in record["cited_changes"]:
        affected = sorted(x for x in outgoing[change, "affects"] if x.startswith("REQ-"))
        if not affected:
            findings.add("no affected requirements for cited change " + change)
        record["affected_requirements"][change] = affected
        scope.update([change, *affected])
    return record


def audit(root):
    root = Path(root).resolve()
    trace = Trace(root / "trace/links.csv")
    outgoing, incoming = trace.outgoing, trace.incoming
    findings = set()
    definitions, cited, release_citations = catalogs(root, findings)
    scope = set(POLICIES)
    policies, evidence = {}, {}
    for policy in POLICIES:
        allocated = incoming[policy, "allocated-to-policy"]
        if not allocated:
            findings.add("missing requirement allocation for " + policy)
        scope.update(allocated)
        direct = outgoing[policy, "partially-implemented-by"]
        scope.update(direct)
        claims = {kind: sorted(outgoing[policy, relation])
                  for relation, kind in CLAIMS.items()}
        claimed = set().union(*claims.values())
        cited_only = cited[policy] - claimed
        direct_paths = {}
        directly_tested = set()
        for imp in sorted(direct):
            tests = outgoing[imp, "tested-in"]
            verifications = outgoing[imp, "partially-verified-by"]
            directly_tested.update(tests)
            scope.update(verifications)
            if not tests and not verifications:
                findings.add("dangling implementation for %s: %s" % (policy, imp))
            direct_paths[imp] = {}
            for ver in sorted(verifications):
                matched = tests & outgoing[ver, "has-partial-evidence"]
                direct_paths[imp][ver] = sorted(matched)
                if not matched:
                    findings.add("no matching evidence for %s %s %s" % (policy, imp, ver))
        # Release rows also cite contextual records. Only a direct tested-in
        # path puts such a citation within this policy's ownership check.
        missing_claims = (cited_only | (release_citations[policy] - claimed)) & directly_tested
        for evd in sorted(missing_claims):
            findings.add("catalog and direct implementation cite %s without a typed %s evidence link"
                         % (evd, policy))
        for evd in sorted(claimed | cited_only | directly_tested):
            scope.add(evd)
            if not evd.startswith("EVD-"):
                continue  # check_graph reports the malformed edge
            if evd not in evidence:
                evidence[evd] = trace_record(root, evd, trace, scope, findings)
        kinds = [kind for kind, records in claims.items() if records]
        if "live_switch" in kinds:
            status = "live_switch_recorded"
        elif kinds == ["decision"]:
            status = "decision_only"
        elif kinds:
            status = "analysis_recorded"
        else:
            status = "no_classified_record"
        # Shared evidence (notably EVD-004) can have an implementation with
        # no direct POL -> IMP link. Report the two origins independently.
        policies[policy] = {"classification": status, "claims": claims,
                            "allocated_requirements": sorted(allocated),
                            "direct_implementation_verification_paths": direct_paths,
                            "catalog_citations_without_policy_edge": sorted(cited_only)}
    for node in scope:
        kind = node[:3]
        if kind in definitions and node not in definitions[kind]:
            findings.add("undefined bilingual %s record %s" % (kind, node))
        if kind == "VER":
            # Shared verification IDs can also cite other domains' records.
            # Check their targets exist without attributing them to any policy.
            for evd in outgoing[node, "has-partial-evidence"] - evidence.keys():
                if evd.startswith("EVD-") and not all(
                    (root / "docs/evidence" / (evd.lower() + suffix)).is_file()
                    for suffix in (".md", ".en.md")
                ):
                    findings.add("dangling evidence target %s from %s" % (evd, node))
    check_graph(trace, scope, findings)
    return {"schema": 1, "scope": "Policy classes describe CSV links; findings concern local "
            "references and captures. Product qualification and test execution are outside this audit.",
            "policies": policies, "evidence": evidence, "findings": sorted(findings)}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    try:
        report = audit(args.root)
    except (OSError, UnicodeError, ValueError, csv.Error) as exc:
        print(json.dumps({"schema": 1, "input_error": str(exc)}))
        return 2
    print(json.dumps(report, indent=2, sort_keys=True))
    return int(bool(report["findings"]))


if __name__ == "__main__":
    raise SystemExit(main())
