"""Falsification checks for #417. A capture argument additionally exercises its actual artifacts."""
import argparse
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

import linux_gateway_evidence as gw


def frame(src, dst, sport, dport, seq, data, protocol, lead=0):
    if protocol == 6:
        transport = struct.pack("!HHIIBBHHH", sport, dport, seq, 0, 5 << 4, 0x18, 2048, 0, 0)
    else:
        transport = struct.pack("!HHHH", sport, dport, 8+len(data), 0)
    ip = struct.pack("!BBHHHBBH4s4s", 0x45, 0, 20+len(transport)+len(data), 1, 0,
                     64, protocol, 0, src, dst)
    return bytes(lead+12) + b"\x08\x00" + ip + transport + data


def write_pcap(path, packets):
    buf = bytearray(struct.pack("<IHHIIII", 0xa1b2c3d4, 2, 4, 0, 0, 65535, 1))
    for packet in packets:
        buf.extend(struct.pack("<IIII", 1, 0, len(packet), len(packet)) + packet)
    path.write_bytes(buf)


class EthernetChecks(unittest.TestCase):
    def test_both_tcp_directions_and_retransmit(self):
        data, back = bytes(range(24)), b"abcdefgh"
        send = lambda seq, payload: frame(gw.GUEST, gw.HOST, 8080, 3000, seq, payload, 6, 10)
        incoming = frame(gw.HOST, gw.GUEST, 3000, 8080, 48, back, 6)
        wire = [send(124, data[12:]), incoming, send(112, data[:12]), send(112, data[:12])]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/"guest.pcap"
            write_pcap(path, wire)
            gw.tcp_device_exchange(path, data, back)
            for corrupt in (wire+[send(112,b"X")], wire[:2], [incoming], wire[0:1]+wire[2:]):
                with self.subTest(wire=len(corrupt)), self.assertRaises(ValueError):
                    write_pcap(path, corrupt)
                    gw.tcp_device_exchange(path, data, back)

    def test_udp_direction_and_record_size(self):
        p = frame(gw.GUEST, gw.HOST, 43003, 43004, 0, b"one", 17)
        q = frame(gw.HOST, gw.GUEST, 43004, 43003, 0, b"two", 17, 12)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/"native.pcap"
            write_pcap(path, [p,q])
            self.assertEqual(gw.udp_records(path), [(True,b"one"), (False,b"two")])
            path.write_bytes(path.read_bytes()[:-1])
            with self.assertRaises(ValueError):
                gw.udp_records(path)


def negative_controls(root):
    """Replace one observation at a time in memory. Leave all original artifacts intact."""
    gw.verify(root)
    original = Path.read_bytes

    def rejection(path, alter, phrase):
        damaged = alter(original(path))
        def read(self):
            return damaged if self == path else original(self)
        try:
            with patch.object(Path, "read_bytes", read):
                gw.verify(root)
        except ValueError as ex:
            if phrase not in str(ex):
                raise
        else:
            raise AssertionError("accepted a changed observation: " + str(path))

    def change_word(data, index, replacement):
        m, probes = gw.layout(root/"package")
        ttbr, va, _size = probes[0]
        records = json.loads(data)
        mem = gw.PhysicalReads(records)
        word_addr = va + index*8
        page = mem.leaf(ttbr, word_addr) & 0x0000fffffffff000
        addr = page + word_addr % 4096
        for key, saved in records.items():
            base, count = key.split("/")
            begin = int(base, 16)
            if begin <= addr and addr + 8 <= begin + int(count):
                buf = bytearray.fromhex(saved)
                struct.pack_into("<Q", buf, addr-begin, replacement)
                records[key] = buf.hex()
                return json.dumps(records).encode()
        raise ValueError("failed to locate the sampled guest word")

    results = root/"native"/"receipt-final.json"
    rejection(results, lambda data: change_word(data, 13, 0), "stale/invalid")
    rejection(results, lambda data: change_word(data, 18, 1), "stale/invalid")
    rejection(results, lambda data: change_word(data, 11, 0), "monotonic time")
    native = root/"native"
    rejection(native/"receipt-linux_ready.json", lambda _data: original(native/"receipt-linux_boot.json"),
              "no heartbeat advance")
    rejection(root/"events.jsonl", lambda data: b"\n".join(
        line for line in data.splitlines() if json.loads(line)["event"] != "offline"), "host offline")
    rejection(root/"linux-2"/"uart.log", lambda data: data.replace(b"GWACCEPT ",b"GWABSENT "), "console observations")
    rejection(root/"linux-2"/"input.bin", lambda data: data.replace(b"GWREQ ", b"GWERR "), "serial-origin")
    rejection(root/"linux-2"/"wire.pcap", lambda data: data[:24], "device-operation TCP connection")
    rejection(root/"native"/"wire.pcap", lambda data: data[:24], "UDP trace")
    print("PASS nine altered native, lifecycle, Linux and wire observations were rejected")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", type=Path)
    args = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(EthernetChecks)
    if not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful():
        raise SystemExit(1)
    if args.capture:
        negative_controls(args.capture)
