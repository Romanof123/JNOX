"""Offline byte controls for the native EL0 UART observer."""
import unittest

from native_el0_replay import check_uart


class NativeUartTests(unittest.TestCase):
    def test_complete_stream_and_corruptions(self):
        digest = "1a" * 32
        lines = [b"JNOX M1 BOOT", b"JNOX M1 READY",
                 b"NATIVE EL0 ELF " + digest.encode("ascii"), b"NATIVE EL0 EXIT 37"]
        raw = b"\n".join(lines) + b"\n"
        check_uart(raw, digest)
        corruptions = {
            "missing common lines": raw.split(b"\n", 2)[2],
            "changed ready": raw.replace(b"READY", b"WAIT"),
            "reordered boot": raw.replace(lines[0] + b"\n" + lines[1], lines[1] + b"\n" + lines[0]),
            "wrong image": raw.replace(digest.encode(), b"0" * 64),
            "wrong status": raw.replace(b"EXIT 37", b"EXIT 0"),
            "application bytes": raw.replace(lines[3], b"37\n" + lines[3]),
            "extra prefix": b"x" + raw,
            "extra suffix": raw + b"\0",
            "missing final LF": raw[:-1],
            "CRLF": raw.replace(b"\n", b"\r\n"),
        }
        for name, changed in corruptions.items():
            with self.subTest(name=name), self.assertRaises(ValueError):
                check_uart(changed, digest)


if __name__ == "__main__":
    unittest.main()
