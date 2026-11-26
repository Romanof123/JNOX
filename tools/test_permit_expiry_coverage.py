"""Counterfactual arithmetic and evidence controls for #404. No native boots are represented."""

import unittest

from permit_expiry_coverage import CASES, CLOCKS, HZ, NO, POLICIES, UNKNOWN, YES, clock_coverage, score


D = 2_000_000


def control(offsets=(-2, -1), check=0, queued=True):
    c = D + check
    # In the service-first same-tick control the store and service return fit that tick.
    finish = c + 20 if queued else c
    fall = [c+3, c+5] if queued else [c, c]
    head = dict(operand=c, journal_tick=finish, old_issue=D - HZ // 50, generation=1, old_sequence=3,
                old_fault=0, new_fault=1, gate=1, active_after=1, flags=3,
                occupancy=int(queued), handle=16 if queued else 0, length=44 if queued else 0)
    at_d = "queued" if offsets[1] < 0 else "unpublished" if offsets[0] > 0 else "publication-overlaps-D"
    row = dict(run="counterfactual", receipt_sha256="counterfactual", clock="count0", policy="fifo",
               calendar=CASES[0], D=D, issue=D-100, expiry_opening=head, publication_minus_D=offsets,
               frame_handle=16, frame_length=44, seq4_at_D=at_d, prior_output_high=True,
               output_transitions=[dict(before=0, after=1, store_bounds=[D-80, D-75]),
                                   dict(before=1, after=0, store_bounds=fall)],
               private_entry_newly_expired=True, receiver_decision_minus_C=30,
               ended_on_seq3=True, commit_skipped=True, receiver_decision_status=17,
               post_seq3_commit_calls=[], authority_confirmed_fault_and_seq3=True,
               authority_post_expiry_flags=[5, 5], terminal_output=0)
    chronology = dict(published_before_check=queued, published_after_check=not queued, check_return_tick=finish,
                      received_after_check=True, fall_inside_check=True)
    return row, chronology


class CounterfactualCoverage(unittest.TestCase):
    def test_positive_exact_witnesses_and_late_safe_boot(self):
        for offsets, head, target in [((-2, -1), True, "before"), ((2, 4), False, "after"),
                                      ((0, 0), True, "equality"), ((0, 0), False, "equality")]:
            with self.subTest(target=target, head=head):
                r = score(*control(offsets, queued=head))
                self.assertEqual(r["exact_" + target], YES)
                self.assertEqual(r["functional_expiry"], YES)
                self.assertEqual(r["fall_minus_D"], [3, 5] if head else [0, 0])
                self.assertEqual(r["fall_minus_C"], r["fall_minus_D"])
        late = score(*control(check=1))
        self.assertEqual((late["functional_expiry"], late["exact_before"]), (YES, NO))
        self.assertEqual((late["fall_minus_D"], late["fall_minus_C"]), ([4, 6], [3, 5]))

    def test_crossing_and_touching_publication_brackets(self):
        for bounds in ((-1, 0), (-1, 1), (0, 1)):
            for queued in (True, False):
                with self.subTest(bounds=bounds, queued=queued):
                    r = score(*control(bounds, queued=queued))
                    self.assertEqual((r["publication"], r["exact_equality"]), ("unresolved", UNKNOWN))
        with self.assertRaisesRegex(ValueError, "reversed"):
            score(*control((2, 1)))
        row, order = control()
        row["seq4_at_D"] = "receive-overlaps-D"
        self.assertEqual(score(row, order)["exact_before"], UNKNOWN)

    def test_device_bounds_and_chronology(self):
        for bounds, outcome in [((-4, -2), NO), ((-1, 1), UNKNOWN), ((0, 2), YES), ((18, 22), UNKNOWN)]:
            with self.subTest(bounds=bounds):
                row, order = control()
                row["output_transitions"][1]["store_bounds"] = [D + v for v in bounds]
                r = score(row, order)
                self.assertEqual((r["exact_before"], r["fall_minus_C"]), (outcome, list(bounds)))
                self.assertEqual(r["fall_store_bounds"], [D + v for v in bounds])
        row, order = control()
        order["fall_inside_check"] = False
        self.assertEqual(score(row, order)["exact_before"], NO)
        row, order = control()
        row["output_transitions"][0]["store_bounds"] = [D-1, D+1]
        self.assertEqual(score(row, order)["exact_before"], UNKNOWN)
        row["output_transitions"].append(dict(before=0, after=1, store_bounds=[D+50, D+51]))
        self.assertEqual(score(row, order)["exact_before"], NO)

    def test_wrong_front_and_missing_safety_evidence(self):
        for section, key, value in (
            ("expiry_opening", "handle", 12), ("expiry_opening", "length", 43),
            ("expiry_opening", "gate", 0), ("row", "prior_output_high", False),
            ("row", "terminal_output", 1), ("row", "post_seq3_commit_calls", [[1, 60, 2]]),
            ("order", "published_before_check", False), ("order", "received_after_check", False),
        ):
            with self.subTest(key=key):
                row, order = control((0, 0))
                {"expiry_opening": row["expiry_opening"], "row": row, "order": order}[section][key] = value
                self.assertEqual(score(row, order)["exact_equality"], NO)
        row, order = control()
        row["output_transitions"] = []
        result = score(row, order)
        self.assertEqual((result["exact_before"], result["fall_minus_C"]), (NO, None))

    def test_same_tick_requires_agreement_between_head_and_operation_order(self):
        for queued in (True, False):
            for before, after in ((False, False), (True, True), (not queued, queued)):
                with self.subTest(queued=queued, before=before, after=after):
                    row, order = control((0, 0), queued=queued)
                    order.update(published_before_check=before, published_after_check=after)
                    self.assertEqual(score(row, order)["exact_equality"], NO)
        row, order = control((0, 0), queued=False)
        row["expiry_opening"].update(occupancy=1, handle=12, length=44)
        self.assertEqual(score(row, order)["exact_equality"], NO)
        row, order = control((0, 0), queued=False)
        order["check_return_tick"] = D + 1
        self.assertEqual(score(row, order)["exact_equality"], NO)

    def test_clock_policy_and_independent_boot_matrix(self):
        rows = []
        for clock in CLOCKS:
            for policy in POLICIES:
                for calendar, offsets, queued in zip(CASES, ((-2, -1), (0, 0), (1, 2)), (True, True, False)):
                    r = score(*control(offsets, check=int(clock == "default"), queued=queued))
                    name = f"counterfactual-{clock}-{policy}-{calendar}"
                    rows.append(dict(r, run=name, receipt_sha256=name, clock=clock, policy=policy, calendar=calendar))
        matrix = clock_coverage(rows)
        self.assertEqual((matrix["default"]["before_after_at_D"], matrix["count0"]["before_after_at_D"]),
                         ("pending", "covered"))
        self.assertEqual(matrix["count0"]["exact_equality"], "covered")
        rows[-1]["exact_after"] = NO
        self.assertEqual(clock_coverage(rows)["count0"]["before_after_at_D"], "pending")
        with self.assertRaisesRegex(ValueError, "incomplete"):
            clock_coverage(rows[:-1])
        rows[-1]["receipt_sha256"] = rows[0]["receipt_sha256"]
        with self.assertRaisesRegex(ValueError, "duplicate"):
            clock_coverage(rows)


if __name__ == "__main__":
    unittest.main()
