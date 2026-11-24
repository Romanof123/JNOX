"""Linux host-only controls for quiet-gated MG2 collection. No emulator or guest is launched."""
import argparse
import json
from pathlib import Path
import socket
import struct
import sys
import tempfile
import threading
import unittest
from zipfile import ZipFile

import pi4_mg2_terminal as mg
from pi4_migration_capture import acquire, save, sha


class HostControls(unittest.TestCase):
    def test_quiet_gate(self):
        for emits_terminal in (False, True):
            with self.subTest(
                terminal=emits_terminal
            ), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                fake = root / "stdout-peer"
                # A native host process emits synthetic serial bytes to exercise the shared reader.
                line = (
                    "print('PI4 MIG FAIL synthetic host control',flush=True); time.sleep(3)"
                    if emits_terminal
                    else "print('early EOF',flush=True)"
                )
                fake.write_text("#!" + sys.executable + "\nimport time\n" + line + "\n")
                fake.chmod(0o700)
                for f in mg.FILES:
                    (root / f).write_bytes(b"synthetic transport control")
                save(
                    root / "freeze.json",
                    dict(
                        platform="raspi4b-tcg",
                        flags=mg.FLAGS,
                        case=mg.CASES[0],
                        qemu=str(fake),
                        qemu_sha256=sha(fake),
                        hashes={f: sha(root / f) for f in mg.FILES},
                    ),
                )

                class Probe:
                    called = False

                    def argv(self):
                        return []

                    def capture(self, *args):
                        self.called = True
                        raise ValueError("simulated post-terminal receiver failure")

                probe = Probe()
                status = acquire(
                    argparse.Namespace(
                        directory=root, digest=sha(root / "freeze.json"), ordinal=1
                    ),
                    probe,
                )
                summary = mg.read_json(root / "first-1/session.json")
                self.assertEqual(status, 1)
                self.assertEqual(probe.called, emits_terminal)
                self.assertEqual(
                    summary["post_terminal"]["status"],
                    "failed" if emits_terminal else "unobserved",
                )
                if emits_terminal:
                    self.assertGreaterEqual(summary["quiet_ns"], 1_000_000_000)

    def test_stopped_ram_and_cpu_attribution(self):
        with tempfile.TemporaryDirectory() as temp:
            dest = Path(temp)
            name = dest / "status.sock"
            commands = []
            cpus = [{"cpu-index": n, "thread-id": 100 + n} for n in range(4)]
            errors = []
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
                listener.bind(str(name))
                listener.listen(1)

                def server():
                    try:
                        with listener.accept()[0] as conn, conn.makefile(
                            "rwb", buffering=0
                        ) as pipe:
                            pipe.write(b'{"QMP":{}}\n')
                            for raw in pipe:
                                packet = json.loads(raw)
                                cmd = packet["execute"]
                                params = packet["arguments"]
                                answer = {}
                                if cmd == "query-status":
                                    paused = "stop" in commands
                                    answer = {
                                        "running": not paused,
                                        "status": "paused" if paused else "running",
                                    }
                                if cmd == "pmemsave":
                                    self.assertEqual(params["val"], 0x8D000)
                                    Path(params["filename"]).write_bytes(
                                        bytes(params["size"])
                                    )
                                if cmd == "query-cpus-fast":
                                    answer = cpus
                                if cmd == "human-monitor-command":
                                    answer = f"PC={params['cpu-index']:016x}"
                                commands.append(cmd)
                                pipe.write(
                                    (json.dumps({"return": answer}) + "\n").encode()
                                )
                    except Exception as error:
                        errors.append(error)

                thread = threading.Thread(target=server, daemon=True)
                thread.start()
                mg.TerminalRAM(name).capture(
                    dest,
                    {"ram": {"pa": 0x8D000, "bytes": 256}},
                    {str(100 + n): f"CPU {n}/TCG" for n in range(4)},
                )
                thread.join(5)
            self.assertEqual(errors, [])
            self.assertFalse(thread.is_alive())
            self.assertEqual(
                commands,
                [
                    "qmp_capabilities",
                    "query-status",
                    "stop",
                    "query-status",
                    "pmemsave",
                    "query-cpus-fast",
                ]
                + ["human-monitor-command"] * 4,
            )
            self.assertEqual((dest / "terminal-ram.bin").stat().st_size, 256)

    def test_archived_layout_and_committed_prefix(self):
        with ZipFile(mg.ARCHIVE) as z:
            for case in ("global", "partitioned", "clustered"):
                scope = mg.symbols_and_window(z.read(case + "/kernel.elf"))
                self.assertEqual(scope["bytes"], 162016)
                raw = bytearray(scope["bytes"])
                off = scope["symbols"]["LOG"][0] - scope["pa"]
                struct.pack_into("<9Q", raw, off, *([1] * 8), 12)
                struct.pack_into("<9Q", raw, off + 72, *([7] * 8), 99)
                struct.pack_into("<Q", raw, off + 6912, 1)
                got = mg.rows_and_frames(raw, scope)
                self.assertEqual([r["kind"] for r in got["journals"][0]["rows"]], [12])
                struct.pack_into("<Q", raw, off + 6912, 2)
                with self.assertRaisesRegex(ValueError, "invalid committed row"):
                    mg.rows_and_frames(raw, scope)

    def test_raw_bootstrap_fields_by_layout(self):
        with ZipFile(mg.ARCHIVE) as archive:
            scope = mg.symbols_and_window(archive.read("clustered/kernel.elf"))
        symbols = scope["symbols"]
        raw = bytearray(scope["bytes"])
        raw[symbols["PEER_READY"][0] - scope["pa"]] = 1
        ticks_offset = symbols[mg.LEGACY_TICKS][0] - scope["pa"]
        struct.pack_into("<2Q", raw, ticks_offset, 11, 23)
        legacy = mg.rows_and_frames(raw, scope)
        self.assertEqual(legacy["flags"]["PEER_READY"], 1)
        self.assertEqual(legacy["last_pre_eret_per_cpu"], (11, 23))

        del symbols["PEER_READY"]
        symbols[mg.PUBLISHED_TICKS] = symbols.pop(mg.LEGACY_TICKS)
        published = mg.rows_and_frames(raw, scope)
        self.assertNotIn("PEER_READY", published["flags"])
        self.assertEqual(published["last_pre_eret_per_cpu"], (11, 23))


if __name__ == "__main__":
    unittest.main()
