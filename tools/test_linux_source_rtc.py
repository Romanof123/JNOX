"""Offline falsification controls for a retained native Linux-source RTC run."""
import argparse
import json
from pathlib import Path
import re
import shutil
import struct
import tempfile
import unittest

import verify_linux_source_rtc as rtc


def write_json(path, obj):
    path.write_text(json.dumps(obj), encoding="utf-8")


class RetainedRTC(unittest.TestCase):
    args = None

    def setUp(self):
        if self.args is None:
            self.skipTest("supply --package, --driver-manifest and --out")
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.work = Path(temp.name)
        self.pkg = self.args.package
        self.imp = self.args.driver_manifest
        self.out = self.work / "capture"
        self.out.mkdir()
        for name in ("launch.json", "qmp.json", "halted.json", "ram.json", "uart.log", "pl031.trace"):
            shutil.copyfile(self.args.out / name, self.out / name)
        selection = self.args.out / "rtc-gic.events"
        if selection.is_file():
            shutil.copyfile(selection, self.out / selection.name)

    def copy_package(self):
        pkg = self.work / "package"
        pkg.mkdir()
        for name in ("manifest.json", "kernel.elf", "user.elf", "native_peer.elf"):
            shutil.copyfile(self.pkg / name, pkg / name)
        self.pkg = pkg

    def test_missing_gic_ack(self):
        if not (self.out / "rtc-gic.events").is_file():
            self.skipTest("earlier capture used only device-model tracing")
        self.change_trace(r"^.*gic_acknowledge_irq cpu 0 acknowledged irq 34(?:\n|$)", "")
        self.assertRaisesRegex(ValueError, re.escape("missing ordered device trace ('gic_ack', 34)"), self.replay)

    def replay(self):
        return rtc.replay(self.pkg, self.imp, self.out)

    def change_trace(self, pattern, replacement):
        path = self.out / "pl031.trace"
        trace, count = re.subn(pattern, replacement, path.read_text(), flags=re.MULTILINE)
        self.assertGreater(count, 0, "mutation must change a real device observation")
        path.write_text(trace)

    def test_original_capture(self):
        self.assertEqual(self.replay()["peer_wakes"], 8)

    def test_alarm_not_raised(self):
        self.change_trace(r"^.*\bpl031_alarm_raised[ \t]+alarm raised[ \t]*(?:\n|$)", "")
        with self.assertRaisesRegex(ValueError, re.escape("missing ordered device trace ('alarm_raised',)")):
            self.replay()

    def test_level_never_cleared(self):
        self.change_trace(r"^.*\bpl031_irq_state[ \t]+irq state 0[ \t]*(?:\n|$)", "")
        with self.assertRaisesRegex(ValueError, re.escape("missing ordered device trace ('irq_state', 0)")):
            self.replay()

    def test_rtc_load_value_differs(self):
        self.change_trace(r"(\bpl031_write\s+addr 0x0*8 value )0x[0-9a-fA-F]+$",
                          lambda m: m[1] + hex(int(m[0].split()[-1], 16) ^ 1))
        with self.assertRaisesRegex(ValueError, re.escape("missing ordered device trace ('write', 8,")):
            self.replay()

    def test_peer_denial_in_physical_ram(self):
        path = self.out / "ram.json"
        records = rtc.load(path)
        root = rtc.linked_roots(self.pkg / "kernel.elf", 2)[1]
        va, size = rtc.journal_location(self.pkg / "native_peer.elf", "jnox_rtc_peer_result")
        self.assertGreaterEqual(size, 32)
        word_va = va + 3 * 8
        descriptor = rtc.RAM(records).leaf(root, word_va)
        self.assertIsNotNone(descriptor)
        address = (descriptor & rtc.PA_MASK) + word_va % 4096
        changed = 0
        for key, raw in records.items():
            base_text, size_text = key.split("/")
            base, length = int(base_text, 16), int(size_text)
            if base <= address and address + 8 <= base + length:
                block = bytearray.fromhex(raw)
                self.assertEqual(struct.unpack_from("<q", block, address - base)[0], -13)
                struct.pack_into("<q", block, address - base, 0)
                records[key] = block.hex()
                changed += 1
        self.assertGreater(changed, 0)
        write_json(path, records)
        with self.assertRaisesRegex(ValueError, "denied peer or independent progress"):
            self.replay()

    def test_enlarged_grant(self):
        self.copy_package()
        path = self.pkg / "manifest.json"
        manifest = rtc.load(path)
        manifest["linux_source_rtc"]["mmio_bytes"] += 4096
        write_json(path, manifest)
        # Bind the altered manifest so replay reaches its grant validation.
        path_launch = self.out / "launch.json"
        launch = rtc.load(path_launch)
        launch["identities"]["package_sha256"] = rtc.sha(path)
        write_json(path_launch, launch)
        with self.assertRaisesRegex(ValueError, "native RTC profile"):
            self.replay()

    def test_swapped_service_elf(self):
        self.copy_package()
        shutil.copyfile(self.pkg / "native_peer.elf", self.pkg / "user.elf")
        with self.assertRaisesRegex(ValueError, "original object or linked native service identity"):
            self.replay()

    def test_changed_original_digest(self):
        meta = rtc.load(self.imp)
        meta["source"]["sha256"] = meta["upstream"]["sha256"] = "0" * 64
        self.imp = self.work / "import.json"
        write_json(self.imp, meta)
        with self.assertRaisesRegex(ValueError, "original Linux translation unit"):
            self.replay()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("package", "driver-manifest", "out"):
        parser.add_argument("--" + flag, type=Path, required=True)
    RetainedRTC.args = parser.parse_args()
    unittest.main(argv=[__file__], verbosity=2)
