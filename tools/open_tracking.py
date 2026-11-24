"""Check team labels and reciprocal body references in a saved GitHub REST inventory."""
import argparse
import json
import re
import sys
from pathlib import Path

REPO = "romanof123/jnox"
ROLES = {"team:" + name for name in
         ("development", "functional-safety", "integration", "pm", "systems", "verification")}
URL = re.compile(r"https://github\.com/" + REPO + r"/(issues|pull)/([1-9][0-9]*)/?", re.I)
TOKEN = re.compile(r"https?://[^\s<>\[\]()]+|(?<![\w/#\\.-])"
                   r"(?:(?P<repo>[\w.-]+/[\w.-]+))?#(?P<num>[1-9][0-9]*)(?![A-Za-z0-9_])", re.I)


def references(body):
    """Extract supported prose references, retaining the target type of URLs."""
    lines = []
    fence = ""
    for line in body.splitlines():
        marker = re.match(r"^ {0,3}(`{3,}|~{3,})(.*)$", line)
        if fence:
            if marker and marker[1].startswith(fence) and not marker[2].strip():
                fence = ""
        elif marker:
            fence = marker[1]
        elif not line.startswith(("    ", "\t", ">")):
            lines.append(line)
    prose = re.sub(r"<!--.*?(?:-->|\Z)", " ", "\n".join(lines), flags=re.S)
    prose = re.sub(r"(?<!`)(`+)(?!`).*?(?<!`)\1(?!`)", " ", prose, flags=re.S)
    # For Markdown links, the destination identifies the referenced repository.
    prose = re.sub(r"\[[^\]\n]*\]\(([^()\s]+)\)", r" \1 ", prose)
    refs = set()
    for token in TOKEN.finditer(prose):
        if token["num"]:
            if not token["repo"] or token["repo"].lower() == REPO:
                refs.add((int(token["num"]), None))
        else:
            url = URL.fullmatch(token[0].rstrip(".,;:!?"))
            if url:
                refs.add((int(url[2]), url[1].lower()))
    return refs


def audit(data):
    if not isinstance(data, list):
        raise ValueError("expected REST list or array of pages")
    rows = [row for page in data for row in page] if all(isinstance(p, list) for p in data) else data
    items = {}
    findings = []
    reports = []
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("expected an issue object")
        n = row.get("number")
        if type(n) is not int or n <= 0 or n in items:
            raise ValueError(f"invalid or duplicate item number {n!r}")
        kind = "pull" if "pull_request" in row else "issues"
        if (row.get("state") != "open" or
                str(row.get("html_url")).lower() != f"https://github.com/{REPO}/{kind}/{n}"):
            raise ValueError(f"#{n}: expected an open item of the indicated repository and kind")
        labels = row.get("labels")
        if not isinstance(labels, list) or not all(
                isinstance(label, dict) and isinstance(label.get("name"), str) for label in labels):
            raise ValueError(f"#{n}: invalid labels")
        teams = [label["name"] for label in labels if label["name"].startswith("team:")]
        if len(teams) != 1 or teams[0] not in ROLES:
            findings.append(f"#{n}: expected one recognized team label, found {teams}")
        if "body" not in row or (row["body"] is not None and not isinstance(row["body"], str)):
            raise ValueError(f"#{n}: invalid body")
        items[n] = kind, references(row["body"] or "")
    for n, (kind, refs) in sorted(items.items()):
        if kind != "pull":
            continue
        matches = []
        for number, fmt in sorted(refs, key=lambda pair: pair[0]):
            target, backlinks = items.get(number, (None, set()))
            if target == "issues" and fmt in (None, target) and (
                    (n, None) in backlinks or (n, "pull") in backlinks):
                matches.append(number)
        linked = ", ".join(f"#{i}" for i in sorted(set(matches)))
        if linked:
            reports.append(f"PR #{n}: reciprocal issues {linked}")
        else:
            findings.append(f"PR #{n}: no reciprocal open issue in supplied inventory")
    reports.insert(0, f"Supplied inventory: {len(rows)} items; archive completeness unverified")
    return reports, findings


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inventory", type=Path)
    args = parser.parse_args()
    try:
        reports, findings = audit(json.loads(args.inventory.read_text(encoding="utf-8-sig")))
    except (ValueError, OSError) as exc:
        print(f"Invalid inventory: {exc}", file=sys.stderr)
        return 2
    for line in reports + findings:
        print(line)
    return int(bool(findings))


if __name__ == "__main__":
    sys.exit(main())
