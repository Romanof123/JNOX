"""Replay a retained board gateway capture and reject individual evidence mutations."""
import argparse
import json
from pathlib import Path
import shutil
import struct
import tempfile
import unittest

from board_gateway_receipts import replay


def alter_tcp(path, *, port=False):
    """Change only the first response payload byte, or only TCP port fields."""
    data = bytearray(path.read_bytes())
    endian = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">",
              b"\x4d\x3c\xb2\xa1": "<", b"\xa1\xb2\x3c\x4d": ">"}[bytes(data[:4])]
    cursor = 24
    changed = 0
    while cursor < len(data):
        size = struct.unpack_from(endian + "I", data, cursor + 8)[0]
        frame = cursor + 16
        cursor = frame + size
        for prefix in (0, 10, 12):
            ip = frame + prefix + 14
            if ip + 20 > cursor or data[ip-2:ip] != b"\x08\x00" or data[ip] >> 4 != 4:
                continue
            if data[ip+9] != 6 or int.from_bytes(data[ip+6:ip+8], "big") & 0x3fff:
                break
            tcp = ip + (data[ip] & 15) * 4
            end = ip + int.from_bytes(data[ip+2:ip+4], "big")
            assert tcp + 20 <= end <= cursor
            source, dest = struct.unpack_from("!HH", data, tcp)
            if port:
                for offset, value in ((tcp, source), (tcp+2, dest)):
                    if value == 8080:
                        struct.pack_into("!H", data, offset, 8081)
                        changed += 1
            elif dest == 8080:
                payload = tcp + (data[tcp+12] >> 4) * 4
                if payload < end and not changed:
                    data[payload] ^= 1
                    changed = 1
            break
    assert changed, "retained capture has no matching TCP field"
    path.write_bytes(data)


class GatewayReceipts(unittest.TestCase):
    paths = None

    def setUp(self):
        if self.paths is None:
            self.skipTest("supply --out, --native and --linux to replay retained evidence")
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.out = Path(tmp.name)
        for name in ("native-pending.json", "native-pending_later.json", "native-final.json",
                     "peer.json", "wire.pcap", "uart.log"):
            shutil.copyfile(self.paths.out / name, self.out / name)

    def verify(self):
        return replay(self.out, self.paths.native, self.paths.linux)

    def load(self, name):
        return json.loads((self.out / name).read_bytes())

    def save(self, name, value):
        (self.out / name).write_text(json.dumps(value), encoding="utf-8")

    def alter_uart(self, old, new):
        path = self.out / "uart.log"
        data = path.read_bytes()
        self.assertEqual(data.count(old), 1)
        path.write_bytes(data.replace(old, new))

    def test_real_capture(self):
        self.assertEqual(len(self.verify()["device_replies"]), 2)

    def test_packet_payload(self):
        alter_tcp(self.out / "wire.pcap")
        with self.assertRaisesRegex(ValueError, "captured TCP payloads|conflicting TCP retransmission"):
            self.verify()

    def test_service_port(self):
        alter_tcp(self.out / "wire.pcap", port=True)
        with self.assertRaisesRegex(ValueError, "captured TCP payloads"):
            self.verify()

    def test_gateway_uid(self):
        self.alter_uart(b"BG PRIV uid=65534", b"BG PRIV uid=0")
        with self.assertRaisesRegex(ValueError, "Linux gateway UART transcript"):
            self.verify()

    def test_peer_response(self):
        records = self.load("peer.json")
        response = bytearray.fromhex(records[0]["response"])
        response[0] ^= 1
        records[0]["response"] = response.hex()
        self.save("peer.json", records)
        with self.assertRaisesRegex(ValueError, "peer receipt differs"):
            self.verify()

    def test_stalled_heartbeat(self):
        earlier = self.load("native-pending.json")
        later = self.load("native-pending_later.json")
        for key in ("start_ns", "end_ns"):
            earlier[key] = later[key]
        self.save("native-pending_later.json", earlier)
        with self.assertRaisesRegex(ValueError, "no independent progress"):
            self.verify()

    def test_heartbeat_after_exchange(self):
        later = self.load("native-pending_later.json")
        later["end_ns"] = self.load("peer.json")[0]["end_ns"] + 1
        self.save("native-pending_later.json", later)
        with self.assertRaisesRegex(ValueError, "outstanding observation chronology"):
            self.verify()

    def test_guest_physical_mapping(self):
        final = self.load("native-final.json")
        read = final["reads"][0]
        read["pa"] = hex(int(read["pa"], 16) + 4096)
        self.save("native-final.json", final)
        with self.assertRaisesRegex(ValueError, "unexpected retained physical read"):
            self.verify()

    def test_loaded_module_identity(self):
        self.alter_uart(b"LD LOADED virtio_net ", b"LD LOADED virtio_blk ")
        with self.assertRaisesRegex(ValueError, "Linux device identity"):
            self.verify()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("out", "native", "linux"):
        parser.add_argument("--" + name, type=Path, required=True)
    GatewayReceipts.paths = parser.parse_args()
    unittest.main(argv=[__file__], verbosity=2)
