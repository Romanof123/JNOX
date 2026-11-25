"""Exercise host observation with scripted reads and a synthetic clock."""

import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import MagicMock, patch
import io
from contextlib import redirect_stderr

import pi4_serial_capture as collector

from pi4_serial_capture import EXPECTED, SECOND, build_association, observe
from pi4_transcript import PROFILES
from fixed_runtime_observer import validate


class CaptureTests(unittest.TestCase):
    def capture(self, events, *, profile="pi4-scheduler"):
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        folder = Path(temporary.name)
        chunks = iter(events)
        elapsed = 0

        def read(size):
            nonlocal elapsed
            delay, data = next(chunks)
            elapsed += delay
            self.assertLessEqual(len(data), size)
            return data

        result = observe(folder, read, now=lambda: elapsed, sleep=lambda delay: None, profile=profile)
        data = (folder / "uart.bin").read_bytes()
        self.assertEqual(result["uart_sha256"], hashlib.sha256(data).hexdigest())
        timestamps = (folder / "reads.jsonl").read_text(encoding="utf-8").splitlines()
        return result, data, [json.loads(line) for line in timestamps]

    def test_native_port_disconnect_keeps_first_result(self):
        prefix = b"JNOX PI4 BOOT\nJNOX PI4 READY\nNATIVE PI4 CNTFRQ 62500000\nNATIVE EL0"[:62]
        self.assertEqual(len(prefix), 62)
        with TemporaryDirectory() as tmp:
            output = Path(tmp) / "capture"
            argv = ["collector", "--profile", "pi4-native-rr", "--native-manifest", "manifest.json",
                    "--port", "adapter", "--boot-dir", "boot", "--elf", "kernel.elf",
                    "--board-revision", "rev", "--eeprom", "version", "--output", str(output)]
            port = MagicMock()
            port.read.side_effect = [prefix, OSError("adapter disconnected")]
            stderr = io.StringIO()
            with patch("sys.argv", argv), patch.object(collector, "artifacts", return_value={}), \
                 patch.object(collector, "native_package", return_value={}), \
                 patch.object(collector, "serial_port", return_value=port), \
                 patch("builtins.input", return_value=""), redirect_stderr(stderr):
                self.assertEqual(collector.main(), 2)
            self.assertIn("adapter disconnected", stderr.getvalue())
            run = output / "run-1"
            result = json.loads((run / "result.json").read_bytes())
            identity = json.loads((run / "identity.json").read_bytes())
            for key in ("profile", "run", "run_id", "session_id", "mode"):
                self.assertEqual(result[key], identity[key])
            self.assertEqual(result["uart_verdict"], "read_error")
            self.assertEqual(result["terminal_cause"]["type"], "OSError")
            self.assertEqual(result["bytes"], 62)
            self.assertEqual(result["read_records"], 1)
            self.assertEqual(result["uart_sha256"], hashlib.sha256(prefix).hexdigest())
            self.assertEqual((run / "uart.bin").read_bytes(), prefix)
            self.assertEqual(json.loads((run / "reads.jsonl").read_bytes())["length"], 62)
            self.assertIsNone(result["matched_variant"])
            self.assertFalse((output / "run-2").exists())

    def test_deadline_and_quiet_boundaries(self):
        result, data, log = self.capture([(SECOND, EXPECTED[:4]),
                                          (29 * SECOND, EXPECTED[4:]), (SECOND, b"")])
        self.assertEqual(result["uart_verdict"], "matched")
        self.assertEqual(data, EXPECTED)
        self.assertEqual(log[1], dict(elapsed_ns=30 * SECOND, offset=4,
                                      length=len(EXPECTED) - 4))

    def test_extra_byte_preserved(self):
        result, data, _ = self.capture([(1, EXPECTED), (SECOND // 2, b"\x00")])
        self.assertEqual(result["uart_verdict"], "unexpected_bytes")
        self.assertEqual(data, EXPECTED + b"\x00")

    def test_late_complete_and_missing_input(self):
        for data in [EXPECTED, b""]:
            with self.subTest(data=data):
                result, _, _ = self.capture([(30 * SECOND + 1, data)])
                self.assertEqual(result["uart_verdict"], "timeout")


    def test_fragmentation_and_variants(self):
        for profile, variants in PROFILES.items():
            for variant, expected in variants.items():
                for step in (1, 7, len(expected)):
                    with self.subTest(profile=profile, variant=variant, step=step):
                        chunks = [(1, expected[i:i + step]) for i in range(0, len(expected), step)]
                        result, raw, _ = self.capture(chunks + [(SECOND, b"")], profile=profile)
                        self.assertEqual((result["uart_verdict"], result["matched_variant"], raw),
                                         ("matched", variant, expected))

    def test_wrong_streams_retain_failing_read(self):
        profile = "pi4-fixed-rr"
        raw = PROFILES[profile]["rr-high-first"]
        a, b = b"FIXED RR A START\n", b"FIXED RR B START\n"
        for invalid in (raw.replace(a+b, b+a), raw.replace(a,a+a), raw.replace(a,b""),
                        raw.replace(b"PI4",b"M1"), EXPECTED, raw.replace(b"\n",b"\r\n"),
                        b"\xff", raw+b"X"):
            with self.subTest(invalid=invalid):
                result, kept, _ = self.capture([(1, invalid)], profile=profile)
                self.assertEqual(result["uart_verdict"], "unexpected_bytes")
                self.assertIsNone(result["matched_variant"])
                self.assertIsNotNone(result["first_unexpected_offset"])
                self.assertEqual(kept, invalid)
                self.assertFalse(validate(invalid, b"FIXED RR", b"PI4"))
        result, _, _ = self.capture([(1,raw[:-1]),(30*SECOND,b"")], profile=profile)
        self.assertEqual(result["uart_verdict"], "timeout")

    def test_pi_order_does_not_change_general_rr_contract(self):
        raw = PROFILES["pi4-fixed-rr"]["rr-high-first"]
        a, b = b"FIXED RR A START\n", b"FIXED RR B START\n"
        unusual = raw.replace(a+b, b+a)
        self.assertTrue(validate(unusual.replace(b"PI4", b"M1"), b"FIXED RR"))
        self.assertFalse(validate(unusual, b"FIXED RR", b"PI4"))

    def test_build_record_keeps_claim_and_capture_separate(self):
        metadata = dict(elf_sha256="e", files_sha256={"config.txt":"c"},
                        package_manifest_sha256="m", firmware_commit="f")
        with TemporaryDirectory() as temporary:
            path = Path(temporary) / "submitted.json"
            path.write_text(json.dumps(dict(metadata, revision="revision", build_options=["fifo"],
                                            profile="pi4-fixed-fifo")), encoding="utf-8")
            association = build_association(path, metadata, "pi4-fixed-fifo")
            self.assertEqual(association["status"], "submitted_hashes_match_candidate_files")
            self.assertEqual(association["capture_boot_identity"], "unverified")
            self.assertIn("profile", build_association(path, metadata, "pi4-fixed-rr")["fields_to_verify"])


if __name__ == "__main__":
    unittest.main()
