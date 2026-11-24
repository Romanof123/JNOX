"""Offline synthetic controls only. These bytes are never sent to a guest."""
import unittest

from permit_cost_guard import ContainmentError, DEVICE, Guard, Lines


class GuardVectors(unittest.TestCase):
    def setUp(self):
        self.guard = Guard("F")
        self.lines = Lines(self.guard.mechanism)
        self.send("reset", "reset")

    def send(self, event, body, device=DEVICE):
        self.lines.feed(f"pl061_{event} {device} {body}\n".encode())

    def test_split_output_and_interleaved_exceptions(self):
        self.lines.feed(b"Taking exception 5 [IRQ]\n")
        self.send("write", "offset 0x400 value 0x1")
        self.lines.feed(b"...with ELR 0x40200000\n")
        self.lines.feed(f"pl061_set_output {DEVICE} setting output ".encode())
        with self.assertRaises(ContainmentError):
            self.lines.feed(b"0 to 1\n")

    def test_aliased_data_write(self):
        with self.assertRaises(ContainmentError):
            self.send("write", "offset 0x3fc value 0x1")

    def test_gpio_id_and_direction(self):
        self.send("set_output", "setting output 0 to 1", "/machine/other")
        self.send("set_output", "setting output 1 to 1")
        self.send("write", "offset 0x8 value 0x1")
        self.send("write", "offset 0x400 value 0x1")
        self.send("update", "GPIODIR 0x0 GPIODATA 0x1 pullups 0x0 floating 0x0")
        self.send("set_output", "setting output 0 to 0")
        self.send("set_output", "setting output 0 to 0")
        with self.assertRaises(ContainmentError):
            self.send("update", "GPIODIR 0x1 GPIODATA 0x1 pullups 0x0 floating 0x0")

    def test_partial_or_malformed(self):
        self.lines.feed(b"pl061_wri")
        with self.assertRaises(ValueError):
            self.lines.finish()
        with self.assertRaises(ValueError):
            self.send("te", "bad record")

    def test_reset_and_filter(self):
        with self.assertRaises(ValueError):
            self.send("reset", "reset")
        reduced = Guard("R")
        with self.assertRaises(ValueError):
            reduced.mechanism(f"pl061_set_output {DEVICE} setting output 0 to 0")
        reduced.mechanism(f"pl061_reset {DEVICE} reset")
        with self.assertRaises(ValueError):
            reduced.mechanism(f"pl061_read {DEVICE} offset 0x4 value 0x0")

    def test_guest_request_is_not_actual_output(self):
        self.guard.uart("80 1 1 1")
        with self.assertRaises(ContainmentError):
            self.guard.uart("82 2 1 1")


if __name__ == "__main__":
    unittest.main()
