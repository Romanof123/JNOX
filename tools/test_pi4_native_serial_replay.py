"""Public Pi 4 replay and native UART contract, using the four original EVD-129 boots."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

from pi4_native_transcript import Matcher


TOOLS = Path(__file__).resolve().parent
PACKET = TOOLS.parent / "docs/evidence/pi4-native-fixed-applications-first.zip"
PACKET_SHA = "39b5e911a6a0abb040c9749054a3cf5693170f5e5e50d99a48deea8b8b074445"
UART_SHA = {
    "fifo-plain": "aed598940f4e4f224e6c1f211d127501965a552cbceb8212c0daf899b5434cc2",
    "fifo-traced": "f05f32fa0f210246ed0b2ca453596f9a339ab28527c41d92cf8f89cd0b586e83",
    "rr-plain": "c8dd752d4a3cd734e3b4004b6a819cde2fbf50b200bbdec87a67428bc3c20605",
    "rr-traced": "74ec659d03bff362c43d7d3a12448639d79f8c332682fd01194c268a94d9af81",
}
MANIFEST_SHA = {
    "fifo": "957d66fad211b79bcf506bb775b50664b1f64ef65f0797f27fc880b6a9aaf591",
    "rr": "f605f74d0942b2a9f4835436dbfc4e154b07be4ce550f6ec63e132034ad2da0a",
}
APP_SHA = (
    b"4e106b3e8355870dcc41bb015be760233d4eeb145de8654d8569e56513e386fa",
    b"915e86f291706cbb460dadd8b0e21905cab534285bfd8c61ea5aba75f4e7b0c8",
)
EVENT = b"NATIVE FIXED EVENT "
NO_PID = (1 << 64) - 1
BOOT_FILES = ("kernel8.img", "config.txt", "SHA256SUMS", "bcm2711-rpi-4-b.dtb",
              "COPYING.linux", "LICENCE.broadcom", "fixup4.dat", "start4.elf",
              "overlays/disable-bt.dtbo")
PACKAGE_FILES = ("kernel.elf", "kernel8.img", "user.elf", "native_peer.elf",
                 "manifest.json", "build.log")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def json_file(path):
    return json.loads(path.read_bytes())


def early_rr_exits(original, rotations):
    """Retire both live workers normally after zero or one real rotation.

    Preserve the original prefix and re-account both synthetic exits. These
    cases lack the complete pre-exit A->B->A observation even with zero drops.
    """
    lines = original.splitlines(keepends=True)
    first = next(i for i, line in enumerate(lines) if line.startswith(EVENT))
    last = first + rotations
    fields = [int(v) for v in lines[last][len(EVENT):].split()]
    now, current, compare = fields[0], fields[4], fields[11]
    service, budgets = fields[15:18], fields[18:21]
    saved = [(0x400000, 16846768)] * 2
    if rotations:
        saved[fields[3]] = (fields[5], fields[6])
    replacement = []
    for following in (1 - current, NO_PID):
        now += 1
        service[current] += 1
        budgets[current] -= 1
        resumed = (0, 0) if following == NO_PID else saved[following]
        event = [now, 3, 0, current, following, 4194336, 16846768, *resumed, 0,
                 compare, 0, NO_PID, NO_PID, NO_PID, *service, *budgets]
        replacement.append(EVENT + b" ".join(str(v).encode() for v in event) + b"\n")
        current, compare = following, 0
    return b"".join(lines[:last + 1] + replacement + lines[-3:])


def full_journal(original, retained_events, dropped):
    """Fill the bounded trace with early timer polls by its current owner.

    The retained original transitions specify any policy observation. An early
    poll charges one tick and retains the current worker and its compare.
    """
    lines = original.splitlines(keepends=True)
    first = next(i for i, line in enumerate(lines) if line.startswith(EVENT))
    prefix = lines[:first + retained_events]
    row = [int(v) for v in prefix[-1][len(EVENT):].split()]
    current, pc, sp, compare = row[4], row[7], row[8], row[11]
    service, budgets = row[15:18], row[18:21]
    for tick in range(row[0] + 1, row[0] + 1 + 128 - retained_events):
        service[current] += 1
        budgets[current] = max(0, budgets[current] - 1)
        poll = [tick, 4, 0, current, current, pc, sp, pc, sp, 0, compare, compare,
                *row[12:15], *service, *budgets]
        prefix.append(EVENT + b" ".join(str(v).encode() for v in poll) + b"\n")
    return b"".join(prefix + [f"NATIVE FIXED DROPPED {dropped}\n".encode(), *lines[-2:]])


class NativeReplayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        if sha(PACKET.read_bytes()) != PACKET_SHA:
            raise AssertionError("EVD-129 ZIP identity mismatch")
        cls.uart = {}
        with zipfile.ZipFile(PACKET) as archive:
            for label, digest in UART_SHA.items():
                data = archive.read(f"d905/{label}/uart")
                if sha(data) != digest:
                    raise AssertionError(f"wrong EVD-129 UART for {label}")
                cls.uart[label] = data

    def test_original_streams_across_reader_boundaries(self):
        for label, raw in self.uart.items():
            policy = label.split("-")[0]
            for chunk_size in (1, 31, 4096):
                with self.subTest(label=label, chunk_size=chunk_size):
                    m = Matcher(f"pi4-native-{policy}")
                    for i in range(0, len(raw) - 1, chunk_size):
                        self.assertTrue(m.feed(raw[i:min(i + chunk_size, len(raw) - 1)]))
                    self.assertIsNone(m.variant)
                    self.assertTrue(m.feed(raw[-1:]))
                    self.assertEqual(m.variant, f"pi4-native-{policy}")
                    self.assertEqual(m.frequency, 62500000)
                    self.assertEqual(m.rotations, 16 if policy == "rr" else 0)
                    self.assertIsNone(m.failure_offset)

    def test_full_journal_only_credits_observations_in_its_retained_prefix(self):
        cases = (
            ("rr", 1, 2, False, "inconclusive-retained-prefix"),
            ("rr", 2, 2, False, "inconclusive-retained-prefix"),
            ("rr", 3, 2, True, "rr-alternation-retained-prefix"),
            ("fifo", 1, 2, False, "inconclusive-retained-prefix"),
            ("fifo", 2, 1, True, "fifo-retention-through-first-exit"),
        )
        for policy, prefix, omitted, qualified, observation in cases:
            with self.subTest(policy=policy, original_rows=prefix):
                raw = full_journal(self.uart[f"{policy}-plain"], prefix, omitted)
                m = Matcher(f"pi4-native-{policy}")
                self.assertTrue(m.feed(raw))
                self.assertTrue(m.packet_complete)
                self.assertFalse(m.journal_complete)
                self.assertEqual((m.events, m.dropped, m.observation), (128, omitted, observation))
                self.assertEqual(m.variant, f"pi4-native-{policy}" if qualified else None)
        for impossible_drop in (0, 1):
            m = Matcher("pi4-native-rr")
            self.assertFalse(m.feed(full_journal(self.uart["rr-plain"], 2, impossible_drop)))

    def test_policy_identity_event_integrity_and_drop_controls(self):
        rr = self.uart["rr-plain"]
        events = [line for line in rr.splitlines(keepends=True) if line.startswith(EVENT)]
        event = events[1]
        values = event[len(EVENT):].split()
        swapped = rr.replace(APP_SHA[0], b"@").replace(APP_SHA[1], APP_SHA[0]).replace(b"@", APP_SHA[1])
        controls = {
            "fifo grammar": self.uart["fifo-plain"],
            "different app hash": rr.replace(APP_SHA[0], b"0" * 64),
            "swapped apps": swapped,
            "exit code": rr.replace(b"EXIT PID 1 77", b"EXIT PID 1 12"),
            "missing column": rr.replace(event, EVENT + b" ".join(values[:-1]) + b"\n"),
            "noninteger": rr.replace(event, EVENT + b"NaN " + b" ".join(values[1:]) + b"\n"),
            "counter overflow": rr.replace(event, EVENT + b"18446744073709551616 " + b" ".join(values[1:]) + b"\n"),
            "torn event inside capture": rr.replace(event, event[:len(event) // 2] + b"\n"),
            "duplicate event": rr.replace(event, event + event),
            "zero rotations": early_rr_exits(rr, 0),
            "only A to B": early_rr_exits(rr, 1),
            "missing return rotation with claimed drop": (
                rr.replace(events[2], b"").replace(b"DROPPED 0", b"DROPPED 1")),
            "disclosed drop": rr.replace(b"DROPPED 0", b"DROPPED 1"),
            "garbage after final LF": rr + b"\x00",
        }
        for name, raw in controls.items():
            with self.subTest(name=name):
                self.assertNotEqual(raw, rr)
                m = Matcher("pi4-native-rr")
                self.assertFalse(m.feed(raw))
                self.assertIsNone(m.variant)
                self.assertIsNotNone(m.failure_offset)
        m = Matcher("pi4-native-fifo")
        self.assertFalse(m.feed(rr))
        for prefix in (rr[:rr.index(event) + len(event) // 2], rr[:-1]):
            m = Matcher("pi4-native-rr")
            self.assertTrue(m.feed(prefix))
            self.assertIsNone(m.variant)

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.folder = Path(tmp.name)
        self.runs = 0

    def stage(self, policy):
        with zipfile.ZipFile(PACKET) as archive:
            for base, members in ((f"d905/boot-{policy}", BOOT_FILES),
                                  (f"d905/{policy}", PACKAGE_FILES)):
                for member in members:
                    archive.extract(f"{base}/{member}", self.folder)
        return self.folder / f"d905/{policy}"

    def replay(self, package, profile, inputs, expected, *, native=True):
        self.runs += 1
        output = self.folder / f"capture-{self.runs}"
        paths = []
        for i, data in enumerate(inputs):
            path = self.folder / f"source-{self.runs}-{i}.uart"
            path.write_bytes(data)
            paths.append(path)
        command = [sys.executable, str(TOOLS / "pi4_serial_capture.py"),
                   "--profile", profile, "--elf", str(package / "kernel.elf"),
                   "--boot-dir", str(package.parent / f"boot-{package.name}"),
                   "--boot-dir-source", "archive", "--output", str(output), "--replay", *map(str, paths)]
        if native:
            command.extend(("--native-manifest", str(package / "manifest.json")))
        run = subprocess.run(command, capture_output=True, timeout=75)
        self.assertEqual(run.returncode, expected, (run.stdout, run.stderr))
        return output

    def test_cli_replays_all_four_authenticated_originals(self):
        for policy in ("fifo", "rr"):
            with self.subTest(policy=policy):
                package = self.stage(policy)
                profile = f"pi4-native-{policy}"
                inputs = [self.uart[f"{policy}-{kind}"] for kind in ("plain", "traced")]
                output = self.replay(package, profile, inputs, 0)
                session = json_file(output / "session.json")
                self.assertEqual((session["mode"], session["profile"]), ("file_replay", profile))
                self.assertEqual(session["native_package"]["manifest_sha256"], MANIFEST_SHA[policy])
                self.assertEqual(session["native_package"]["policy"], policy)
                self.assertEqual(session["native_package"]["commit"], "d905d924a90ea1041821bac78919194852263fe5")
                slots = session["native_package"]["slots"]
                self.assertEqual([(s["slot"], s["elf_sha256"]) for s in slots],
                                 [(i, h.decode()) for i, h in enumerate(APP_SHA)])
                self.assertEqual(session["native_package"]["quantum_ticks"], 1250000 if policy == "rr" else None)
                self.assertEqual(session["build_association"]["capture_boot_identity"], "unverified")
                ids = []
                for n, raw in enumerate(inputs, 1):
                    folder = output / f"run-{n}"
                    self.assertEqual((folder / "uart.bin").read_bytes(), raw)
                    result = json_file(folder / "result.json")
                    self.assertEqual((result["run"], result["mode"], result["profile"]),
                                     (n, "file_replay", profile))
                    self.assertEqual(result["session_id"], session["session_id"])
                    self.assertTrue(result["journal_complete"])
                    self.assertEqual((result["native_rotations"], result["dropped"], result["observation"]),
                                     (16 if policy == "rr" else 0, 0, "complete-journal"))
                    self.assertIsNone(result["operator_off_ack_unix_ns"])
                    self.assertEqual((result["uart_verdict"], result["matched_variant"], result["uart_sha256"]),
                                     ("matched", profile, sha(raw)))
                    self.assertEqual(result["replay_input_sha256"], sha(raw))
                    ids.append(result["run_id"])
                self.assertNotEqual(*ids)

    def test_cli_preserves_failed_second_run(self):
        package = self.stage("rr")
        raw = self.uart["rr-traced"].replace(b"DROPPED 0", b"DROPPED 1") + b"X" * 5000
        output = self.replay(package, "pi4-native-rr", [self.uart["rr-plain"], raw], 1)
        self.assertEqual(json_file(output / "run-1/result.json")["uart_verdict"], "matched")
        result = json_file(output / "run-2/result.json")
        self.assertEqual(result["uart_verdict"], "unexpected_bytes")
        self.assertIsNone(result["matched_variant"])
        observed = (output / "run-2/uart.bin").read_bytes()
        self.assertTrue(raw.startswith(observed))
        self.assertLess(result["first_unexpected_offset"], len(observed))
        self.assertEqual(result["uart_sha256"], sha(observed))
        self.assertEqual((output / "run-2/replay-input.bin").read_bytes(), raw)
        self.assertEqual(result["replay_input_sha256"], sha(raw))

    def test_cli_reports_inconclusive_when_return_rotation_is_hidden(self):
        raw = full_journal(self.uart["rr-plain"], 2, 2)
        output = self.replay(self.stage("rr"), "pi4-native-rr", [raw, self.uart["rr-traced"]], 1)
        result = json_file(output / "run-1/result.json")
        self.assertEqual((result["uart_verdict"], result["observation"], result["matched_variant"]),
                         ("inconclusive", "inconclusive-retained-prefix", None))
        self.assertFalse(result["journal_complete"])
        self.assertEqual((output / "run-1/uart.bin").read_bytes(), raw)

    def test_cli_refuses_mismatched_package_before_capture(self):
        package = self.stage("fifo")
        inputs = [self.uart["fifo-plain"], self.uart["fifo-traced"]]

        def refused(profile):
            output = self.replay(package, profile, inputs, 2)
            self.assertFalse((output / "run-1/uart.bin").exists())

        refused("pi4-native-rr")
        self.replay(package, "pi4-native-fifo", inputs, 2, native=False)
        original = (package / "manifest.json").read_bytes()
        (package / "manifest.json").write_bytes(original + b"\n")
        refused("pi4-native-fifo")
        (package / "manifest.json").write_bytes(original)
        first, second = package / "user.elf", package / "native_peer.elf"
        a, b = first.read_bytes(), second.read_bytes()
        first.write_bytes(b)
        second.write_bytes(a)
        refused("pi4-native-fifo")

    def test_boot_image_hash_must_match_package_even_if_card_hashes_match(self):
        package = self.stage("fifo")
        boot = package.parent / "boot-fifo"
        image = boot / "kernel8.img"
        original = image.read_bytes()
        wrong = original[:-1] + bytes([original[-1] ^ 1])
        image.write_bytes(wrong)
        sums = boot / "SHA256SUMS"
        sums.write_bytes(sums.read_bytes().replace(sha(original).encode(), sha(wrong).encode()))
        output = self.replay(package, "pi4-native-fifo",
                             [self.uart["fifo-plain"], self.uart["fifo-traced"]], 2)
        self.assertFalse((output / "run-1/uart.bin").exists())

    def test_public_cli_accepts_legacy_scheduler_without_native_manifest(self):
        package = self.stage("fifo")
        raw = (b"JNOX PI4 BOOT\nJNOX PI4 READY\nSCHED LOW RUN\nSCHED TIMER IRQ\n"
               b"SCHED HIGH RUN\nSCHED LOW RESUMED\nSCHED PASS\n")
        output = self.replay(package, "pi4-scheduler", [raw, raw], 0, native=False)
        self.assertEqual(json_file(output / "run-2/result.json")["matched_variant"], "scheduler")


if __name__ == "__main__":
    unittest.main()
