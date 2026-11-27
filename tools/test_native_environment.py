import argparse
import unittest
from types import SimpleNamespace

from build_native_el0 import encode_environment


class NativeEnvironmentTests(unittest.TestCase):
    def encode(self, declarations, vectors=None, policy="fifo", **options):
        class Parser(argparse.ArgumentParser):
            def error(self, message):
                raise ValueError(message)

        args = SimpleNamespace(env=declarations, **{
            "elf": "app", "source": None, "peer_elf": ["app"] * 4, "exec_child": [],
            **options,
        })
        return encode_environment(Parser(), args, vectors or [[] for _ in range(5)], policy)

    def test_slot_isolation_empty_values_and_utf8(self):
        env, wire = self.encode(["0:DISPLAY_1=/wayland-0", "4:DISPLAY_1=한=글", "4:_EMPTY="])
        self.assertEqual(env[0], {"DISPLAY_1": "/wayland-0"})
        self.assertEqual(env[1:4], [{}, {}, {}])
        self.assertEqual(bytes.fromhex(wire["JNOX_NATIVE_ENV4_HEX"]), "DISPLAY_1=한=글\0_EMPTY=\0".encode())
        self.assertEqual(set(wire), {"JNOX_NATIVE_ENV0_HEX", "JNOX_NATIVE_ENV4_HEX"})

    def test_rejects_invalid_inputs(self):
        for invalid in ("A=B", "00:A=x", "5:A=x", "0:1A=B", "0:x-y=", "0:A", "0:=B",
                        "0:Ä=v", "0:A=\0", "0:A=\ud800"):
            with self.subTest(declaration=invalid), self.assertRaises(ValueError):
                self.encode([invalid])
        for args in ((["0:A=", "0:A=b"], {}), (["3:A=x"], {"peer_elf": ["p"]}),
                     (["0:A="], {"elf": None}), (["1:A="], {"exec_child": ["1:a", "2:b"]})):
            with self.assertRaises(ValueError):
                self.encode(args[0], **args[1])
        with self.assertRaises(ValueError):
            self.encode(["0:A=x"], policy="amc")

    def test_combined_limits_and_legacy(self):
        env, wire = self.encode([])
        self.assertEqual((env, wire), ([{}] * 5, {}))
        vectors = [["x"] * 14, [], [], [], []]
        # 28 bytes of arguments plus 'A=', 993 value bytes and the final NUL.
        self.encode(["0:A=" + "x" * 993], vectors)
        with self.assertRaises(ValueError):
            self.encode(["0:A=" + "x" * 994], vectors)
        with self.assertRaises(ValueError):
            self.encode(["0:A=", "0:B="], vectors)


if __name__ == "__main__":
    unittest.main()
