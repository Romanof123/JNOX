#!/usr/bin/env python3
"""Inject two real QMP tablet clicks into a running, mapped Wayland guest."""
import argparse
import json
import socket
import struct
import time
from pathlib import Path

from qemu_capture import journal_location, request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("socket", type=Path)
    parser.add_argument("kernel", type=Path, help="exact guest ELF with symbols")
    parser.add_argument("output", type=Path)
    parser.add_argument("--points", type=int, nargs=4, default=(40, 54, 230, 146),
                        metavar=("X1", "Y1", "X2", "Y2"))
    args = parser.parse_args()
    points = list(zip(args.points[::2], args.points[1::2]))
    if any(not (0 <= x <= 319 and 0 <= y <= 239) for x, y in points):
        parser.error("points must be inside the 320 by 240 output")
    symbols = {s: journal_location(args.kernel, "jnox_tablet_" + s)
               for s in ("state", "events", "queues", "transport")}
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(3)
        sock.connect(str(args.socket))
        with sock.makefile("rwb", buffering=0) as stream:
            if "QMP" not in json.loads(stream.readline()):
                raise ValueError("invalid QMP greeting")
            request(stream, "qmp_capabilities")
            mice = request(stream, "query-mice")
            (out / "mice.json").write_text(json.dumps(mice))
            if not any(m["current"] and m["absolute"] for m in mice):
                raise ValueError("QEMU has no active absolute pointing device")

            def capture(symbol, name):
                address, size = symbols[symbol]
                path = out / name
                request(stream, "pmemsave", val=address, size=size, filename=str(path))
                return path.read_bytes()

            def state(name):
                return struct.unpack("<12Q", capture("state", name))

            boot = state("before-state.bin")
            if boot[1] == 0 or boot[2] != 1:
                raise ValueError("guest compositor has not acquired the tablet")
            capture("transport", "transport.bin")
            capture("queues", "before-queues.bin")
            sequence = boot[9]
            with (out / "qmp-input.jsonl").open("x") as log:
                stage = 0
                for x, y in points:
                    # QEMU input core accepts absolute coordinates from 0 to
                    # 32767. Round up so integer guest scaling reaches the point.
                    axes = [(x * 32767 + 318) // 319, (y * 32767 + 238) // 239]
                    motion = [{"type": "abs", "data": {"axis": axis, "value": value}}
                              for axis, value in zip(("x", "y"), axes)]
                    for packet in [motion, *[[{"type": "btn", "data": {"down": down, "button": "left"}}]
                                              for down in (True, False)]]:
                        stage += 1
                        command = {"events": packet}
                        log.write(json.dumps(command) + "\n")
                        log.flush()
                        request(stream, "input-send-event", **command)
                        deadline = time.monotonic() + 3
                        while True:
                            current = state(f"{stage}-state.bin")
                            raw = capture("events", f"{stage}-events.bin")
                            rows = sorted((seq, event, desc, length)
                                          for seq, event, desc, length in struct.iter_unpack("<4Q", raw)
                                          if seq > sequence)
                            if rows and rows[-1][1] == 0 and rows[-1][0] == current[9]:
                                break
                            if time.monotonic() >= deadline:
                                raise RuntimeError(f"missing guest SYN_REPORT for QMP packet {stage}")
                            time.sleep(.01)
                        if [r[0] for r in rows] != list(range(sequence + 1, rows[-1][0] + 1)):
                            raise ValueError("overwritten or missing raw ring receipt")
                        expected = ([3 | (axis << 16) | (value << 32) for axis, value in enumerate(axes)]
                                    if packet is motion else [1 | (272 << 16) | (int(packet[0]["data"]["down"]) << 32)])
                        if [r[1] for r in rows] != [*expected, 0]:
                            raise ValueError(f"unexpected guest events for QMP packet {stage}: {rows}")
                        queue = capture("queues", f"{stage}-queues.bin")
                        for seq, event, desc, length in rows:
                            entry_index = (boot[8] + seq - boot[9] - 1) % 16
                            entry = struct.unpack_from("<II", queue, 296 + entry_index * 8)
                            observed = struct.unpack_from("<Q", queue, 424 + desc * 8)[0]
                            if length != 8 or entry != (desc, 8) or observed != event:
                                raise ValueError(f"DMA queue bytes differ at event {seq}")
                        sequence = rows[-1][0]


if __name__ == "__main__":
    main()
