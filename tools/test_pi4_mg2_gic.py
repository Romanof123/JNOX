"""Host-only parser cases for the bounded MG2 GIC comparison."""
import unittest

from pi4_mg2_gic import model_path


class CausalRecords(unittest.TestCase):
    def test_direct_and_reverse_have_distinct_producers(self):
        cpu1 = [
            "gic_dist_write dist write at 0x00000f00 size 4: 0x00018001",
            "gic_update_bestirq cpu 0 irq 1 priority 128 cpu priority mask 255 cpu running priority 256",
            "gic_update_set_irq cpu[0]: irq = 1",
            "gic_cpu_read cpu 1 iface read at 0x0000000c: 0x00000001",
        ]
        cpu0 = [
            "gic_acknowledge_irq cpu 0 acknowledged irq 1",
            "gic_cpu_read cpu 0 iface read at 0x0000000c: 0x00000401",
            "gic_cpu_write cpu 0 iface write at 0x00000010 0x00000401",
            "gic_dist_write dist write at 0x00000f00 size 4: 0x00028001",
        ]
        logs = {0: cpu0, 1: cpu1}
        opening = model_path(logs, 1, 0, 0x18001, 0x401)
        self.assertEqual(opening["adjacent_target_best_and_level"][0]["level_1_decision_line"], 3)
        self.assertEqual(opening["target_iar_result_lines"], [2])
        self.assertEqual(opening["target_eoir_write_lines"], [3])
        reverse = model_path(logs, 0, 1, 0x28001, 1)
        self.assertEqual(reverse["sgi_write_lines"], [4])
        self.assertEqual(reverse["target_iar_result_lines"], [4])
        self.assertIsNone(reverse["adjacent_target_best_and_level"][0]["best_line"])

    def test_gap_or_other_target_does_not_attach_level(self):
        send = "gic_dist_write dist write at 0x00000f00 size 4: 0x00018001"
        best = "gic_update_bestirq cpu 0 irq 1 priority 128 cpu priority mask 255 cpu running priority 256"
        level = "gic_update_set_irq cpu[0]: irq = 1"
        for stream in (
            [send, "different event", best, level],
            [send, best, "different event", level],
            [send, best.replace("cpu 0 irq", "cpu 1 irq"), level],
        ):
            with self.subTest(stream=stream):
                route = model_path({0: [], 1: stream}, 1, 0, 0x18001, 0x401)
                self.assertIsNone(route["adjacent_target_best_and_level"][0]["level_1_decision_line"])
                self.assertFalse(route["target_ack_lines"])

    def test_control_and_duplicate_writes(self):
        blank = model_path({0: [], 1: []}, 1, 0, 0x18001, 0x401)
        self.assertEqual(blank["sgi_write_lines"], [])
        send = "gic_dist_write dist write at 0x00000f00 size 4: 0x00018001"
        duplicate = model_path({0: [], 1: [send, send]}, 1, 0, 0x18001, 0x401)
        self.assertEqual(duplicate["sgi_write_lines"], [1, 2])


if __name__ == "__main__":
    unittest.main()
