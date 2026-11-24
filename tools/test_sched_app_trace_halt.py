"""Synthetic transport controls for the normal capture wrapper, seal and decoder."""
import contextlib
import io
import json
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import qemu_capture as common
import sched_app_trace_capture as wrapper
import sched_app_trace_decode as reader
from test_sched_app_trace_decode import trace, witness


UART = b"JNOX M1 BOOT\nJNOX M1 READY\nMLFQ WAKE ADMIT\nMLFQ WAKE VERIFIED\nMULTILEVEL DONE\n"
RUNNING = {"running": True, "status": "running", "singlestep": False}
PAUSED = {"running": False, "status": "paused", "singlestep": False}
MEMORY = {"mlfq_wake_witness": witness(), "jnox_sched_app_trace_v1": trace(mode="disabled"),
          "multilevel_terminal_ctl": struct.pack("<Q", 0)}
SYMBOLS = {"fixed_runtime_idle": (0x1000, 8)}
for i, (name, data) in enumerate(MEMORY.items(), 1):
    SYMBOLS[name] = (i * 0x10000, len(data))


class QmpPeer:
    """Exercise the production request framing while controlling only QMP replies."""
    def __init__(self, out, pre=RUNNING, post=PAUSED, fail_at=None, require_halt=True):
        self.out = out
        self.pre = pre
        self.post = post
        self.fail_at = fail_at
        self.require_halt = require_halt
        self.commands = []
        self.reply = {"QMP": {}}

    def write(self, data):
        message = json.loads(data)
        cmd, args = message["execute"], message["arguments"]
        self.commands.append(cmd)
        if self.require_halt and cmd in ("human-monitor-command", "pmemsave"):
            if reader.load(self.out / "halted.json") != self.post:
                raise AssertionError("missing or incorrect paused observation before extraction")
        after_stop = "stop" in self.commands
        if (cmd, after_stop) == self.fail_at:
            self.reply = {"error": {"class": "GenericError", "desc": "fixture failure"}}
            return
        if cmd == "query-status":
            result = self.post if after_stop else self.pre
        elif cmd == "query-cpus-fast":
            result = [{"cpu-index": 0}]
        elif cmd == "human-monitor-command":
            result = "PC=00001004"
        elif cmd == "pmemsave":
            symbol = next(k for k, (addr,size) in SYMBOLS.items()
                          if (addr,size) == (args["val"],args["size"]))
            Path(args["filename"]).write_bytes(MEMORY[symbol])
            result = {}
        elif cmd in ("qmp_capabilities", "stop"):
            result = {}
        else:
            raise AssertionError(cmd)
        self.reply = {"return": result}

    def readline(self):
        return json.dumps(self.reply).encode() + b"\n"


def stub_socket(peer):
    sock = mock.MagicMock()
    sock.__enter__.return_value = sock
    sock.makefile.return_value = contextlib.nullcontext(peer)
    return sock


class HaltHandoff(unittest.TestCase):
    def drive(self, out, peer):
        # Run capture(), main(), snapshot() and request(); fake the guest and local tools.
        elf = out.parent / "fixture.elf"
        elf.write_bytes(b"synthetic ELF fixture")
        table = "\n".join(f"{addr:x} {size:x} B {name}" for name,(addr,size) in SYMBOLS.items())

        def output(cmd, **kwargs):
            if cmd[:2] == ["git", "rev-parse"]:
                return "a"*40
            if cmd[:2] == ["git", "status"]:
                return ""
            if cmd[1] == "--version":
                return "synthetic tool"
            if cmd[:2] == ["aarch64-linux-gnu-nm", "-S"]:
                return table
            raise AssertionError(cmd)

        read_fd, write_fd = os.pipe()
        with os.fdopen(write_fd, "wb") as writer:
            writer.write(UART)
        guest = mock.MagicMock()
        guest.__enter__.return_value = guest
        guest.poll.return_value = None
        guest.pid = 123
        guest.returncode = 0
        handle = []
        original_tempdir = tempfile.TemporaryDirectory
        with contextlib.ExitStack() as ctx:
            guest.stdout = ctx.enter_context(os.fdopen(read_fd, "rb"))
            ctx.enter_context(mock.patch.dict(os.environ, {
                wrapper.ENV: str(out), "JNOX_BOARD": "virt", "JNOX_PROFILE": "multilevel",
                "JNOX_APP": "mlfq-wake", "JNOX_SCHED_APP_TRACE": "disabled"}, clear=True))
            ctx.enter_context(mock.patch.object(sys, "argv", ["wrapper", "-kernel", str(elf)]))
            ctx.enter_context(mock.patch.object(sys, "stdout", mock.Mock(buffer=io.BytesIO())))
            ctx.enter_context(mock.patch.object(common.subprocess, "check_output", side_effect=output))
            ctx.enter_context(mock.patch.object(common.signal, "signal", side_effect=lambda _, cb: handle.append(cb)))
            ctx.enter_context(mock.patch.object(common.time, "sleep", side_effect=lambda _: handle[0](0, None)))
            ctx.enter_context(mock.patch.object(common.tempfile, "TemporaryDirectory",
                                               side_effect=lambda **_: original_tempdir(dir=out.parent)))
            ctx.enter_context(mock.patch.object(common.socket, "socket", return_value=stub_socket(peer)))
            def launch(*args, **kwargs):
                (out / "int.trace").write_bytes(b"synthetic transcript")
                return guest
            ctx.enter_context(mock.patch.object(common.subprocess, "Popen", side_effect=launch))
            wrapper.capture()
            build, boot = out.parent/"build.log", out.parent/"boot.log"
            build.write_text("synthetic build")
            boot.write_text("Run 1/1  ELF SHA-256 = "+reader.digest(elf)+"\nPASS: 1 cold boot(s)\n")
            self.assertTrue(reader.seal(out, build, boot))
            return reader.decode(out)

    def test_observed_handoff_through_decoder(self):
        with tempfile.TemporaryDirectory() as td:
            out = Path(td)/"capture"
            peer = QmpPeer(out)
            report = self.drive(out, peer)
            self.assertEqual(report["coverage"], "disabled")
            self.assertEqual(reader.load(out/"qmp.json")["status"], RUNNING)
            self.assertEqual(reader.load(out/"qmp.json")["stop_return"], {})
            self.assertEqual(reader.load(out/"halted.json"), PAUSED)
            self.assertEqual(peer.commands, ["qmp_capabilities", "query-status", "query-cpus-fast",
                             "stop", "query-status", "human-monitor-command"] + ["pmemsave"]*3)

    def test_failed_transitions_preserve_observations(self):
        for pre, post, fail, stage in [
            (PAUSED, PAUSED, None, "pre-stop state"),
            (RUNNING, PAUSED, ("query-cpus-fast", False), "pre-stop query-cpus-fast"),
            (RUNNING, PAUSED, ("stop", True), "stop"),
            (RUNNING, RUNNING, None, "post-stop state"),
            (RUNNING, {"running": False, "status": "shutdown"}, None, "post-stop state"),
            (RUNNING, PAUSED, ("query-status", True), "post-stop query-status"),
        ]:
            with self.subTest(stage=stage, post=post), tempfile.TemporaryDirectory() as td:
                out = Path(td)/"capture"
                peer = QmpPeer(out, pre, post, fail)
                with self.assertRaises((ValueError, RuntimeError)):
                    self.drive(out, peer)
                self.assertEqual(reader.load(out/"qmp.json")["status"], pre)
                self.assertEqual(reader.load(out/"halt-error.json")["stage"], stage)
                self.assertIn("error", reader.load(out/"capture.json"))
                self.assertNotIn("human-monitor-command", peer.commands)
                self.assertNotIn("pmemsave", peer.commands)
                if stage == "post-stop state":
                    self.assertEqual(reader.load(out/"halted.json"), post)
                else:
                    self.assertFalse((out/"halted.json").exists())

    def test_archived_qmp_types_and_earlier_format(self):
        old = {"status": RUNNING, "cpus": [{"cpu-index": 0}]}
        reader.check_pre_halt(old)
        reader.check_pre_halt(dict(old, stop_return={}))
        for index in (False, 0.0, "0", None, 1):
            with self.subTest(index=index), self.assertRaisesRegex(ValueError, "CPU0"):
                reader.check_pre_halt(dict(old, cpus=[{"cpu-index": index}]))
        for result in (False, None, [], {"unexpected": 1}):
            with self.subTest(result=result), self.assertRaisesRegex(ValueError, "stop result"):
                reader.check_pre_halt(dict(old, stop_return=result))

    def test_collector_typed_input(self):
        for index, result, stage in [(False, {}, "pre-stop state"),
                                     (0.0, {}, "pre-stop state"),
                                     (0, [], "stop"), (0, None, "stop")]:
            with self.subTest(index=index, result=result), tempfile.TemporaryDirectory() as td:
                out = Path(td)
                replies = [RUNNING, [{"cpu-index": index}], result]
                with mock.patch.object(common, "request", side_effect=replies) as transport:
                    with self.assertRaises(ValueError):
                        common.capture_halt(None, out)
                self.assertEqual(reader.load(out / "halt-error.json")["stage"], stage)
                commands = [call.args[1] for call in transport.call_args_list]
                expected = ["query-status", "query-cpus-fast"]
                if stage == "stop":
                    expected.append("stop")
                    self.assertEqual(reader.load(out / "qmp.json")["stop_return"], result)
                self.assertEqual(commands, expected)

    def test_existing_frozen_snapshot(self):
        with tempfile.TemporaryDirectory() as td:
            out = Path(td)
            peer = QmpPeer(out, require_halt=False)
            with mock.patch.object(common.socket, "socket", return_value=stub_socket(peer)):
                common.snapshot(out/"socket", out, *SYMBOLS["mlfq_wake_witness"], freeze=True)
            self.assertEqual(peer.commands, ["qmp_capabilities", "query-status", "query-cpus-fast",
                             "stop", "human-monitor-command", "pmemsave"])
            self.assertTrue((out/"audit.bin").exists())
            self.assertFalse((out/"halted.json").exists())


if __name__ == "__main__":
    unittest.main()
