"""Exercise the public native packager without invoking a compiler."""
import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_native_el0 as package


class PartitionPhasePackaging(unittest.TestCase):
    def test_kernel_selection(self):
        with tempfile.TemporaryDirectory() as tmp:
            for app in ("partition-p", "partition-q"):
                for option in ([], ["--partition-phase", "irq-v1"],
                               ["--partition-frames", "retained-v1"],
                               ["--partition-phase", "irq-v1", "--partition-frames", "retained-v1"]):
                    with self.subTest(app=app, option=option):
                        out = Path(tmp) / ("-".join([app, *option]))
                        environments = []

                        def fake_run(command, *, env, **kwargs):
                            environments.append(dict(env))
                            if command[0] == "sh":
                                Path(command[-1]).write_bytes(b"user")
                            else:
                                kernel = Path(env["CARGO_TARGET_DIR"]) / package.TARGET / "release/jnox-kernel"
                                kernel.parent.mkdir(parents=True)
                                kernel.write_bytes(b"kernel")

                        ambient = {"JNOX_PARTITION_PHASE": "ambient", "JNOX_OTHER": "ambient",
                                   "JNOX_PARTITION_FRAMES": "ambient"}
                        with patch.dict(os.environ, ambient), patch.object(package, "output", return_value="stub"), \
                                patch.object(package.subprocess, "run", side_effect=fake_run):
                            package.main(["--out", str(out), "--app", app, *option])
                        user, kernel = environments
                        self.assertFalse(any(k.startswith("JNOX_") for k in user))
                        self.assertNotIn("JNOX_OTHER", kernel)
                        self.assertEqual(kernel["JNOX_APP"], app)
                        manifest = json.loads((out / "manifest.json").read_text())
                        if "--partition-frames" in option:
                            self.assertEqual(kernel["JNOX_PARTITION_FRAMES"], "retained-v1")
                            self.assertEqual(manifest["partition_frames"], "retained-v1")
                        else:
                            self.assertNotIn("JNOX_PARTITION_FRAMES", kernel)
                            self.assertNotIn("partition_frames", manifest)
                        if "--partition-phase" in option:
                            self.assertEqual(kernel["JNOX_PARTITION_PHASE"], "irq-v1")
                            self.assertEqual(manifest["partition_phase"], "irq-v1")
                        else:
                            self.assertNotIn("JNOX_PARTITION_PHASE", kernel)
                            self.assertNotIn("partition_phase", manifest)

    def test_invalid_options_precede_output_creation(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "package"
            cases = [(app, "irq-v1") for app in ("startup-probe", "weighted-share", "weighted-wake")]
            for app, value in [*cases, ("partition-q", "irq-v2")]:
                with self.subTest(app=app, value=value), contextlib.redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit) as error:
                        package.main(["--out", str(out), "--app", app, "--partition-phase", value])
                    self.assertEqual(error.exception.code, 2)
                    self.assertFalse(out.exists())


if __name__ == "__main__":
    unittest.main()
