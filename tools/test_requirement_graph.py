"""Offline controls for requirement_graph.py. Run with python tools/test_requirement_graph.py."""
import tempfile
import unittest
from pathlib import Path

from requirement_graph import inspect


class GraphTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / "docs").mkdir()
        (self.root / "trace").mkdir()
        documents = {
            "requirements": "| `REQ-001` Title | OBJ-001 | mentions REQ-999 | DES-001 · VER-001 |",
            "product-plan": "| `OBJ-001` | objective |",
            "safety-security-concept": "\n".join(f"| `{k}-001` | rationale |" for k in ("THR", "HAZ", "ASM")),
            "architecture": "## DES-001 Design",
            "verification-plan": "| `VER-001` | REQ-001 | acceptance |",
        }
        for doc, content in documents.items():
            for suffix in (".en", ""):
                (self.root / f"docs/{doc}{suffix}.md").write_text(content, encoding="utf-8")
        self.csv = self.root / "trace/links.csv"
        self.csv.write_text(
            "source_id,relationship,target_id\nOBJ-001,derives,REQ-001\n"
            "REQ-001,implemented-by-design,DES-001\nREQ-001,verified-by,VER-001\n",
            encoding="utf-8",
        )

    def test_reverse_view_is_source_located(self):
        report, errors = inspect(self.root, "VER-001")
        self.assertEqual(errors, [])
        self.assertEqual(len(report), 2)
        for expected in ("REQ-001 verified-by VER-001", "trace/links.csv:4",
                         "docs/verification-plan.en.md:1", "docs/verification-plan.md:1"):
            self.assertIn(expected, report[1])

    def test_unrelated_relation_and_undefined_endpoint_cannot_supply_coverage(self):
        original = self.csv.read_text()
        self.csv.write_text(original.replace("verified-by", "related") +
                            "REQ-001,verified-by,VER-999\nCAP-001,affects,REQ-999\n")
        _, errors = inspect(self.root, "OBJ-001")
        for message in ("missing VER coverage", "missing declared edge", "undefined endpoint VER-999",
                        "undefined endpoint REQ-999"):
            self.assertTrue(any(message in error for error in errors), errors)

    def test_duplicate_and_malformed_rows(self):
        with self.csv.open("a") as stream:
            stream.write("OBJ-001,derives,REQ-001\nOBJ-001,derives\n")
        _, errors = inspect(self.root)
        self.assertEqual(len(errors), 2)
        self.assertIn("duplicate edge", errors[0])
        self.assertIn("malformed CSV row", errors[1])

    def test_different_declarations_and_allocations(self):
        path = self.root / "docs/requirements.md"
        row = path.read_text(encoding="utf-8")
        path.write_text(row.replace("OBJ-001", "ASM-001") + "\n" + row.replace("REQ-001", "REQ-002"), encoding="utf-8")
        _, errors = inspect(self.root)
        for expected in ("edition mismatch REQ-002", "REQ-001 edition allocation mismatch",
                         "ASM-001 constrains REQ-001", "REQ-002 missing upstream coverage"):
            self.assertTrue(any(expected in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
