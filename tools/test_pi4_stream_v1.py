"""Independent application vectors for the optional Pi 4 sample stream."""
import unittest
import zlib

from pi4_migration_capture import STREAM
from pi4_migration_replay import checkpoint_summary, stream_reference


class StreamReferenceTests(unittest.TestCase):
    def test_all_blocks_and_first_payload(self):
        prefixes, terminal = stream_reference()
        self.assertEqual(list(prefixes), list(range(1024, 4194305, 1024)))
        first = bytes(value ^ page for page in range(4) for value in range(256))
        self.assertEqual(prefixes[1024], zlib.crc32(first))
        self.assertEqual(terminal, (4096, 8765611335505774534, 4194304, 0x78d22ad1))

    def test_default_and_pinned_activations(self):
        self.assertEqual(checkpoint_summary(0, 1), (4096, 18080812676020141577, 65536, 2147516416))
        for task, epoch in ((1, 1), (2, 0), (2, 1)):
            self.assertEqual(checkpoint_summary(task, epoch, STREAM), checkpoint_summary(task, epoch))
