"""Host-only regression fixtures for the JNP1 text-to-wire contract."""
import hashlib
from pathlib import Path
import tempfile
import unittest

from task_policy import APP, ELF, HEADER, PH, SH, SYM, TASK, compile_policy


def fixture(info=2, address=0x400000, symbol_size=4, entry=0x400000, permissions=(5, 4, 6),
            note_va=0x401000, note_version=1, note_name=b".note.jnox.abi", rx_size=4):
    names = b"\0run\0"
    section_names = b"\0" + note_name + b"\0"
    blob = bytearray(12292)
    ident = b"\x7fELF\x02\x01\x01" + bytes(9)
    ELF.pack_into(blob, 0, ident, 2, 183, 1, entry, 64, 256, 0, 64, 56, len(permissions), 64, 6, 5)
    for i, flags in enumerate(permissions):
        size = (rx_size, 24, 4)[i]
        PH.pack_into(blob, 64 + i * PH.size, 1, flags, 4096 * (i + 1), 0x400000 + 4096 * i,
                     0, size, max(size, 8), 4096)
    SH.pack_into(blob, 320, 0, 1, 6, 0x400000, 4096, 4, 0, 0, 4, 0)
    SH.pack_into(blob, 384, 0, 2, 0, 0, 640, 48, 3, 1, 8, 24)
    SH.pack_into(blob, 448, 0, 3, 0, 0, 688, len(names), 0, 0, 1, 0)
    SH.pack_into(blob, 512, 1, 7, 2, note_va, 8192, 24, 0, 0, 4, 0)
    SH.pack_into(blob, 576, 0, 3, 0, 0, 720, len(section_names), 0, 0, 1, 0)
    SYM.pack_into(blob, 664, 1, info, 0, 1, address, symbol_size)
    blob[688:688 + len(names)] = names
    blob[720:720 + len(section_names)] = section_names
    blob[8192:8216] = bytes.fromhex("05000000 04000000 01000000 4a4e4f58 00000000") + note_version.to_bytes(4, "little")
    return bytes(blob)


class PolicyTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.path = self.root / "policy.txt"
        self.elf = self.root / "my image.elf"
        self.elf.write_bytes(fixture())

    def compile(self, body):
        self.path.write_text("version 1\n" + body, encoding="utf-8")
        return compile_policy(self.path)

    def test_grouping_offsets_and_hashes(self):
        blob, manifest = self.compile(
            'task b beta run 10 255 5 19\napp a "my image.elf"\n'
            'app b "my image.elf" # note\ntask a first run 11 1 11 off\n'
            'task a second run 4294967295 2 1 4294967295\n')
        self.assertEqual(HEADER.unpack_from(blob), (b"JNP1", 1, 2, 3))
        first_elf = HEADER.size + 2 * APP.size + 3 * TASK.size
        for i, (name, first, count) in enumerate([(b"a", 0, 2), (b"b", 2, 1)]):
            name_field, offset, length, start, num = APP.unpack_from(blob, HEADER.size + i * APP.size)
            self.assertEqual(name_field, name.ljust(32, b"\0"))
            self.assertEqual((offset, length, start, num), (first_elf + i * len(fixture()), len(fixture()), first, count))
            self.assertEqual(blob[offset:offset + length], fixture())
        expected = [(b"first", 0, 0, 11, 1, 11, 0),
                    (b"second", 0, 1, 4294967295, 2, 1, 4294967295),
                    (b"beta", 1, 0, 10, 255, 5, 19)]
        for i, (name, app, slot, *settings) in enumerate(expected):
            row = TASK.unpack_from(blob, HEADER.size + 2 * APP.size + i * TASK.size)
            self.assertEqual(row, (name.ljust(32, b"\0"), app, slot, 0x400000, *settings))
        self.assertEqual(manifest["policy_sha256"], hashlib.sha256(blob).hexdigest())
        self.assertEqual(manifest["apps"][0]["elf_sha256"], hashlib.sha256(fixture()).hexdigest())

    def test_many_apps(self):
        declarations = [f'app app{i} "my image.elf"\ntask app{i} t run 1 1 1 off' for i in range(20)]
        blob, _ = self.compile("\n".join(declarations))
        self.assertEqual(HEADER.unpack_from(blob)[2:], (20, 20))

    def test_invalid_declarations_locate_line(self):
        for line in ["task absent t run 1 1 1 off", "task a t run 1 256 1 off",
                     "task a t run 1 1 2 off", "task a t run 1 1 1 0",
                     "task a t run 4294967296 1 1 off", "task a é run 1 1 1 off",
                     "task a " + "a" * 32 + " run 1 1 1 off", 'app a "my image.elf"']:
            with self.subTest(line=line), self.assertRaisesRegex(ValueError, r"policy.txt:3:"):
                self.compile('app a "my image.elf"\n' + line)
        with self.assertRaisesRegex(ValueError, r"policy.txt:2:"):
            self.compile('app a "my image.elf"')
        with self.assertRaisesRegex(ValueError, r"policy.txt:4:"):
            self.compile('app a "my image.elf"\ntask a t run 1 1 1 off\ntask a t run 2 2 2 off')

    def test_guest_native_abi_rejections_locate_app_line(self):
        for kwargs, diagnostic in [
            (dict(permissions=(5,)), "RX, R, RW"),
            (dict(permissions=(5, 6, 4)), "RX, R, RW"),
            (dict(entry=0x400004, rx_size=8), "ELF entry"),
            (dict(note_version=2), "invalid native v1 ABI note"),
            (dict(note_va=0x401004), "invalid native v1 ABI note"),
            (dict(note_name=b".note.other"), "missing native v1 ABI note"),
        ]:
            with self.subTest(kwargs=kwargs), self.assertRaisesRegex(ValueError, f"policy.txt:2: a: .*{diagnostic}"):
                self.elf.write_bytes(fixture(**kwargs))
                self.compile('app a "my image.elf"\ntask a t run 1 1 1 off')

    def test_task_symbol_must_be_file_backed_function(self):
        for image in [fixture(info=1), fixture(address=0x400004), fixture(address=0x400001),
                      fixture(symbol_size=8)]:
            with self.subTest(image=image[-4:]), self.assertRaisesRegex(ValueError, r"policy.txt:3:"):
                self.elf.write_bytes(image)
                self.compile('app a "my image.elf"\ntask a t run 1 1 1 off')


if __name__ == "__main__":
    unittest.main()
