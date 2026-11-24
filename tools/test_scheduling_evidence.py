"""Offline host tests using synthetic trace tables and bilingual records."""

import csv
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from scheduling_evidence import POLICIES, audit, main


class SchedulingEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.docs = self.root / "docs"
        self.records = self.docs / "evidence"
        self.records.mkdir(parents=True)
        (self.root / "trace").mkdir()
        self.table = self.root / "trace/links.csv"
        self.rows = [
            ("REQ-023", "implemented-by-design", "DES-012"),
            ("REQ-023", "verified-by", "VER-023"),
            ("CHG-017", "affects", "REQ-023"),
            ("DES-012", "implemented-by", "IMP-004"),
            ("IMP-004", "tested-in", "EVD-004"),
            ("IMP-004", "partially-verified-by", "VER-023"),
            ("VER-023", "has-partial-evidence", "EVD-004"),
            ("POL-002", "has-switch-evidence", "EVD-004"),
            ("POL-002", "partially-implemented-by", "IMP-004"),
        ]
        for policy in POLICIES:
            self.rows.append(("REQ-023", "allocated-to-policy", policy))
            self.rows.append((policy, "has-partial-policy-evidence", "EVD-004"))
        self.bilingual(self.docs / "scheduling-policies",
                       "\n".join("| %s · family | rule |" % p for p in POLICIES))
        self.bilingual(self.docs / "scheduling-release-cases",
                       "| Policy | Recorded subcases | Remaining work |")
        self.bilingual(self.docs / "requirements", "| REQ-023 | policy selection |")
        self.bilingual(self.docs / "architecture", "## DES-012 policy layers")
        self.bilingual(self.docs / "verification-plan", "| VER-023 | tests |")
        self.bilingual(self.records / "evd-004",
                       "# EVD-004\nCHG-017 IMP-004 [first](first.json) [second](second.json)")
        for name in ("first", "second"):
            (self.records / (name + ".json")).write_text("{}", encoding="utf8")

    def bilingual(self, stem, text):
        for suffix in (".md", ".en.md"):
            stem.with_suffix(suffix).write_text(text + "\n", encoding="utf8")

    def write_table(self):
        with self.table.open("w", encoding="utf8", newline="") as stream:
            writer = csv.writer(stream)
            writer.writerow(["source_id", "relationship", "target_id"])
            writer.writerows(self.rows)

    def report(self):
        self.write_table()
        return audit(self.root)

    def test_shared_record_preserves_policy_specific_class_and_path(self):
        result = self.report()
        self.assertEqual(result["findings"], [])
        self.assertEqual(len(result["policies"]), 18)
        decisions = result["policies"]["POL-010"]
        self.assertEqual(decisions["classification"], "decision_only")
        self.assertEqual(decisions["direct_implementation_verification_paths"], {})
        live = result["policies"]["POL-002"]
        self.assertEqual(live["classification"], "live_switch_recorded")
        self.assertEqual(live["direct_implementation_verification_paths"],
                         {"IMP-004": {"VER-023": ["EVD-004"]}})
        evd = result["evidence"]["EVD-004"]
        path = evd["tested_implementations"]["IMP-004"]
        self.assertEqual(path["requirements_for_verifications"], {"REQ-023": ["DES-012"]})
        self.assertEqual(evd["affected_requirements"], {"CHG-017": ["REQ-023"]})
        self.assertEqual(len(evd["json_attachments"]), 2)

    def test_prose_citation_and_host_analysis_do_not_claim_a_switch(self):
        for name in ("scheduling-policies.md", "scheduling-policies.en.md"):
            with (self.docs / name).open("a", encoding="utf8") as stream:
                stream.write("| archived case | POL-010 | [EVD-005](evidence/evd-005.md) |\n")
        self.bilingual(self.records / "evd-005", "# EVD-005\nIMP-005 [sample](first.json)")
        self.rows.extend([("IMP-005", "tested-in", "EVD-005"),
                          ("IMP-005", "partially-verified-by", "VER-023"),
                          ("VER-023", "has-partial-evidence", "EVD-005"),
                          ("POL-003", "has-analysis-evidence", "EVD-005")])
        result = self.report()
        self.assertEqual(result["findings"], [])
        self.assertEqual(result["policies"]["POL-010"]["classification"], "decision_only")
        self.assertEqual(result["policies"]["POL-010"]["catalog_citations_without_policy_edge"],
                         ["EVD-005"])
        self.assertEqual(result["policies"]["POL-003"]["classification"], "analysis_recorded")

    def test_removing_amc_policy_claim_leaves_directly_tested_release_citation(self):
        claim = ("POL-015", "has-partial-policy-evidence", "EVD-058")
        self.rows.extend([claim, ("POL-015", "partially-implemented-by", "IMP-058"),
                          ("IMP-058", "tested-in", "EVD-058"),
                          ("IMP-058", "partially-verified-by", "VER-023"),
                          ("VER-023", "has-partial-evidence", "EVD-058")])
        self.bilingual(self.records / "evd-058", "# EVD-058\nIMP-058")
        self.bilingual(self.docs / "scheduling-release-cases",
                       "| POL-015 | [EVD-058](evidence/ordinary-amc-finite-first.en-ko.md) | M5 |")
        self.assertEqual(self.report()["findings"], [])
        self.rows.remove(claim)
        self.assertEqual(self.report()["findings"], [
            "catalog and direct implementation cite EVD-058 without a typed POL-015 evidence link"])

    def test_release_context_and_pending_work_do_not_assign_ownership(self):
        self.rows.remove(("POL-002", "has-switch-evidence", "EVD-004"))
        self.rows.remove(("POL-002", "has-partial-policy-evidence", "EVD-004"))
        self.rows.remove(("POL-010", "has-partial-policy-evidence", "EVD-004"))
        self.bilingual(self.docs / "scheduling-release-cases",
                       "| POL-002 | [EVD-090](static.md) | [EVD-004](board-plan.md) |\n"
                       "| POL-010 | [EVD-004](shared.md) | product workload |")
        result = self.report()
        self.assertEqual(result["findings"], [])
        self.assertNotIn("EVD-090", result["evidence"])
        self.assertEqual(result["policies"]["POL-010"]["classification"], "no_classified_record")

    def test_unrelated_verification_evidence_cannot_complete_path(self):
        self.rows.remove(("VER-023", "has-partial-evidence", "EVD-004"))
        self.rows.append(("VER-023", "has-partial-evidence", "EVD-999"))
        self.rows.append(("POL-005", "partially-implemented-by", "IMP-999"))
        findings = self.report()["findings"]
        self.assertIn("no matching evidence for POL-002 IMP-004 VER-023", findings)
        self.assertIn("dangling implementation for POL-005: IMP-999", findings)
        self.assertIn("dangling evidence target EVD-999 from VER-023", findings)

    def test_cycles_and_reversed_edge_types_are_reported(self):
        self.rows.append(("EVD-004", "tested-in", "IMP-004"))
        findings = self.report()["findings"]
        self.assertTrue(any(item.startswith("cycle in scheduling trace:") for item in findings))
        self.assertIn("wrong endpoint types: EVD-004 tested-in IMP-004", findings)

    def test_missing_translation_definition_and_capture_are_independent_findings(self):
        (self.records / "evd-004.en.md").unlink()
        (self.records / "second.json").write_text("[", encoding="utf8")
        (self.docs / "architecture.en.md").write_text("## DES-013\n", encoding="utf8")
        findings = self.report()["findings"]
        self.assertTrue(any("unreadable evidence document docs/evidence/evd-004.en.md" in x
                            for x in findings))
        self.assertTrue(any("unreadable JSON capture docs/evidence/second.json" in x
                            for x in findings))
        self.assertIn("undefined bilingual DES record DES-012", findings)

    def test_cli_input_error_is_json(self):
        self.table.write_text("from,relation,to\n", encoding="utf8")
        output = io.StringIO()
        with redirect_stdout(output):
            self.assertEqual(main(["--root", str(self.root)]), 2)
        self.assertIn("input_error", json.loads(output.getvalue()))


if __name__ == "__main__":
    unittest.main()
