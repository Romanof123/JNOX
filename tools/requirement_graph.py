"""Inspect the bilingual requirement graph. See docs/requirement-graph.md."""
import argparse
import csv
import re
import sys
from pathlib import Path

DOCUMENTS = {
    "product-plan": ("OBJ",),
    "safety-security-concept": ("HAZ", "THR", "ASM"),
    "requirements": ("REQ",),
    "architecture": ("DES",),
    "verification-plan": ("VER",),
}
ID = r"[A-Z]+-[0-9]{3}"
KINDS = {kind for kinds in DOCUMENTS.values() for kind in kinds}
RELATION = {
    "OBJ": "derives", "HAZ": "derives", "THR": "derives", "ASM": "constrains",
    "DES": "implemented-by-design", "VER": "verified-by",
}


def kind(identifier):
    return identifier.partition("-")[0]


def allocation(req, other):
    relation = RELATION[kind(other)]
    if kind(other) in ("DES", "VER"):
        return req, relation, other
    return other, relation, req


def read_edition(root, suffix, errors):
    declarations = {}
    allocations = {}
    for doc, kinds in DOCUMENTS.items():
        path = f"docs/{doc}{suffix}.md"
        lines = (root / path).read_text(encoding="utf-8").splitlines()
        prefix = "|".join(kinds)
        heading = doc == "architecture"
        start = r"^## " if heading else r"^\|\s*`?"
        candidates = re.compile(start + rf"(?:{prefix})-")
        pattern = (rf"^## ({ID})\s+\S" if heading else rf"^\|\s*`({ID})`(?=\s|\|)")
        found = set()
        for number, line in enumerate(lines, 1):
            if not candidates.match(line):
                continue
            location = f"{path}:{number}"
            match = re.match(pattern, line)
            if not match:
                errors.append(f"{location} malformed declaration")
                continue
            identifier = match[1]
            found.add(kind(identifier))
            if identifier in declarations:
                errors.append(f"{location} duplicate declaration {identifier}")
            declarations[identifier] = location
            if doc != "requirements":
                continue
            cells = re.split(r"(?<!\\)\|", line.strip())[1:-1]
            if len(cells) != 4 or not cells[0].strip().partition("` ")[2].strip():
                errors.append(f"{location} malformed requirement row")
                continue
            expected = set()
            for cell, permitted in ((cells[1], kinds_upstream()), (cells[3], {"VER", "DES"})):
                for token in re.split(r"\s*[,·]\s*", cell.strip()):
                    item = re.fullmatch(rf"(?:`({ID})`|({ID}))", token)
                    other = next(filter(None, item.groups())) if item else ""
                    if kind(other) not in permitted:
                        errors.append(f"{location} malformed allocation {token!r}")
                    else:
                        expected.add(allocation(identifier, other))
            allocations[identifier] = expected
        for missing in sorted(set(kinds) - found):
            errors.append(f"{path} missing {missing} declarations")
    return declarations, allocations


def kinds_upstream():
    return {"OBJ", "HAZ", "THR", "ASM"}


def read_edges(root, errors):
    path = "trace/links.csv"
    edges = {}
    with (root / path).open(encoding="utf-8", newline="") as stream:
        reader = csv.reader(stream, strict=True)
        if next(reader, None) != ["source_id", "relationship", "target_id"]:
            errors.append(f"{path}:1 malformed header")
        try:
            for row in reader:
                location = f"{path}:{reader.line_num}"
                if (len(row) != 3 or not re.fullmatch(ID, row[0])
                        or not re.fullmatch(r"[a-z]+(?:-[a-z]+)*", row[1])
                        or not re.fullmatch(ID, row[2])):
                    errors.append(f"{location} malformed CSV row {row!r}")
                    continue
                edge = tuple(row)
                if edge in edges:
                    errors.append(f"{location} duplicate edge {edge} first at {edges[edge]}")
                else:
                    edges[edge] = location
        except csv.Error as exc:
            errors.append(f"{path}:{reader.line_num} malformed CSV: {exc}")
    return edges


def inspect(root, query=None):
    errors = []
    editions = [read_edition(root, suffix, errors) for suffix in (".en", "")]
    en, ko = [edition[0] for edition in editions]
    all_ids = en.keys() | ko.keys()
    edges = read_edges(root, errors)
    for identifier in sorted(en.keys() ^ ko.keys()):
        errors.append(f"edition mismatch {identifier}: {en.get(identifier, '-')} {ko.get(identifier, '-')}")
    for edge, location in edges.items():
        for endpoint in (edge[0], edge[2]):
            if kind(endpoint) in KINDS and endpoint not in all_ids:
                errors.append(f"{location} undefined endpoint {endpoint}")

        source, _, target = edge
        if kind(source) == kind(target) == "REQ":
            errors.append(f"{location} invalid requirement edge {' '.join(edge)}: no REQ-to-REQ allocation")
            continue
        if kind(source) == "REQ" and kind(target) in RELATION:
            expected = allocation(source, target)
        elif kind(target) == "REQ" and kind(source) in RELATION:
            expected = allocation(target, source)
        else:
            continue
        if edge != expected:
            errors.append(f"{location} invalid requirement edge {' '.join(edge)}: expected {' '.join(expected)}")

    report = []
    if query and query not in all_ids:
        errors.append(f"unknown declaration {query}")
    for req in sorted(i for i in all_ids if kind(i) == "REQ"):
        expected_en, expected_ko = [entry[1].get(req, set()) for entry in editions]
        if expected_en != expected_ko:
            errors.append(f"{req} edition allocation mismatch {sorted(expected_en ^ expected_ko)}")
        for edge in sorted((expected_en | expected_ko) - edges.keys()):
            errors.append(f"{req} missing declared edge {' '.join(edge)}")
        actual = []
        coverage = set()
        for edge, location in edges.items():
            source, relation, target = edge
            other = target if source == req else source
            if req not in (source, target) or kind(other) not in RELATION:
                continue
            if edge != allocation(req, other):
                continue
            actual.append((edge, location, other))
            if other in all_ids:
                coverage.add("upstream" if kind(other) in kinds_upstream() else kind(other))
        for missing in sorted({"upstream", "DES", "VER"} - coverage):
            errors.append(f"{req} missing {missing} coverage")
        if query and query != req:
            actual = [row for row in actual if row[2] == query]
            if not actual:
                continue
        report.append(f"{req}  {en.get(req, '-')}  {ko.get(req, '-')}")
        for edge, location, other in sorted(actual):
            report.append(f"  {' '.join(edge)}  {location}  {en.get(other, '-')}  {ko.get(other, '-')}")
    return report, errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("id", nargs="?")
    args = parser.parse_args()
    try:
        report, errors = inspect(Path(__file__).resolve().parents[1], args.id)
    except (OSError, UnicodeError, csv.Error) as exc:
        parser.exit(1, f"input error: {exc}\n")
    for line in report:
        print(line)
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
