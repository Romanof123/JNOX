"""Standalone UART observer cases using synthetic timer and frame samples."""

import unittest

from amc_observer import MAX_BYTES, validate


def witness(freq=20_003, origin=750, irq_delay=4, commit_delay=2, work_delay=5,
            code=0x40201400, stack=0x40403FF0, pc_offsets=(0, 224, 216, 212, 108, 156)):
    quantum = freq // 20
    lines = ["JNOX M1 BOOT", "JNOX M1 READY", "AMC BEGIN",
             f"AMC CLOCK freq={freq} origin={origin} quantum={quantum}"]
    jobs = (
        (0, 0, 1, 0, 3, 36, 0, 150),
        (1, 0, 1, 3, 5, 0, 2, 18),
        (1, 1, 2, 2, 5, 0, 2, 992),
        (1, 1, 3, 1, 5, 0, 2, 1015),
        (3, 1, 1, 0, 6, 528, 4, 202),
        (0, 0, 1, 0, 8, 136, 5, 39),
    )
    for t, (task, mode, used, left, due, value, instance, progress) in enumerate(jobs):
        cval = origin + (t + 1) * quantum
        lines.append(f"AMC SLOT t={t} id={task} mode={mode} used={used} left={left} "
                     f"deadline={due} cval={cval} irq={cval + irq_delay} "
                     f"elr={code + pc_offsets[t]} sp={stack + instance * 16384} "
                     f"progress={progress} value={value}")
    trigger = origin + 2 * quantum + irq_delay
    commit = trigger + commit_delay
    work = commit + work_delay
    lines += [
        f"AMC SWITCH t=2 carry=2,3,2,1 drop=5 irq={trigger} commit={commit} "
        f"work={work} latency={irq_delay + commit_delay + work_delay}",
        "AMC GUARD new_lo=0 early_reset=0 fourth=0 fault=1",
        f"AMC DEADLINE id=3 end=5 due=6 irq={origin + 5 * quantum + irq_delay} "
        f"limit={origin + 6 * quantum} value=528",
        "AMC RESET unapproved=0 approved=1", "AMC STOP", "AMC PASS", "",
    ]
    return "\n".join(lines).encode("ascii")


def change(data, row, field, value):
    """Change exactly one named field on the chosen UART row."""
    lines = data.split(b"\n")
    words = lines[row].split()
    prefix = field.encode("ascii") + b"="
    index, = [i for i, word in enumerate(words) if word.startswith(prefix)]
    words[index] = prefix + str(value).encode("ascii")
    lines[row] = b" ".join(words)
    return b"\n".join(lines)


class AmcObserverTests(unittest.TestCase):
    def test_accepts_distinct_clocks_frame_addresses_progress_and_timing(self):
        self.assertTrue(validate(witness()))
        varied = witness(freq=62_500_000, origin=8_000_000, irq_delay=60, commit_delay=8,
                         work_delay=20_000, code=0x40203000, stack=0x40500010,
                         pc_offsets=(4, 212, 244, 216, 104, 160))
        self.assertTrue(validate(change(varied, 6, "progress", 999)))
        # Latest representable exclusive end of the final IRQ window.
        self.assertTrue(validate(witness(freq=20, origin=(1 << 64) - 8, irq_delay=0,
                                         commit_delay=0, work_delay=0)))
        # All three switch samples may fall in the last counter tick of the slot.
        self.assertTrue(validate(witness(irq_delay=999, commit_delay=0, work_delay=0)))

    def test_rejects_missing_duplicated_reordered_and_trailing_records(self):
        full = witness()
        rows = full.splitlines(keepends=True)
        for i, row in enumerate(rows):
            with self.subTest(i=i):
                self.assertFalse(validate(b"".join(rows[:i] + rows[i + 1:])))
                self.assertFalse(validate(b"".join(rows[:i] + [row] + rows[i:])))
        for i in range(len(rows) - 1):
            swap = rows.copy()
            swap[i], swap[i + 1] = swap[i + 1], swap[i]
            self.assertFalse(validate(b"".join(swap)))
        for data in (b"", full[:-1], full + b"\n", full + b"AMC FAIL\n", full + b"garbage",
                     full.replace(b"\n", b"\r\n"), b"x" * (MAX_BYTES + 1)):
            self.assertFalse(validate(data))

    def test_requires_exact_labels_and_canonical_bounded_integers(self):
        data = witness()
        for bad in (b"", b"+750", b"-1", b"0750", b"7.5", b"0xff", b"7_50",
                    b"18446744073709551616", b"9" * 100, b"\xff", b"750\x00", b"750 "):
            with self.subTest(bad=bad):
                self.assertFalse(validate(data.replace(b"origin=750", b"origin=" + bad)))
        for replacement in (b"ori=750", b"origin=750 x=1", b"origin=750 origin=750",
                            b"origin=750\t", b"origin =750"):
            self.assertFalse(validate(data.replace(b"origin=750", replacement)))

    def test_checks_each_clock_relation_and_sample_reference(self):
        data = witness()  # slot1 compare=2750, IRQ=2754, commit=2756, work=2761
        bad = [
            (3, "quantum", 1001), (4, "cval", 1751), (4, "irq", 1749), (4, "irq", 2750),
            (10, "irq", 2753), (10, "commit", 2753), (10, "commit", 2762),
            (10, "work", 3750), (10, "latency", 10),
            (12, "irq", 5755), (12, "limit", 6751), (12, "value", 527),
        ]
        for row, field, value in bad:
            with self.subTest(row=row, field=field):
                self.assertFalse(validate(change(data, row, field, value)))
        self.assertFalse(validate(witness(freq=19, irq_delay=0, commit_delay=0, work_delay=0)))
        self.assertFalse(validate(witness(freq=1 << 32)))
        self.assertFalse(validate(witness(freq=20, origin=(1 << 64) - 7, irq_delay=0,
                                          commit_delay=0, work_delay=0)))

    def test_checks_discard_fault_deadline_and_slot_results(self):
        data = witness()
        for row, field, value in [
            (5, "mode", 1), (6, "left", 1), (7, "id", 2), (8, "used", 2), (9, "deadline", 9),
            (8, "value", 136), (10, "carry", "2,3,0,1"), (10, "drop", 1), (11, "fourth", 1),
            (11, "new_lo", 1), (11, "early_reset", 1), (11, "fault", 3), (12, "due", 5),
            (13, "unapproved", 1), (13, "approved", 0),
        ]:
            with self.subTest(row=row, field=field):
                self.assertFalse(validate(change(data, row, field, value)))

    def test_checks_stack_instances_saved_pc_loops_and_high_job_progress(self):
        data = witness()
        for row, field, value in [
            (4, "progress", 1), (6, "progress", 18), (7, "progress", 991),
            (5, "sp", 0x40403ff0), (6, "sp", 0x4040bff0 + 16),
            (9, "sp", 0x40403ff0), (4, "elr", 0x40201402),
            (5, "elr", 0x40201400 + 108), (8, "elr", 0x40201400 + 160),
        ]:
            with self.subTest(row=row, field=field):
                self.assertFalse(validate(change(data, row, field, value)))
        self.assertFalse(validate(witness(code=0x30000000)))
        self.assertFalse(validate(witness(code=0x40500000)))
        self.assertFalse(validate(witness(stack=0x40403ff8)))
        self.assertFalse(validate(witness(stack=0x47fffff0)))


if __name__ == "__main__":
    unittest.main()
