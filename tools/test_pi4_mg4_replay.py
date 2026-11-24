"""Fixed host-only journal and byte controls for both directed handoff routes."""
import struct
import unittest

from pi4_mg4_replay import corroborate_terminal, decode_terminal, handoffs


def row(kind, *words):
    return kind, tuple(words) + (0,) * (8-len(words))


def fixture(case):
    task = 2 if case else 0
    ptr = 0x3000 if case else 0x1000
    opening = (1, 0, 0, 1, 2, 0, 0, 0)
    reverse = (2, 1, task, 4, 0, ptr, 2, 1 if case else 2)
    cpu0 = (row(1,2,0x2000,0x3340), row(11,0,0,1), row(4,0,0,0x1000,0,0,0,1025),
            row(17,*opening), row(3,0,0,1,0,0,0,0,1), row(6,1,7,3,1,task),
            row(2,1,1,3,0x2000), row(2,1,task,4,ptr), row(16,*reverse), row(7,1,0,1))
    cpu1 = (row(3,0,2,2), row(16,*opening), row(7,0,0,0), row(4,1,3,0,0,0,0,1),
            row(17,*reverse), row(11,1,task,4,ptr), row(5,task,4,4))
    record = (0x3130524554464F48,1,256,0,2,*opening,2,*reverse,
              0x1000,2,2,0x2000,1,1,0x3000,2,1,0)
    return {0:cpu0,1:cpu1}, record


class HandoffControls(unittest.TestCase):
    def test_routes_and_projection(self):
        for case in range(3):
            sections, words = fixture(case)
            report = handoffs(sections, case)
            terminal = decode_terminal(struct.pack("<32Q", *words))
            checked = corroborate_terminal(terminal, sections, case)
            self.assertEqual(report["reverse"]["payload"], terminal["slots"]["reverse"]["payload"])
            self.assertTrue(checked["slots"]["reverse"]["consumption_observed"])

    def test_missing_duplicate_and_wrong_claim(self):
        sections, _ = fixture(1)
        for cpu, index in ((0,3),(0,8),(1,1),(1,4)):
            rows = sections[cpu]
            for replacement in ((), (rows[index], rows[index])):
                with self.assertRaises(ValueError):
                    handoffs({**sections,cpu:rows[:index]+replacement+rows[index+1:]}, 1)
        for cpu, index, field in ((0,7,2),(0,7,3),(1,5,3),(0,8,7),(1,4,7)):
            rows = sections[cpu]
            kind, d = rows[index]
            changed = d[:field] + (d[field]+1,) + d[field+1:]
            with self.assertRaises(ValueError):
                handoffs({**sections,cpu:rows[:index]+((kind,changed),)+rows[index+1:]}, 1)

    def test_empty_pending_and_absent_journal(self):
        _, words = fixture(0)
        for phase in (0,1,2):
            block = (phase, *(words[5:13] if phase else (0,)*8))
            raw = struct.pack("<32Q", *words[:4], *block, *((0,)*9), 0x1000,1,1,0x2000,1,1,0x3000,1,1,0)
            decoded = decode_terminal(raw)
            result = corroborate_terminal(decoded, {}, 0)
            self.assertEqual(result["slots"]["opening"]["phase"], ("Empty","Pending","Consumed")[phase])
            self.assertFalse(result["slots"]["opening"]["publication_observed"])

    def test_malformed_bytes(self):
        _, words = fixture(0)
        for field in (0,1,2,3,4,13,20,21,22,23,24,31):
            changed = words[:field] + (words[field]+1,) + words[field+1:]
            with self.assertRaises(ValueError):
                decode_terminal(struct.pack("<32Q", *changed))
        with self.assertRaises(ValueError):
            decode_terminal(bytes(255))
