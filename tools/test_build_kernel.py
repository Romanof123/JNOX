"""Architecture selection uses a temporary registry and a simulated Cargo process."""

import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock

import build_kernel


class BuildKernelTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        descriptor = self.root / "arch" / "aarch64" / "target"
        descriptor.parent.mkdir(parents=True)
        descriptor.write_text("aarch64-unknown-none\n", encoding="utf-8")
        (self.root / "arch" / "notes").mkdir()
        self.runner = Mock(return_value=Mock(returncode=17))

    def launch(self, args, env):
        return build_kernel.main(args, root=self.root, environ=env, run=self.runner)

    def test_list_only_registered_architectures(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(self.launch(["--list-arches"], {"JNOX_ARCH": ""}), 0)
        self.assertEqual(output.getvalue(), "aarch64\n")
        self.runner.assert_not_called()

    def test_default_and_explicit_arch_preserve_platform_and_status(self):
        platform = {"JNOX_BOARD": "pi4", "JNOX_PROFILE": "fixed", "JNOX_APP": "fixed-fifo"}
        for args, env in [([], platform), ([], dict(platform, JNOX_ARCH="aarch64")),
                          (["--arch", "aarch64"], dict(platform, JNOX_ARCH="old"))]:
            with self.subTest(args=args, env=env):
                self.assertEqual(self.launch(args, env), 17)
                self.runner.assert_called_with(
                    ["cargo", "build", "--locked", "--release", "-j1", "--target", "aarch64-unknown-none"],
                    cwd=self.root / "kernel", env=dict(platform, JNOX_ARCH="aarch64"))

    def test_invalid_selection_stops_before_cargo(self):
        for args, env in [(["--arch", ""], {}), (["--arch", "../aarch64"], {}),
                          ([], {"JNOX_ARCH": ""}), ([], {"JNOX_ARCH": "unknown"}),
                          ([], {"CARGO_BUILD_TARGET": "incompatible"})]:
            with self.subTest(args=args, env=env), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    self.launch(args, env)
                self.assertEqual(raised.exception.code, 2)
        self.runner.assert_not_called()


if __name__ == "__main__":
    unittest.main()
