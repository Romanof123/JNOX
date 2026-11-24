import unittest

from open_tracking import audit


def item(n, body, pr=False, roles=("team:pm",)):
    kind = "pull" if pr else "issues"
    row = dict(number=n, state="open", body=body, labels=[dict(name=r) for r in roles],
               html_url=f"https://github.com/Romanof123/JNOX/{kind}/{n}")
    if pr:
        row["pull_request"] = {}
    return row


class TrackingTests(unittest.TestCase):
    def test_multiple_reciprocal_issues_across_pages(self):
        reports, errors = audit([[item(4, "#1 Romanof123/JNOX#2", True)],
                                 [item(2, "https://github.com/Romanof123/JNOX/pull/4"), item(1, "#4")]])
        self.assertFalse(errors)
        self.assertIn("PR #4: reciprocal issues #1, #2", reports)

    def test_dangling_one_way_and_pr_to_pr(self):
        for other in ([], [item(1, None)], [item(1, "#4", True)]):
            with self.subTest(other=other):
                self.assertTrue(audit([item(4, "#1", True), *other])[1])

    def test_excluded_backlinks(self):
        for body in ("other/JNOX#4", "https://github.com/other/JNOX/pull/4",
                     "`#4`", "~ example\n~~~\n#4\n~~~", "    #4", "<!--\n#4\n-->",
                     "[#4](https://github.com/other/JNOX/pull/4)",
                     "https://github.com/Romanof123/JNOX/issues/4"):
            with self.subTest(body=body):
                self.assertTrue(audit([item(4, "#1", True), item(1, body)])[1])

    def test_invalid_team_assignments(self):
        for roles in ((), ("team:fiction",), ("team:pm", "team:systems")):
            self.assertTrue(audit([item(1, None, roles=roles)])[1])

    def test_duplicate_closed_and_foreign_rows(self):
        valid = item(1, None)
        for rows in ([valid, valid], [dict(valid, state="closed")],
                     [dict(valid, html_url="https://github.com/other/JNOX/issues/1")]):
            with self.assertRaises(ValueError):
                audit(rows)


if __name__ == "__main__":
    unittest.main()
